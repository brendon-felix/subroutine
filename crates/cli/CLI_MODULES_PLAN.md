# CLI Modules Plan

This document outlines the plan for building out the remaining CLI modules. Each section covers a new module in `crates/cli/src/`, the subcommands it exposes, which database functions it uses, and any new database functions that need to be written first.

## ✅ Completed Modules (through Phase 6.2)

- **`actions.rs`** - Full CRUD for actions (list, create, show, delete) ✅
- **`instances.rs`** - Full management of action instances (list, create, show, status, delete) ✅
- **`resolve.rs`** - Shared identifier resolution utilities with trait-based generic resolver ✅
- **Scoring System (database)** - Extensible scoring framework with 5 basic scorers ✅

## 🚧 Status (Phase 6.2 Complete — Phase 7 Next)

**Current Step:** Context management and mental state tracking (foundation for intelligent recommendations)

**Completed:** Steps 1-3 of implementation order
**Remaining:** Context snapshots, mental states, scoring integration, smart pipeline, events, routines

**Architecture Note:** After reviewing against project philosophy, we've identified that the CLI needs **context-aware action recommendation** capabilities, not just task management. See `ARCHITECTURE_GAPS_ANALYSIS.md` for full details.

---

## Shared Patterns

All modules should follow the conventions established in `actions.rs`:

- A `<Module>Command` enum deriving `clap::Subcommand`, exported from the module.
- A `handle_<module>_command(command, conn)` entry point called from `main.rs`.
- A `resolve_<entity>(conn, identifier)` helper that matches by full ID, ID prefix, or case-insensitive name/title prefix, returning a clear error on zero or ambiguous matches.
- Short ID prefixes (`&id[..8]`) in list and confirmation output for human-friendly display.
- `allow_hyphen_values = true` on any numeric arg that accepts negative values.

After all modules are created, `main.rs` should be updated to:
1. Declare each module with `mod <name>;`.
2. Replace the remaining inline `Instances` and `Pipeline` match arms with delegated calls.
3. Add new top-level `Commands` variants for `Events` and `Routines`.

---

## ✅ 1. Instances Module (`instances.rs`) - COMPLETED (including scoring)

Instances are the core operational unit — an `Instance` is a specific occurrence of an `Action` that can be scheduled, started, completed, or deleted.

**Status:** ✅ Fully implemented and tested

### Subcommands

| Command | Description | Database Functions Used | Status |
|---------|-------------|----------------------|--------|
| `instances list` | List all instances, showing status, source, action ID prefix, and schedule info | `fetch_instances` | ✅ |
| `instances list --status <STATUS>` | Filter listed instances by status (e.g. "active", "scheduled", "completed") | `fetch_instances` (filter in Rust) | ✅ |
| `instances create <ACTION>` | Create a new instance from an action (by ID/name prefix). Defaults to status "scheduled" | `fetch_actions` (to resolve action), `insert_instance` | ✅ |
| `instances create <ACTION> --enqueue` | Create a new instance and immediately add it to the default pipeline | `create_instance_and_enqueue` | ✅ |
| `instances show <IDENTIFIER>` | Show full details of a specific instance | `fetch_instances` (resolve by ID/prefix) | ✅ |
| `instances status <IDENTIFIER> <STATUS>` | Update the status of an instance (e.g. "active", "completed", "paused") | `set_instance_status` | ✅ |
| `instances delete <IDENTIFIER>` | Delete an instance and clean up its pipeline items | `delete_instance` | ✅ |

### Implementation Notes ✅

- ✅ The `create` subcommand takes an action identifier (resolved via shared `resolve_action`) and optionally `--status` to override the default "scheduled" status, plus `--enqueue` to add to pipeline.
- ✅ The `show` command displays the associated action's title by fetching both instances and actions, then joining in Rust.
- ✅ The `list` output includes the action title (not just ID prefix) for better UX.
- ✅ Uses shared `resolve_instance` from `resolve.rs` module.

### Database Functions

All required database functions already exist. Optional optimization:

| Function | Signature | Purpose | Status |
|----------|-----------|---------|--------|
| `fetch_instance_by_id` | `(conn, id) -> Result<Option<Instance>>` | Fetch a single instance by ID to avoid loading all instances | Optional (not needed yet) |

