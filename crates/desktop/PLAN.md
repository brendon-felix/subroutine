# Desktop Migration Plan

This document describes the work needed to bring `crates/desktop` from its original, non-compiling state (built against an obsolete `database` crate) into a working GUI that uses the current `app-core` and `database` crates.

---

## Background: What Broke and Why

The desktop crate was moved from `src/` without modification. That original code was written against an older version of the `database` crate that no longer exists. The old database crate had fundamentally different types:

| Old type (deleted) | New equivalent |
|---|---|
| `database::ActionModel` (action template) | `app_core::SavedAction` / `database::SavedActionModel` (internal) |
| `database::Instance` (concrete pipeline entry) | `app_core::Action` / `database::ActionModel` (internal) |
| `database::PipelineItem` (position metadata) | `app_core::PipelineEntry` + `app_core::Pipeline` |
| `database::ContextSnapshot` | `app_core::Context` (computed fresh, never stored) |
| `database::MentalState` (energy+attention f64 pair) | `app_core::MentalState` (spoons + 4 declared axes) |
| `database::MentalStateEvent` | removed |
| `database::Routine` (with `is_sequential` field) | `app_core::Routine` (steps-based, no sequential flag yet) |
| `database::RoutineStep` | `app_core::SavedStep` (enum: `Action(Uuid)` \| `Event(Uuid)`) |
| `database::ScoredInstance` | `app_core::ScoreBreakdown` computed by `app_core::score()` |
| `database::DEFAULT_PIPELINE_ID` | removed (no pipeline ID concept) |
| `database::InstantiateRoutineOptions` | removed (routine instantiation is a method on `Routine`) |
| `database::ensure_default_pipeline()` | removed |
| `database::fetch_pipeline_items()` | `database::load_pipeline()` |
| `database::fetch_instances()` | removed |
| `database::insert_event()` | removed (event logging not yet in new schema) |

Additionally, `task_store.rs` and `auth.rs` depend on a `ticks` crate (TickTick API client) that has nothing to do with the new architecture and should be deleted.

---

## New Architecture Summary

The new stack works as follows:

- **`app_core`** owns all rich in-memory types (`SavedAction`, `Action`, `Routine`, `Pipeline`, `MentalState`, `SavedMentalState`, etc.) and all business logic (scoring, constraint checking, recurrence scheduling).
- **`database`** handles persistence only. It converts between `app_core` types and flat SQLite rows. Callers use `database::connect_and_migrate()` to get a `DatabaseConnection`, then call functions like `insert_saved_action`, `fetch_saved_actions`, `load_pipeline`, `save_pipeline`, etc.
- **`desktop/stores/database_store.rs`** is the GPUI `Entity` that bridges the background SQLite thread with the foreground UI. It owns the live `Pipeline`, the `SavedAction` list, the `MentalState`, etc., and fires GPUI events when state changes.
- **Views** subscribe to `DatabaseStore` events and read state via the store's getter methods.

---

## Step 1 — Clean Up Dead Code

### Delete files
- `src/stores/task_store.rs` — entirely dead (TickTick API, never used)
- `src/auth.rs` — only existed to serve `task_store.rs`
- `src/tasks.rs` — same; already `// mod tasks;` commented out in `main.rs`

### Update `src/main.rs`
Remove the `mod auth;` declaration (already removed for `tasks`).

### Update `src/stores/mod.rs`
Remove the commented-out `// pub mod task_store;` and `// pub use task_store::TaskStore;` lines and the now-gone `pub use database_store::DatabaseStore;` import is fine to keep.

### Update `Cargo.toml`
- Remove: `axum`, `im` (only used by old task_store), `tokio` (only needed for TickTick OAuth server), `rusqlite` (direct usage no longer needed — `database` crate exposes `DatabaseConnection`).
- Add: `app-core = { workspace = true }` (types and scoring live here now).
- Keep: `database = { workspace = true }`, `anyhow`, `gpui`, `gpui-component`, `gpui_transitions`, `chrono`, `tracing`, `serde`, `serde_json`, `dirs`, `unicode-segmentation`, `smallvec`, `rust-embed`, `smol`, `regex`, `fuzzy-matcher`, `open`.
- Add `uuid = { workspace = true }` (needed for `Uuid` in store methods).

