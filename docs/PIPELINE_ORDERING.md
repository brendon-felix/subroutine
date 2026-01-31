# Pipeline Ordering System

This document describes how the pipeline ordering system works and how to use the methods for inserting, reordering, and deleting items.

## Overview

The pipeline uses a position-based ordering system where each `PipelineItem` has a `position` field (an `i64`). Items are ordered by their position value in ascending order. The system **automatically maintains position consistency** by:

1. Shifting positions when items are inserted
2. Shifting positions when items are deleted
3. Shifting positions when items are reordered
4. Normalizing positions after operations to ensure they're contiguous (1, 2, 3, ...)

This ensures that pipeline positions never have gaps and remain consistent regardless of how many operations are performed.

## Key Concepts

### Positions
- Positions are 1-based integers (starting from 1, not 0)
- Positions don't need to be contiguous (gaps are allowed)
- Items are always ordered by `position ASC, created_at ASC` when fetched
- Lower position values appear first in the pipeline

### Position Management
The system maintains position consistency through three main operations:

1. **Insertion at Position** - Inserts a new item and shifts existing items
2. **Reordering** - Moves an item to a new position and shifts other items
3. **Normalization** - Compresses positions to be contiguous (1, 2, 3, ...)

## Database Functions

### `insert_instance_at_position()`
Creates a new instance and adds it to the pipeline at a specific position.

**Signature:**
```rust
pub async fn insert_instance_at_position(
    pool: &SqlitePool,
    action: &Action,
    status: &str,
    pipeline_id: &str,
    position: i64,
) -> Result<(Instance, PipelineItem)>
```

**Behavior:**
- Creates a new instance with the given action
- Shifts all items at positions >= target position UP by 1
- Creates a new pipeline item at the target position
- Returns both the instance and pipeline item

**Example:**
```rust
// Insert action at position 3 in the default pipeline
let (instance, item) = database::insert_instance_at_position(
    &pool,
    &action,
    "pending",
    DEFAULT_PIPELINE_ID,
    3
).await?;
// Items that were at positions 3+ are now at positions 4+
```

### `update_pipeline_item_position()`
Moves an existing pipeline item to a new position.

**Signature:**
```rust
pub async fn update_pipeline_item_position(
    pool: &SqlitePool,
    pipeline_item_id: &str,
    new_position: i64,
) -> Result<()>
```

**Behavior:**
- Fetches the item to get its current position and pipeline_id
- If moving UP (new_position < current_position):
  - Items between new_position and current_position are shifted DOWN by 1
- If moving DOWN (new_position > current_position):
  - Items between current_position and new_position are shifted UP by 1
- Updates the item to its new position
- Returns early if new_position == current_position (no-op)

**Example:**
```rust
// Move item from position 5 to position 2
let item_id = "some-pipeline-item-id";
database::update_pipeline_item_position(&pool, item_id, 2).await?;

// Before: [1, 2, 3, 4, 5, 6, 7]  (item at 5)
// After:  [1, 2, 3, 4, 5, 6, 7]  (item at 2, previous 2-4 shifted to 3-5)
```

### `normalize_pipeline_positions()`
Compresses positions to be contiguous (1, 2, 3, ..., N) without gaps.

**Signature:**
```rust
pub async fn normalize_pipeline_positions(
    pool: &SqlitePool,
    pipeline_id: &str,
) -> Result<()>
```

**Behavior:**
- Fetches all items ordered by position and creation time
- Reassigns positions sequentially (1, 2, 3, ...)
- Called automatically after every insertion and reordering operation
- Also called after deletion to ensure consistent positions
- Preserves the relative order of items

**Note:** This is called automatically by the system, so you typically don't need to call it manually.

**Example:**
```rust
// Manual normalization (rarely needed - happens automatically)
database::normalize_pipeline_positions(&pool, DEFAULT_PIPELINE_ID).await?;
```

## Store Methods

The `DatabaseStore` provides async-aware wrappers around the database functions:

### `insert_instance_at_position()`
```rust
pub fn insert_instance_at_position(
    &self,
    action_id: String,
    position: i64,
    cx: &mut Context<Self>,
)
```

Creates an instance at a specific position and reloads the pipeline.

**Usage in Views:**
```rust
database_store.update(cx, |store, cx| {
    store.insert_instance_at_position(
        action_id,
        position,
        cx,
    );
    cx.notify();
});
```

### `reorder_pipeline_item()`
```rust
pub fn reorder_pipeline_item(
    &self,
    pipeline_item_id: String,
    new_position: i64,
    cx: &Context<Self>,
)
```

