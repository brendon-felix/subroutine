# Async Deadlock Fix Summary

## Problem

The application was experiencing "stuck" operations where:
1. A database operation would complete successfully
2. But subsequent operations would hang indefinitely
3. Restarting the app would show the previously stuck operation had gone through
4. But no new operations could proceed

### Root Cause

The issue was **nested async spawns within update closures**. The pattern looked like this:

```rust
cx.spawn(async move |this, cx| {
    match database::some_operation(&pool).await {
        Ok(result) => {
            if let Err(error) = this.update(cx, move |this, cx| {
                // Update state
                this.load_all_instances(cx);  // ❌ This spawns a NEW task
                this.load_pipeline(cx);       // ❌ This spawns ANOTHER NEW task
                cx.notify();
            }) {
                // ...
            }
        }
        // ...
    }
})
.detach();
```

**Why this is a problem:**

1. The outer task spawns from the foreground thread
2. Inside the update closure, we call `load_all_instances()` and `load_pipeline()`
3. These methods spawn NEW detached tasks that also need foreground thread access
4. The outer task's closure doesn't await these spawned tasks
5. If the database pool has limited connections (max 5), and we keep spawning new loads that also need the pool:
   - Task 1: Acquires pool connection, does work, then spawns 2 load tasks
   - Task 1's update closure finishes before the load tasks complete
   - The load tasks are now waiting for pool connections
   - If more operations come in, they stack up waiting for connections
   - Foreground thread might get starved if too many detached tasks are queued

Additionally, calling `this.update()` multiple times from within a single spawned task context can cause issues with task scheduling.

## Solution

**Move all database fetches into the same async task, before the update closure.**

Instead of:
```rust
cx.spawn(async move |this, cx| {
    database::operation().await?;
    this.update(cx, |this, cx| {
        this.load_pipeline(cx);  // ❌ Nested spawn
    })?;
})?
```

Do:
```rust
cx.spawn(async move |this, cx| {
    database::operation().await?;
    
    // Fetch data BEFORE update, not inside
    let items = database::fetch_pipeline_items(&pool).await?;
    
    this.update(cx, move |this, cx| {
        this.pipeline_items = items;
        cx.notify();
    })?;
})?
```

### Changes Made

The following methods in `DatabaseStore` were refactored:

1. **`delete_pipeline_item()`**
   - Deleted the call to `this.load_pipeline(cx)` inside update
   - Added explicit `fetch_pipeline_items()` after update, with sequential updates

2. **`delete_instance()`**
   - Deleted calls to `this.load_all_instances(cx)` and `this.load_pipeline(cx)` inside update
   - Added explicit sequential fetches and updates

3. **`create_instance_for_action()`**
   - Same pattern: fetch after initial update, then update again with fetched data

4. **`enqueue_instance()`**
   - Explicit fetch pattern instead of nested spawn

5. **`update_instance_status()` (complete/uncomplete)**
   - Explicit fetch pattern for both instances and pipeline

6. **`insert_instance_at_position()`**
   - Explicit fetch pattern after insertion

7. **`reorder_pipeline_item()`**
   - Explicit fetch pattern after reordering

8. **`normalize_positions()`**
   - Explicit fetch pattern after normalization

## Pattern Used

The new pattern is:

```rust
pub fn some_operation(&self, id: String, cx: &Context<Self>) {
    let Some(pool) = self.pool() else { return; };
    
    cx.spawn(async move |this, cx| {
        // Step 1: Do the database operation
        match database::some_operation(&pool, &id).await {
            Ok(()) => {
                // Step 2: Update state (non-database)
                if let Err(error) = this.update(cx, move |this, cx| {
                    this.some_field.retain(|item| item.id != id);
                    println!("Operation completed");
                    cx.notify();
                }) {
                    eprintln!("Failed: {error}");
                    return;  // ✅ Early return on error
                }
                
                // Step 3: Fetch fresh data (still in the spawn context)
                match database::fetch_data(&pool).await {
                    Ok(data) => {
                        // Step 4: Update with fetched data
                        if let Err(error) = this.update(cx, move |this, cx| {
                            this.data_field = data;
                            cx.notify();
                        }) {
                            eprintln!("Failed to update: {error}");
                        }
                    }
                    Err(error) => {
                        if let Err(update_error) = 
                            this.update(cx, |_, cx| Self::emit_error(cx, format!("{error}")))
                        {
                            eprintln!("Failed to emit error: {update_error}");
                        }
                    }
                }
            }
            Err(error) => {
                if let Err(update_error) =
                    this.update(cx, |_, cx| Self::emit_error(cx, format!("{error}")))
                {
                    eprintln!("Failed to emit error: {update_error}");
                }
            }
        }
    })
    .detach();
}
```

## Key Points

1. **All database operations happen in the same `cx.spawn()` context**
   - No spawning of new tasks from within update closures
   - Each operation gets its pool connection and holds it until complete

2. **Sequential updates, not nested spawns**
   - First update: Update in-memory state
   - Fetch: Get fresh data from database
   - Second update: Update with fetched data
   - All in one async task

3. **Early returns on errors**
   - If an intermediate step fails, we exit immediately
   - No orphaned operations waiting for resources

4. **Clear error handling**
   - Each step has explicit error handling
   - Errors are emitted to the UI
   - No silent failures

## Benefits

✅ **No more stuck operations** - Each task completes fully before detaching
✅ **Predictable behavior** - Operations complete in order they were queued
✅ **Better error visibility** - Errors are properly propagated and logged
✅ **Fewer pool connections needed** - Less contention for database connections
✅ **Clearer code flow** - Easier to understand what happens when

## Testing

To verify the fix works:

1. Rapidly add items to the pipeline (drag/drop multiple actions quickly)
2. Delete items while others are being added
3. Reorder items
4. Mark items as complete
5. All operations should now complete smoothly without hanging

Before the fix, you'd see operations hang after 2-3 rapid operations. Now it should handle many rapid operations without issues.