---

## Step 2 — Rewrite `DatabaseStore`

This is the most significant change. The entire `database_store.rs` file needs to be replaced. The old 1,200-line file imported types that no longer exist.

### New struct layout

```rust
pub struct DatabaseStore {
    conn: Option<DatabaseConnection>,

    // Cached app-core state (kept in sync with DB)
    pipeline: Pipeline,
    saved_actions: Vec<SavedAction>,
    routines: Vec<Routine>,
    saved_mental_states: Vec<SavedMentalState>,
    mental_state: MentalState,

    // In-flight async tasks (stored to prevent cancellation)
    initialize_task: Option<Task<()>>,
    save_pipeline_task: Option<Task<()>>,
}
```

`Pipeline`, `SavedAction`, `Routine`, `SavedMentalState`, `MentalState` all come from `app_core`.

### Events to emit

```rust
pub struct DatabaseError { pub message: String }
pub struct PipelineChanged;          // pipeline loaded or mutated
pub struct SavedActionsLoaded;       // saved_actions cache refreshed
pub struct RoutinesLoaded;           // routines cache refreshed
pub struct SavedMentalStatesLoaded;  // saved_mental_states refreshed
pub struct MentalStateChanged;       // mental_state updated
```

Remove old events: `ActionsLoaded`, `PipelineLoaded`, `ContextLoaded`, `PipelineScored`, `SuggestionsLoaded`, `RoutineStepsLoaded`.

### `initialize(cx)`

```
background: connect_and_migrate()
  → foreground: store conn, load_saved_actions, load_pipeline, load_routines, load_saved_mental_states
```

Spoons start at `MAX_SPOONS`; declared state starts as `None`. No separate "load context" or "load mental state" round-trip — `MentalState` is pure in-memory state derived from usage, seeded at startup.

### Key methods (public API used by views)

**Saved actions:**
- `load_saved_actions(cx)` — background `fetch_saved_actions`, then `self.saved_actions = …; emit SavedActionsLoaded`
- `get_saved_actions() -> &Vec<SavedAction>`
- `get_saved_action(id: Uuid) -> Option<&SavedAction>`
- `upsert_saved_action(saved: SavedAction, cx)` — background `insert_saved_action`, then reload
- `delete_saved_action(id: Uuid, cx)` — background delete, then reload

**Pipeline:**
- `load_pipeline(cx)` — background `load_pipeline`, then `self.pipeline = …; emit PipelineChanged`
- `get_pipeline() -> &Pipeline`
- `create_action(title: String, cx)` — creates `SavedAction` + instantiates `Action`, pushes to backlog, persists both, emits `PipelineChanged`
- `create_ephemeral_action(title: String, cx)` — creates ephemeral `Action` (no `SavedAction`), pushes to backlog
- `promote(id: Uuid, cx)` — `pipeline.promote(id)?`, persist, emit `PipelineChanged`
- `demote(id: Uuid, cx)` — `pipeline.demote(id)?`, persist, emit `PipelineChanged`
- `remove_from_pipeline(id: Uuid, cx)` — remove entry from whichever list it's in, persist, emit `PipelineChanged`
- `complete_action(id: Uuid, cx)` — find `Action` in pipeline, call `mental_state.complete_action(&action)`, remove from pipeline, delete the concrete `Action` row in DB, persist pipeline, emit `PipelineChanged` + `MentalStateChanged`
- `refresh_pipeline(cx)` — builds `Context` from current `mental_state`, calls `pipeline.refresh(&context, &completed_ids)`, persists, emits `PipelineChanged`
- `save_pipeline(cx)` — background `save_pipeline`, no event