---

## ✅ 2. Context Module (`context.rs`) - COMPLETED

Context snapshots capture the user's current state for scoring and learning. This is a core primitive that enables intelligent recommendations.

**Status:** ⏳ Not started (blocking intelligent recommendations)

### Subcommands

| Command | Description | Database Functions Used | Status |
|---------|-------------|----------------------|--------|
| `context snapshot` | Manually capture current context | **NEW:** `insert_context_snapshot` | ⏳ |
| `context snapshot --env <ENV> --location <LOC>` | Capture with specific environment/location tags | **NEW:** `insert_context_snapshot` | ⏳ |
| `context current` | Show the current/most recent context snapshot | **NEW:** `fetch_current_context` | ⏳ |
| `context history [--limit N]` | Show recent context snapshots | **NEW:** `fetch_context_snapshots` | ⏳ |
| `context set-energy <0.0-1.0>` | Set current energy level | **NEW:** `update_context_energy` | ⏳ |
| `context set-attention <0.0-1.0>` | Set current attention capacity | **NEW:** `update_context_attention` | ⏳ |

### Implementation Notes ✅

- Context snapshots should capture: time of day, day type, environment tags, location tags, mental state, energy level, attention capacity
- `snapshot` command can take optional flags for environment/location, or auto-detect where possible
- Energy and attention levels are 0.0-1.0 scale (0% to 100%)
- Context snapshots are referenced by events for learning patterns
- The scoring system uses context to make intelligent recommendations

### Database Functions ✅

| Function | Signature | Purpose |
|----------|-----------|---------|
| `insert_context_snapshot` | `(conn, snapshot) -> Result<String>` | Insert a context snapshot, returns snapshot ID |
| `fetch_context_snapshots` | `(conn, limit) -> Result<Vec<ContextSnapshot>>` | Fetch recent context snapshots |
| `fetch_current_context` | `(conn) -> Result<Option<ContextSnapshot>>` | Get most recent context snapshot |
| `update_context_energy` | `(conn, energy) -> Result<()>` | Update energy level in current context |
| `update_context_attention` | `(conn, attention) -> Result<()>` | Update attention capacity in current context |
| `build_scoring_context` | `(conn) -> Result<ScoringContext>` | Build a ScoringContext from current DB state |

**Priority:** 🔥 URGENT - Required for intelligent recommendations
**Estimated Effort:** 6-8 hours

---

## ✅ 3. Mental States Module (`mental_states.rs`) - COMPLETED

Mental state tracking enables the "Navigate by mental state" feature and is core to the Subroutine philosophy.

**Status:** ⏳ Not started (blocking core features)

### Subcommands

| Command | Description | Database Functions Used | Status |
|---------|-------------|----------------------|--------|
| `mental-states list` | List all defined mental states | **NEW:** `fetch_mental_states` | ⏳ |
| `mental-states create <NAME> [--description]` | Define a new mental state | **NEW:** `insert_mental_state` | ⏳ |
| `mental-states record <IDENTIFIER> [--intensity 1-5]` | Record that you're currently in this mental state | **NEW:** `insert_mental_state_event` | ⏳ |
| `mental-states current` | Show your current mental state | **NEW:** `fetch_current_mental_state` | ⏳ |
| `mental-states history [--hours N]` | Show recent mental state timeline | **NEW:** `fetch_mental_state_events` | ⏳ |

### Implementation Notes ✅

- Common mental states to suggest on first run: "focused", "overwhelmed", "anxious", "hyperfocused", "low-energy", "overstimulated"
- Intensity is optional 1-5 scale
- Recording a mental state should also update the current context snapshot
- Mental state is used by scoring system to filter/rank appropriate tasks
- Foundation for "Brain Interrupt", "Emotion Decoder", and "Wisdom Nuggets" features

### Database Functions ✅