Reorders an existing pipeline item to a new position.

**Usage in Views:**
```rust
database_store.update(cx, |store, cx| {
    store.reorder_pipeline_item(
        pipeline_item_id,
        new_position,
        cx,
    );
});
```

### `normalize_positions()`
```rust
pub fn normalize_positions(&self, cx: &Context<Self>)
```

Normalizes all positions in the default pipeline to be contiguous.

**Usage in Views:**
```rust
database_store.update(cx, |store, cx| {
    store.normalize_positions(cx);
});
```

## Position Consistency Guarantees

The system ensures:

1. **No Gaps After Operations** - When inserting or reordering, other items automatically shift
2. **Relative Order Preserved** - Moving an item doesn't change the relative order of items it doesn't affect
3. **Idempotent Operations** - Calling the same operation twice has the same effect as calling it once
4. **Database Consistency** - All position updates happen transactionally in the same operation

## Example Scenarios

### Scenario 1: Drag & Drop Reordering
```rust
// User drags item from position 3 to position 1
// Before: [A:1, B:2, C:3, D:4, E:5]
// User wants: [C:1, A:2, B:3, D:4, E:5]

store.reorder_pipeline_item("item-C-id", 1, cx);

// After:
// C:1, A:2, B:3, D:4, E:5
```

### Scenario 2: Insert at Top
```rust
// User drops new action at position 0 (top of pipeline)
// Before: [A:1, B:2, C:3]

store.insert_instance_at_position(action_id, 1, cx);

// After: [NEW:1, A:2, B:3, C:4]
```

### Scenario 3: Multiple Deletions
```rust
// User deletes items at original positions 2, 4, 6
// Before: [A:1, B:2, C:3, D:4, E:5, F:6, G:7]

// After deleting B, D, F:
// [A:1, C:2, E:3, G:4]  <- positions have gaps

// Call normalize to fix:
store.normalize_positions(cx);

// After normalization:
// [A:1, C:2, E:3, G:4]  <- same result, but clean
```

## Implementation Details

### Insertion Algorithm
When inserting at position N:
1. Shift all items with position >= N: `position = position + 1`
2. Insert new item at position N
3. Automatically normalize all positions (1, 2, 3, ...)
4. Result: All items maintain relative order, with new item at position N, and contiguous positions

### Deletion Algorithm
When deleting a pipeline item at position P:
1. Delete the item from the database
2. Shift all items with position > P: `position = position - 1`
3. Result: No gaps in positions, all items below deleted item shift up

When deleting an instance:
1. Find all pipeline items associated with the instance
2. Delete the instance from the database
3. For each associated pipeline item, shift positions of items after it
4. Result: Instance and all its pipeline entries are removed, positions remain consistent

### Reordering Algorithm
When moving item from position A to position B:

If B < A (moving up):
1. Shift items between B and A: `position = position + 1` (where position >= B AND position < A)
2. Set item to position B
3. Automatically normalize all positions
4. Result: Item at B, items shift down to fill gap, all positions 1, 2, 3, ...

If B > A (moving down):
1. Shift items between A and B: `position = position - 1` (where position > A AND position <= B)
2. Set item to position B
3. Automatically normalize all positions
4. Result: Item at B, items shift up to fill gap, all positions 1, 2, 3, ...

### Normalization Algorithm
1. Fetch all items ordered by position, then created_at
2. Assign new positions 1, 2, 3, ... to items in that order
3. Update database with new positions for any items that changed
4. Called automatically after insertion and reordering operations
5. Called after deletion to ensure consistent positions

## Best Practices

1. **Use `reorder_pipeline_item()` for drag/drop** - It efficiently moves items without recomputing all positions
2. **Call `normalize_positions()` after many deletions** - Keeps positions clean and predictable
3. **Don't manually calculate positions** - Always use the provided methods
4. **Let the database handle shifts** - Don't try to update positions in application code
5. **Reload pipeline after operations** - The store methods do this automatically

## Edge Cases Handled

- **Moving item to itself** - `update_pipeline_item_position()` returns early (no-op)
- **Position out of bounds** - System allows any position value; items are ordered by position + created_at
- **Missing pipeline_item** - Returns error if item not found
- **Concurrent operations** - SQLite's serialization handles ordering
- **Position gaps** - System tolerates gaps; use `normalize_positions()` to clean up

## Future Enhancements

Possible improvements to consider:
- Batch reordering (move multiple items at once)
- Position validation (ensure positions are within expected range)
- Performance monitoring for large pipelines (100+ items)
- Automatic position normalization on certain thresholds