**Mental state:**
- `load_saved_mental_states(cx)` — background `fetch_saved_mental_states`, emits `SavedMentalStatesLoaded`
- `get_saved_mental_states() -> &Vec<SavedMentalState>`
- `get_mental_state() -> &MentalState`
- `declare_mental_state(id: Uuid, cx)` — find saved state by id, set `mental_state.declared = Some(state)`, emit `MentalStateChanged`; optionally trigger `refresh_pipeline`
- `clear_declared_state(cx)` — set `mental_state.declared = None`, emit `MentalStateChanged`
- `upsert_saved_mental_state(state: SavedMentalState, cx)` — background insert, reload
- `delete_saved_mental_state(id: Uuid, cx)` — background delete; if it was the declared state, clear it first

**Routines:**
- `load_routines(cx)` — background `fetch_routines`, emits `RoutinesLoaded`
- `get_routines() -> &Vec<Routine>`
- `get_routine(id: Uuid) -> Option<&Routine>`
- `upsert_routine(routine: Routine, cx)` — background insert, reload
- `delete_routine(id: Uuid, cx)` — remove from DB and pipeline if present
- `activate_routine(id: Uuid, cx)` — fetch saved_actions map, call `routine.instantiate(…)`, remove routine placeholder from pipeline, push resulting entries to backlog, persist, emit `PipelineChanged`

**Scoring helper (used by views):**
- `score_entry(entry: &PipelineEntry) -> f32` — builds a `Context`, calls `app_core::score(entry, &context, &HashSet::new()).total`

### The `db_op!` macro

The existing macro pattern is sound and should be kept. It runs a closure on the background executor and then calls an on-success closure on the foreground. The only change is that closures now work with the new database functions (`database::insert_saved_action`, `database::load_pipeline`, etc.) instead of the deleted old ones.

---

## Step 3 — Rewrite `FocusView`

**Old:** held `Vec<(Instance, ActionModel, f64)>` driven by a `SuggestionsLoaded` event from `store.suggest_next(3, cx)`.

**New:** reads `store.get_pipeline().queue()` directly and takes the top N entries by score.

### Changes
- Remove the `suggestions` field; instead, read the queue on render using `store.get_pipeline().queue()`.
- Subscribe to `PipelineChanged` to call `cx.notify()`.
- The "score" to display beside each item is computed via `store.score_entry(entry)`.
- Actions on a selected entry:
  - **Complete** → `store.complete_action(entry_id, cx)` (for `PipelineEntry::Action` entries)
  - **Demote / Skip** → `store.demote(entry_id, cx)`
  - **Promote Routine** (for `PipelineEntry::Routine`) → `store.activate_routine(id, cx)`
- Remove `start_instance`, `skip_instance`, `snooze_instance`, `abandon_instance` concepts — no "Instance" type exists.
- The `refresh_suggestions` method becomes `refresh_pipeline` which just calls `store.refresh_pipeline(cx)`.

---

## Step 4 — Rewrite `ActionListView`

**Old:** displayed `Vec<ActionModel>` (old action templates). Subscribed to `ActionsLoaded`.

**New:** displays `Vec<SavedAction>`. Subscribed to `SavedActionsLoaded`.

### Changes
- Replace `ActionModel` with `app_core::SavedAction` throughout.
- `ActionListDelegate.actions` is `Vec<SavedAction>`.
- `update_actions` reads `store.get_saved_actions().clone()`.
- The "add to pipeline" affordance: a button per row calls `store.create_action(title, cx)` (which creates a `SavedAction` + `Action` and pushes to backlog). Alternatively, instantiation can be a separate "Add to pipeline" action that calls `saved.instantiate()` and then `store.promote(…)`.
- The create-new-action flow (previously `TaskCreator`) creates a `SavedAction` via `store.upsert_saved_action(…, cx)`. Opening the editor for an existing action passes its `Uuid` to `ActionEditor`.
- Search filtering is unchanged in structure; just filter on `saved.title` instead of `action.title`.