| Function | Signature | Purpose |
|----------|-----------|---------|
| `insert_mental_state` | `(conn, state) -> Result<()>` | Insert or upsert a mental state definition |
| `fetch_mental_states` | `(conn) -> Result<Vec<MentalState>>` | Fetch all defined mental states |
| `insert_mental_state_event` | `(conn, event) -> Result<()>` | Record a mental state occurrence |
| `fetch_mental_state_events` | `(conn, limit) -> Result<Vec<MentalStateEvent>>` | Fetch recent mental state events |
| `fetch_current_mental_state` | `(conn) -> Result<Option<MentalState>>` | Get the most recent mental state |

**Priority:** 🔥 URGENT - Core to Subroutine philosophy
**Estimated Effort:** 4-6 hours

---

## ✅ 4. Pipeline Module (`pipeline.rs`) - COMPLETED

The pipeline is an ordered queue of items (usually backed by instances) representing what the user plans to do next.

**Status:** ✅ Complete - All commands implemented and tested!

### Subcommands

| Command | Description | Database Functions Used | Status |
|---------|-------------|----------------------|--------|
| `pipeline list` | Show all items in the default pipeline, ordered by position | `fetch_pipeline_items` | ✅ |
| `pipeline list --scored` | Show items with their current scores | `score_pipeline_items` | ✅ |
| `pipeline suggest [--count 3]` | Show top N suggested tasks based on current context (doesn't add to pipeline) | `suggest_best_instances` | ✅ |
| `pipeline refresh` | Re-score all pipeline items and re-order by score | `score_pipeline_items` | ✅ |
| `pipeline explain <IDENTIFIER>` | Show scoring breakdown for why an item is ranked as it is | `score_instance_with_context` | ✅ |
| `pipeline add <ACTION>` | Create an instance from an action and enqueue it at the end of the pipeline | `fetch_actions` (resolve), `insert_pipeline_item` | ✅ |
| `pipeline add <ACTION> --position <N>` | Create an instance and insert it at a specific position, shifting others down | `fetch_actions` (resolve), `insert_pipeline_item` | ✅ |
| `pipeline move <IDENTIFIER> --position <N>` | Move an existing pipeline item to a new position | `update_pipeline_item_position` | ✅ |
| `pipeline remove <IDENTIFIER>` | Remove a pipeline item (does not delete the underlying instance) | `delete_pipeline_item` | ✅ |
| `pipeline normalize` | Re-number all positions to be sequential starting from 1 (fix gaps) | `normalize_pipeline_positions` | ✅ |

### Implementation Notes ✅

- ✅ `pipeline list` displays each item as: `<position>. [<id_prefix>] <action_title> (<status>)`. The action title is denormalized on `PipelineItem`.
- ✅ `pipeline list --scored` additionally shows the score: `<position>. [<id_prefix>] <action_title> (<status>) [Score: X.XX]`
- ✅ **`pipeline suggest`** is the primary way users interact with the pipeline - this is the "context-aware action recommender" UX
- ✅ `pipeline refresh` re-scores based on current context (time, energy, mental state, etc.) and reorders items
- ✅ `pipeline explain` shows full scoring breakdown (which factors contributed, why)
- ✅ The `add` command uses `resolve_action` from the shared `resolve.rs` module.
- ✅ `pipeline move` and `pipeline remove` use `resolve_pipeline_item_in` to resolve pipeline items by ID prefix or action title.
- ✅ The `--pipeline` flag defaults to `DEFAULT_PIPELINE_ID` ("default") when omitted.

### Functions Implemented ✅

**resolve.rs:**
- ✅ `resolve_pipeline_item(conn, identifier) -> Result<PipelineItem>` - `Resolvable` impl for `PipelineItem`
- ✅ `resolve_pipeline_item_in(conn, pipeline_id, identifier) -> Result<PipelineItem>` - Resolve within specific pipeline

**database/scoring.rs:**
| Function | Signature | Purpose | Status |
|----------|-----------|---------|--------|
| `build_scoring_context` | `(conn) -> Result<ScoringContext>` | Build context from current DB state (mental state, context snapshot, time) | ✅ DONE |
| `score_instance_with_context` | `(conn, instance_id) -> Result<ScoredInstance>` | Score a single instance with full explanation | ✅ DONE |
| `suggest_best_instances` | `(conn, count) -> Result<Vec<(Instance, Action, f64)>>` | Score all available instances, return top N | ✅ DONE |
| `score_pipeline_items` | `(conn, pipeline_id) -> Result<Vec<(PipelineItem, f64)>>` | Score all items in a pipeline using current context | ✅ DONE |

**Completion:** ✅ All pipeline features implemented and tested (~4 hours)

---

## ✅ 5. Events Module (`events.rs`) - COMPLETED

Events track task completion outcomes and enable pattern learning. The module supports recording different event types (completed, skipped, snoozed, abandoned, etc.) and analyzing completion patterns.

**Status:** ✅ Complete (~4 hours)

### Subcommands

| Command | Description | Database Functions Used | Status |
|---------|-------------|----------------------|--------|
| `events list [--instance] [--action] [--type] [--limit]` | List events with optional filters | `fetch_tracked_events` | ✅ |
| `events record <identifier> <type> [--note]` | Record a typed event for an instance | `insert_tracked_event`, `resolve_instance` | ✅ |
| `events patterns [--action] [--limit]` | Show completion statistics and patterns | `fetch_event_patterns` | ✅ |

### Implementation Notes ✅

- Event types: `suggested`, `accepted`, `completed`, `skipped`, `snoozed`, `abandoned`
- The `record` command resolves the instance by ID, prefix, or action title
- The `list` command supports filtering by instance, action, event type, and limit
- The `patterns` command shows completion rates with visual progress bars
- Events are linked to both instances and actions for flexible querying

### Database Functions Implemented ✅

| Function | Signature | Purpose |
|----------|-----------|---------|
| `insert_tracked_event` | `(conn, event_type, instance_id, action_id, note) -> Result<TrackedEvent>` | Record a new event |
| `fetch_tracked_events` | `(conn, instance_id, action_id, event_type, limit) -> Result<Vec<TrackedEvent>>` | Fetch events with filters |
| `fetch_tracked_events_for_instance` | `(conn, instance_id, limit) -> Result<Vec<TrackedEvent>>` | Fetch events for an instance |
| `fetch_tracked_events_for_action` | `(conn, action_id, event_type, limit) -> Result<Vec<TrackedEvent>>` | Fetch events for an action |
| `fetch_recent_tracked_events` | `(conn, limit) -> Result<Vec<TrackedEvent>>` | Fetch most recent events |
| `fetch_event_patterns` | `(conn, action_id, limit) -> Result<Vec<EventPatternStats>>` | Analyze completion patterns |

---

## ✅ 6. Routines Module (`routines.rs`) - COMPLETED

Routines are templates that group actions into ordered (or randomizable) sequences.

**Status:** ✅ Complete (all database functions and CLI commands implemented)

### Subcommands

| Command | Description | Database Functions Used | Status |
|---------|-------------|----------------------|--------|
| `routines list` | List all routines | `fetch_routines` | ✅ |
| `routines create <NAME>` | Create a new routine | `insert_routine` | ✅ |
| `routines create <NAME> --description <DESC>` | Create with a description | `insert_routine` | ✅ |
| `routines create <NAME> --parallel` | Create a non-sequential routine (default is sequential) | `insert_routine` | ✅ |
| `routines create <NAME> --randomize` | Create a routine that allows randomization | `insert_routine` | ✅ |
| `routines show <IDENTIFIER>` | Show routine details including its steps | `fetch_routines`, `fetch_routine_steps` | ✅ |
| `routines delete <IDENTIFIER>` | Delete a routine and its steps (cascades) | `delete_routine` | ✅ |
| `routines add-step <ROUTINE> <ACTION>` | Add an action as the next step in a routine | `insert_routine_step`, `next_routine_step_order` | ✅ |
| `routines add-step <ROUTINE> <ACTION> --position <N>` | Add an action at a specific step order | `insert_routine_step`, `shift_routine_steps` | ✅ |
| `routines add-step ... --min-duration <N> --max-duration <N>` | Set duration bounds on the step | `insert_routine_step` | ✅ |
| `routines remove-step <ROUTINE> <STEP_ORDER>` | Remove a step by its order number | `delete_routine_step_by_order` | ✅ |

### Implementation Notes ✅

- ✅ The `show` command lists each step with its order number, the referenced action's title, and any duration bounds.
- ✅ `--parallel` sets `is_sequential = false`. `--randomize` sets `allow_randomization = true`.
- ✅ `routines add-step` resolves both the routine (by name/ID prefix) and the action (by title/ID prefix).
- ✅ Step ordering: when no `--position` is given, the new step is appended at `max(step_order) + 1`. When a position is given, existing steps at or after that position are shifted.

### Database Functions Implemented ✅

| Function | Signature | Purpose |
|----------|-----------|---------|
| `insert_routine` | `(conn, routine) -> Result<()>` | ✅ Insert or upsert a routine |
| `fetch_routines` | `(conn) -> Result<Vec<Routine>>` | ✅ Fetch all routines ordered by `created_at DESC` |
| `fetch_routine_by_id` | `(conn, routine_id) -> Result<Option<Routine>>` | ✅ Fetch a single routine |
| `delete_routine` | `(conn, routine_id) -> Result<()>` | ✅ Delete a routine by ID (steps cascade) |
| `insert_routine_step` | `(conn, step) -> Result<()>` | ✅ Insert a routine step |
| `fetch_routine_steps` | `(conn, routine_id) -> Result<Vec<RoutineStep>>` | ✅ Fetch steps for a routine ordered by `step_order` |
| `fetch_routine_step_by_id` | `(conn, step_id) -> Result<Option<RoutineStep>>` | ✅ Fetch a single step |
| `fetch_routine_step_by_order` | `(conn, routine_id, step_order) -> Result<Option<RoutineStep>>` | ✅ Fetch step by order number |
| `delete_routine_step` | `(conn, step_id) -> Result<()>` | ✅ Delete a specific step and re-order remaining steps |
| `delete_routine_step_by_order` | `(conn, routine_id, step_order) -> Result<()>` | ✅ Delete by order number |
| `next_routine_step_order` | `(conn, routine_id) -> Result<i64>` | ✅ Compute the next `step_order` value for a routine |
| `shift_routine_steps` | `(conn, routine_id, from_position, delta) -> Result<()>` | ✅ Shift steps for position insertion |
| `count_routine_steps` | `(conn, routine_id) -> Result<i64>` | ✅ Count steps in a routine |

---

## ✅ 7. Shared Utilities (`resolve.rs`) - COMPLETED

Several modules need to resolve an action, instance, or other entity from a user-provided identifier. Rather than duplicating the resolution logic, extract a shared module.

**Status:** ✅ Implemented with trait-based generic resolver

### Implemented Functions

| Function | Status | Used By |
|----------|--------|---------|
| `resolve_action(conn, identifier) -> Result<Action>` | ✅ | `actions`, `instances` (create), `pipeline` (add), `routines` (add-step), `events` (create --action) |
| `resolve_instance(conn, identifier) -> Result<Instance>` | ✅ | `instances` (show/status/delete), `pipeline` (contextual), `events` (create --instance) |
| `resolve_pipeline_item(conn, pipeline_id, identifier) -> Result<PipelineItem>` | ✅ | `pipeline` (move/remove) |
| `resolve_routine(conn, identifier) -> Result<Routine>` | ✅ | `routines` (show/delete/add-step/remove-step) |

### Implementation Details ✅

- ✅ **`Resolvable` trait** - Generic trait for entities that can be resolved by identifier
- ✅ Implemented for `Action` and `Instance`
- ✅ Resolution pattern:
  1. Try exact ID match
  2. Try ID prefix match (with ambiguity detection)
  3. Try case-insensitive name/title prefix match (with ambiguity detection)
  4. Return clear error messages on zero or multiple matches

---

## 8. Updated `main.rs` Structure

### Current Structure (Phase 2 Complete) ✅
```rust
mod actions;         // ✅
mod context;         // ✅
mod instances;       // ✅
mod mental_states;   // ✅
mod resolve;         // ✅

enum Commands {
    Actions { command: ActionsCommand },
    Instances { command: InstancesCommand },
    Context { command: ContextCommand },           // ✅
    MentalStates { command: MentalStatesCommand }, // ✅
    Pipeline { list_all: bool },  // 🔜 placeholder
}
```

### Target Structure (After All Modules Complete)
```rust
mod actions;
mod context;        // NEW
mod events;
mod instances;
mod mental_states;  // NEW
mod pipeline;
mod resolve;
mod routines;

enum Commands {
    Actions      { command: ActionsCommand },
    Instances    { command: InstancesCommand },
    Context      { command: ContextCommand },      // NEW
    MentalStates { command: MentalStatesCommand }, // NEW
    Pipeline     { command: PipelineCommand },
    Events       { command: EventsCommand },
    Routines     { command: RoutinesCommand },
}
```

Each arm delegates to `handle_<module>_command(command, &conn)`.

---

## Implementation Order & Progress (Phase 5 Complete! 🎉)

### Phase 1: Foundation (Complete) ✅
1. ✅ **`resolve.rs`** — Extract `resolve_action` from `actions.rs`, add `resolve_instance`. This unblocks everything else.
2. ✅ **`instances.rs`** — Most database support already exists.
3. ✅ **Scoring System (database)** — Trait-based extensible scoring framework with 5 basic scorers.

### Phase 2: Context-Aware Intelligence (COMPLETE) ✅
4. ✅ **`context.rs`** — Context snapshot management (DONE)
   - ✅ Database functions for context snapshots
   - ✅ CLI commands: snapshot, current, history, set-energy, set-attention
   - ✅ Foundation for scoring context assembly
5. ✅ **`mental_states.rs`** — Mental state tracking (DONE)
   - ✅ Database functions for mental states
   - ✅ CLI commands: list, create, record, current, history
   - ✅ Integration with context snapshots
   - ✅ Resolve support for mental states
6. ✅ **Scoring Integration** — Connect scoring to context (DONE)
   - ✅ `build_scoring_context()` helper in database
   - ✅ `score_instance_with_context()`, `suggest_best_instances()`
   - ✅ CLI commands: `instances score <id>`

### Phase 3: Pipeline Module (COMPLETE) ✅
7. ✅ **`pipeline.rs`** — Smart queue with recommendations (DONE)
   - ✅ Manual queue commands (list, add, move, remove, normalize)
   - ✅ Smart commands (suggest, refresh, explain)
   - ✅ Integration with scoring system

### Phase 4: Events & Learning (COMPLETE) ✅
8. ✅ **`events.rs`** — Event tracking and pattern analysis (DONE)
   - ✅ Database functions for tracked events and patterns
   - ✅ CLI commands: list, record, patterns
   - ✅ Event type tracking with 6 types
   - ✅ Pattern analysis with completion rates

### Phase 5: Routines (COMPLETE) ✅
9. ✅ **`routines.rs`** — Routine templates (DONE)
   - ✅ Implement database functions (13 new functions)
   - ✅ CLI commands for managing routines (list, create, show, delete, add-step, remove-step)
   - ✅ Sequential/parallel modes with randomization support
   - ✅ Step ordering with position insertion and automatic re-ordering

### Phase 6.1: Routine-Pipeline Integration (COMPLETE) ✅
10. ✅ **Integration & Polish** — Completed:
   - ✅ Integrate routines with pipeline (instantiate routine as a batch of instances)
   - ✅ `instantiate_routine` database function with randomization and position options
   - ✅ `routines start` CLI command to add routine steps to pipeline
   - ✅ Interactive mode routine workflows (create, view, start, add steps, delete)
   - ✅ `fetch_action_by_id` helper function added
   - ✅ 5 new tests for routine instantiation

### Phase 6.2: Interactive Automation (COMPLETE) ✅
11. ✅ **Interactive UX Overhaul** — Completed:
   - ✅ Auto-context detection (time of day, day type from system clock)
   - ✅ Status dashboard shown on every loop iteration (context, energy, attention, mental state, pipeline)
   - ✅ Quick check-in flow (energy + attention with visual selectors, no typing)
   - ✅ "What should I do next?" streamlined flow (auto-context → scoring → start → complete → next)
   - ✅ Quick add to pipeline (single step: title → action + instance + enqueue)
   - ✅ Auto event recording on task completion, skip, and snooze
   - ✅ Post-completion flow (auto-suggest next task after completing one)
   - ✅ Batch mode now auto-enqueues all created tasks to pipeline
   - ✅ Routine creation now offers inline step adding
   - ✅ Routine start now offers to begin first task immediately
   - ✅ Score visualization with bar charts
   - ✅ Completion patterns view in main menu
   - ✅ Pipeline menu: start task, complete with auto-events, skip with auto-events
   - ✅ Explore actions: streamlined to auto-add to pipeline (no intermediate instance step)
   - ✅ Mental state recording: inline "create new" option from selection list

### Phase 7: Advanced Scheduling & Learning (NEXT) 🔥
12. ⏳ **Advanced Features** — Next steps:
   - Add routine scheduling (use default_start_time/default_end_time)
   - Time-bounded instantiation (fit steps within time window)
   - Context-aware routine suggestions
   - Adaptive scoring weights based on completion patterns
   - "Navigate by mental state" feature
   - Recurring task auto-generation

**Current Priority:** Phase 7 advanced scheduling, adaptive learning, and mental-state navigation.

---

## Summary of Database Functions Status

### ✅ Phase 2 - Context & Intelligence (COMPLETED) 🎉

**Phase 2.1: Context Module**
| Module | Function | Priority | Status |
|--------|----------|----------|--------|
| context | `insert_context_snapshot` | ✅ DONE | ✅ Implemented in database/context.rs |
| context | `fetch_context_snapshots` | ✅ DONE | ✅ Implemented in database/context.rs |
| context | `fetch_current_context` | ✅ DONE | ✅ Implemented in database/context.rs |
| context | `update_context_energy` | ✅ DONE | ✅ Implemented in database/context.rs |
| context | `update_context_attention` | ✅ DONE | ✅ Implemented in database/context.rs |
| mental_states | `insert_mental_state` | ✅ DONE | ✅ Implemented in database/context.rs |
| mental_states | `fetch_mental_states` | ✅ DONE | ✅ Implemented in database/context.rs |
| mental_states | `insert_mental_state_event` | ✅ DONE | ✅ Implemented in database/context.rs |
| mental_states | `fetch_mental_state_events` | ✅ DONE | ✅ Implemented in database/context.rs |
| mental_states | `fetch_current_mental_state` | ✅ DONE | ✅ Implemented in database/context.rs |
| scoring | `build_scoring_context` | 🔥 URGENT | ⏳ Required for scoring integration (NEXT STEP) |
| scoring | `score_pipeline_items` | 🔥 HIGH | ⏳ Required for smart pipeline |
| scoring | `suggest_best_instances` | 🔥 HIGH | ⏳ Required for smart pipeline |
| scoring | `score_instance_detailed` | HIGH | ⏳ Required for pipeline explain |

### ✅ Phase 3 - Pipeline Module (COMPLETED) 🎉

**Database Functions:**
- ✅ `score_pipeline_items(conn, pipeline_id) -> Result<Vec<(PipelineItem, f64)>>`

**CLI Module (pipeline.rs):**
- ✅ `pipeline suggest [--count N]` - Smart task recommendations
- ✅ `pipeline refresh` - Re-score and re-order by score
- ✅ `pipeline explain <identifier>` - Detailed scoring breakdown
- ✅ `pipeline list [--scored]` - Show pipeline items with optional scores
- ✅ `pipeline add <action> [--position N]` - Add to pipeline
- ✅ `pipeline move <identifier> --position N` - Reposition items
- ✅ `pipeline remove <identifier>` - Remove from pipeline
- ✅ `pipeline normalize` - Fix position gaps

**Resolve Support:**
- ✅ `resolve_pipeline_item(conn, identifier)`
- ✅ `resolve_pipeline_item_in(conn, pipeline_id, identifier)`

### ✅ Phase 5 - Routines (COMPLETED) 🎉

| Module | Function | Status |
|--------|----------|--------|
| routine | `insert_routine` | ✅ Implemented |
| routine | `fetch_routines` | ✅ Implemented |
| routine | `fetch_routine_by_id` | ✅ Implemented |
| routine | `delete_routine` | ✅ Implemented |
| routine | `insert_routine_step` | ✅ Implemented |
| routine | `fetch_routine_steps` | ✅ Implemented |
| routine | `fetch_routine_step_by_id` | ✅ Implemented |
| routine | `fetch_routine_step_by_order` | ✅ Implemented |
| routine | `delete_routine_step` | ✅ Implemented |
| routine | `delete_routine_step_by_order` | ✅ Implemented |
| routine | `next_routine_step_order` | ✅ Implemented |
| routine | `shift_routine_steps` | ✅ Implemented |
| routine | `count_routine_steps` | ✅ Implemented |

**Resolve Support:**
- ✅ `resolve_routine(conn, identifier)` - Implemented in CLI resolve.rs

### ✅ Phase 6.1 - Routine-Pipeline Integration (COMPLETED) 🎉

### ✅ Phase 6.2 - Interactive Automation (COMPLETED) 🎉

| Module | Feature | Status |
|--------|---------|--------|
| interactive | Auto-context detection (time/day) | ✅ Implemented |
| interactive | Status dashboard | ✅ Implemented |
| interactive | Quick check-in (energy/attention) | ✅ Implemented |
| interactive | "What should I do next?" flow | ✅ Implemented |
| interactive | Quick add to pipeline | ✅ Implemented |
| interactive | Auto event recording | ✅ Implemented |
| interactive | Post-completion flow | ✅ Implemented |
| interactive | Score visualization bars | ✅ Implemented |
| interactive | Completion patterns view | ✅ Implemented |
| interactive | Batch mode auto-enqueue | ✅ Implemented |
| interactive | Routine inline step adding | ✅ Implemented |
| interactive | Routine immediate task start | ✅ Implemented |


| Module | Function | Status |
|--------|----------|--------|
| routine | `instantiate_routine` | ✅ Implemented |
| routine | `instantiate_routine_by_id` | ✅ Implemented |
| routine | `RoutineInstantiationResult` | ✅ Implemented |
| routine | `InstantiateRoutineOptions` | ✅ Implemented |
| action | `fetch_action_by_id` | ✅ Implemented |

**CLI Commands:**
- ✅ `routines start <identifier> [--randomize] [--sequential] [--position N]`

**Interactive Mode:**
- ✅ Routine workflows menu added (start, view, create, add steps, delete)

### Optional / Future
| Module | Function | Priority | Status |
|--------|----------|----------|--------|
| instance | `fetch_instance_by_id` | Optional | ⏳ Not needed yet |
| pipeline | `fetch_pipeline_item_by_id` | Optional | ⏳ Not needed yet |
| routine | Time-bounded instantiation | Future | ⏳ Phase 7 |
| routine | Context-aware suggestions | Future | ⏳ Phase 7 |
| scoring | Adaptive weight learning | Future | ⏳ Phase 7 |
| mental_states | Navigate by mental state | Future | ⏳ Phase 7 |

**Progress:** All core modules complete! 🎉
- ✅ **Context/Mental State modules:** Complete (Phase 2)
- ✅ **Smart Pipeline module:** Complete (Phase 3)
- ✅ **Events module:** Complete (Phase 4)
- ✅ **Routines module:** Complete (Phase 5) - 13 database functions, 6 CLI commands
- ✅ **Routine-Pipeline Integration:** Complete (Phase 6.1) - 5 new functions, 1 CLI command, interactive workflows
- ✅ **Interactive Automation:** Complete (Phase 6.2) - Auto-context, status dashboard, streamlined flows, auto-events

---

## Architecture Notes

**Key Insight:** Subroutine is not just a task manager - it's a **context-aware action recommender** that acts as an "executive function prosthetic". The scoring system (✅ complete) provides the intelligence, but it needs:

1. **Context capture** - What's true about the user/environment right now
2. **Mental state tracking** - Navigate by mental state, adapt suggestions
3. **Integration** - Connect scoring to pipeline for smart recommendations

See `ARCHITECTURE_GAPS_ANALYSIS.md` for full details on missing features and `SCORING_ARCHITECTURE.md` for scoring system design.