---

## Step 5 — Rewrite `ActionEditor`

**Old:** edited old `ActionModel` fields including `action_type` (a category enum that no longer exists), `enjoyment_after_start` (dropped from `app_core`), and `urgency_growth` (not in `app_core`).

**New:** edits `app_core::SavedAction` fields.

### Field mapping

| Old ActionEditor field | New `SavedAction` field | Notes |
|---|---|---|
| `pending_title` | `saved.title` | unchanged |
| `pending_description` | `saved.content` | unchanged |
| `action_type` | **remove** | no type/category in new model |
| `duration_bucket` | `saved.constraints.minimum_duration` | stored as `Option<Duration>` |
| `energy_rate` | `saved.context.energy_rate: Option<i8>` | range −2..+2 |
| `attention_level` | `saved.context.attention_level: Option<u8>` | range 1..5 |
| `transition_difficulty` | `saved.context.transition_difficulty: Option<u8>` | range 1..5 |
| `enjoyment_after_start` | **remove** | not in `app_core::ActionContext` |
| `importance` | `saved.context.importance: Option<u8>` | range 1..5 |
| `urgency_growth` | **remove** | not in `app_core` |
| `preferred_times` | `saved.constraints.valid_times_of_day: Option<TimesOfDay>` | bitflags |
| _(none)_ | `saved.target_time: Option<NaiveTime>` | preferred time-of-day |
| _(none)_ | `saved.constraints.deadline: Option<NaiveTime>` | time-of-day deadline |

### `load_action` method
Signature changes from `load_action(&action_id: &str, cx)` to `load_action(id: Uuid, cx)`. Reads from `store.get_saved_action(id)` instead of calling a DB query.

### `save_action` method
Calls `store.upsert_saved_action(saved_action, cx)`.

### `delete_action` method
Calls `store.delete_saved_action(id, cx)`.

### Subscription change
Subscribe to `SavedActionsLoaded` instead of `ActionsLoaded`.

---

## Step 6 — Rewrite `RightSidebarView`

**Old:** two 1–5 sliders (energy, attention) that persisted a `ContextSnapshot`. Showed a "current mental state name" string.

**New:** displays the live `MentalState` (spoons + declared state axes) and lets the user pick a `SavedMentalState` to declare.

### Remove
- `energy_slider`, `attention_slider`, `energy_value`, `attention_value` fields.
- `ContextLoaded` subscription and `update_energy`/`update_attention` store calls.
- The "suggest next" button (Focus Mode now reads queue directly).

### Add
- Subscribe to `MentalStateChanged` and `SavedMentalStatesLoaded`.
- Render a spoon gauge (current `remaining_spoons` out of `MAX_SPOONS`).
- Render the declared state name if set (same pill display as before), with a clear button.
- Render a row of buttons for each `SavedMentalState` in `store.get_saved_mental_states()`, which call `store.declare_mental_state(id, cx)` when clicked. Use the starter state set (8 entries) as the default; users can customize in a future settings screen.
- Keep the pipeline list below the mental state section.
- Keep the "refresh pipeline" button (now calls `store.refresh_pipeline(cx)` directly).

### Slider removal impact
The `components/slider.rs` wrapper may no longer be needed. If nothing else uses it, delete it.

---

## Step 7 — Update `Pipeline` View

**Old:** displayed `Vec<(PipelineItem, Instance)>` pairs with drag-drop reordering. Subscribed to `PipelineLoaded`, `PipelineScored`, `ActionsLoaded`, `ContextLoaded`, `MentalStatesLoaded`.

**New:** displays `PipelineEntry` values from `store.get_pipeline()`.

### Changes
- Subscribe only to `PipelineChanged`.
- `update_items` reads `store.get_pipeline().queue()` (and optionally `backlog()`).
- Each item renders `entry.title()` and a score via `store.score_entry(entry)`.
- The drag-drop reorder logic needs to work with `Uuid` identifiers and call `store.promote(id)` / `store.demote(id)` / future `store.reorder_queue(from, to)` (not yet in scope — V1 can drop manual reordering and rely on score-based order).
- Completing an action calls `store.complete_action(id, cx)`.
- For `PipelineEntry::Routine` entries, show an "Activate" button that calls `store.activate_routine(id, cx)`.
- Remove the `scores: Vec<(String, f64)>` field — compute scores on render via `store.score_entry(entry)`.

---

## Step 8 — Update `RoutinesView` and `RoutineEditor`

**Old:** used `database::Routine` (with `is_sequential: bool`, `allow_randomization: bool`) and `database::RoutineStep`.

**New:** uses `app_core::Routine` (with `steps: Vec<SavedStep>`) and `app_core::SavedStep`.

### `RoutinesView` changes
- Replace `database::Routine` with `app_core::Routine`.
- Subscribe to `RoutinesLoaded` (same event name).
- Remove the `mode_label` helper (`is_sequential` no longer exists on `Routine`).
- Each routine row shows title and step count (`routine.steps.len()`).
- "Activate" button calls `store.activate_routine(routine.id, cx)`.

### `RoutineEditor` changes
- Replace all `database::Routine` / `database::RoutineStep` with `app_core::Routine` / `app_core::SavedStep`.
- `steps: Vec<SavedStep>` (each step is `SavedStep::Action(Uuid)` or `SavedStep::Event(Uuid)`).
- Remove `is_sequential` and `allow_randomization` toggles (not in `app_core::Routine` yet; add when the field is added).
- When adding a step, the available items come from `store.get_saved_actions()` (and eventually `store.get_saved_events()`). Each chosen action creates a `SavedStep::Action(saved.id)`.
- `save_new_routine` calls `store.upsert_routine(routine, cx)`.
- `RoutineStepsLoaded` event is gone — steps are embedded in `Routine` now.

---

## Step 9 — Update `MainView` Home Screen

**Old:** "I'm feeling…" buttons called `store.record_mental_state("analysis paralysis", ...)` with a string name.

**New:** buttons should call `store.declare_mental_state(id, cx)` using the hardcoded starter state UUIDs from `app_core::mental_state::starter_states`.

### Mapping

| Old button label | Starter state UUID constant |
|---|---|
| "analysis paralysis" | `starter_states::SCATTERED_ID` |
| "overstimulated" | `starter_states::OVERWHELMED_ID` |
| "hyperfocused" | `starter_states::FOCUSED_ID` |
| "an intense emotion" | `starter_states::OVERWHELMED_ID` (closest available) |

A future V2 pass should replace these hardcoded buttons with a dynamic list from `store.get_saved_mental_states()`.

---

## Step 10 — Update `TaskCreator` Component

The `TaskCreator` component creates new actions. It uses the old `insert_action` flow.

### Changes
- Rename conceptually to "ActionCreator" (the file can keep the name to avoid churn, but update internal struct names in a follow-up).
- On submit, call `store.create_action(title, cx)` which creates a `SavedAction` + `Action` and adds to the pipeline backlog.
- Remove fields that mapped to old `ActionModel` (category, project, etc.) — the new form is just a title input to start.
- Follow-up: after saving, optionally open `ActionEditor` to fill in context attributes (energy, importance, etc.).

---

## Step 11 — Custom Components Audit

### `components/custom_list/`
This is a from-scratch `List` + `ListDelegate` + `ListState` implementation. It predates `gpui-component`'s `List` component. Both can coexist for now, but consider migrating to `gpui_component::list::List` in a follow-up to reduce maintenance burden. No type changes required to keep using it.

### `components/drag_drop/`
The drag-drop store (`DragDropStore`) and `ActionLocation` enum reference old concepts. The `ActionLocation` enum has variants that mapped to old types. It should be updated to reference `Uuid` for pipeline entries. However, manual reordering is V2 scope — for V1, the drag-drop component can be kept but its active code paths temporarily disabled while the pipeline uses score-based ordering only.

### `components/command_palette/`
Mostly type-agnostic. The command palette actions that reference old store methods (`suggest_next`, `score_pipeline`, etc.) need to be updated to use new store methods (`refresh_pipeline`, `declare_mental_state`, etc.).

### `components/slider.rs`
Only used by `RightSidebarView` for energy/attention sliders, which are being removed. Check if anything else uses it; if not, delete it.

### `components/checkbox.rs`, `components/popover.rs`, `components/resizable/`
Type-agnostic. No changes needed.

---

## Step 12 — Cargo.toml Final State

```toml
[package]
name = "desktop"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = { workspace = true }
futures = { workspace = true }
gpui = { git = "https://github.com/zed-industries/zed" }
gpui-component = { git = "https://github.com/longbridge/gpui-component" }
gpui_transitions = { path = "/Users/felixb/Projects/gpui_transitions" }
open = { workspace = true }
app-core = { workspace = true }
database = { workspace = true }
uuid = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
dirs = { workspace = true }
chrono = { workspace = true }
unicode-segmentation = { workspace = true }
schemars = { workspace = true }
smol = { workspace = true }
smallvec = { workspace = true }
rust-embed = { workspace = true }
tracing = { workspace = true }
regex = { workspace = true }
fuzzy-matcher = { workspace = true }
```

Note: `app-core` must also be added to `[workspace.dependencies]` in the root `Cargo.toml`:
```toml
app-core = { path = "crates/app-core" }
```

---

## Implementation Order

Work through steps in this order to minimize the number of compile errors that block verifying progress:

1. **Step 1** (dead code) — gets rid of the `ticks`/`axum`/TickTick noise immediately.
2. **Step 2** (DatabaseStore rewrite) — everything else depends on this compiling.
3. **Step 12** (Cargo.toml) — needed before Step 2 can compile.
4. **Step 6** (RightSidebarView) — depends only on DatabaseStore, relatively self-contained.
5. **Step 4** (ActionListView) — depends on DatabaseStore, relatively self-contained.
6. **Step 5** (ActionEditor) — depends on ActionListView and DatabaseStore.
7. **Step 3** (FocusView) — depends on DatabaseStore.
8. **Step 7** (Pipeline view) — depends on DatabaseStore.
9. **Step 8** (RoutinesView + RoutineEditor) — depends on DatabaseStore.
10. **Step 9** (MainView home) — small change, last.
11. **Step 10** (TaskCreator) — polish pass.
12. **Step 11** (component audit) — cleanup pass.

---

## Gaps and Deferred Items

These items are noted but not blocking for V1:

- **`app_core::Routine` has no `is_sequential` or `allow_randomization` fields.** The design calls for them but they are not yet in `app_core`. Leave them out of the editor for now; add them when `app_core` is updated.
- **Manual pipeline reordering** (drag-drop). Score-based ordering is sufficient for V1. The `DragDropStore` can stay in the codebase but its UI integration is disabled until V2.
- **`SavedEvent` / `Event` types** are fully implemented in `app_core` and `database` but no UI exists yet. The `ActionListView` and `ActionEditor` only surface `SavedAction` for now.
- **Spoon recovery timer.** `MentalState::recover_spoons(elapsed)` exists in `app_core` but no periodic timer drives it yet. In V1, spoons only decrease on completion and are reset by declaring a mental state. Add a `cx.spawn` timer loop in `DatabaseStore::initialize` in V2.
- **Completed action tracking for constraint dependency checks.** `refresh_pipeline` currently passes an empty `HashSet` for `completed_ids`. A persistent completed-IDs set needs to be loaded from the DB (or derived from the `actions` table with a `completed_at` column) in V2.
- **`left_sidebar_view.rs` and `task_list_view.rs`** are currently excluded from `views/mod.rs`. They can be deleted in the cleanup pass.