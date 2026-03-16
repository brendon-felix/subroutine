# desktop Design

This document captures the design decisions and conventions for the `desktop` crate.

---

## Purpose

The `desktop` crate is the GPUI-based macOS application. It owns the UI layer — views, components, stores, and application lifecycle. It depends on `app-core` for domain types and business logic, and on `database` for persistence. Neither `app-core` nor `database` has any knowledge of GPUI or the UI.

---

## Crate Architecture

### Dependency Direction

```
desktop  →  app-core  (domain types, scoring, pipeline logic)
desktop  →  database  (persistence)
database →  app-core  (converts to/from app-core types)
app-core has NO knowledge of persistence or UI
```

Never reach across this boundary — `app-core` must not import `database`, and `database` must not import `desktop`.

### Module Structure

| Module | Responsibility |
|---|---|
| `main.rs` | Application entry point — creates the GPUI app, initializes components, opens the window |
| `app.rs` | Application-level setup helpers |
| `assets.rs` | Embedded assets (icons, fonts) via `rust-embed` |
| `themes.rs` | Theme registration and customization |
| `transitions.rs` | Shared animation transition helpers |
| `stores/` | In-memory state stores (GPUI entities wrapping cached DB state) |
| `views/` | Full-screen and panel views composing the application layout |
| `components/` | Reusable UI components that are not full views |

---

## Stores

Stores are GPUI `Entity<T>` types that own the in-memory cache and expose the high-level API used by all views. They bridge the gap between the async database layer and the synchronous GPUI rendering loop.

### `DatabaseStore`

`DatabaseStore` is the central store for all persisted state. It is defined in `stores/database_store.rs` and passed to every view that needs data access. Views never call `database` functions directly — they always go through `DatabaseStore`.

#### Events Emitted

| Event | Meaning |
|---|---|
| `PipelineChanged` | Queue/backlog changed — pipeline views should re-render |
| `SavedActionsLoaded` | `saved_actions` cache refreshed — action list views should update |
| `RoutinesLoaded` | `routines` cache refreshed |
| `SavedMentalStatesLoaded` | `saved_mental_states` cache refreshed |
| `MentalStateChanged` | Live `MentalState` changed — sidebar/focus view should update |
| `DatabaseError { message }` | A background DB task failed |

#### Key Methods

```rust
// Read cached data (only needs &App / &Context<T>):
store.get_saved_actions() -> &Vec<SavedAction>
store.get_saved_action(id: Uuid) -> Option<&SavedAction>
store.get_pipeline() -> &Pipeline
store.get_mental_state() -> &MentalState
store.get_saved_mental_states() -> &Vec<SavedMentalState>
store.get_routines() -> &Vec<Routine>

// Actions:
store.upsert_saved_action(saved: SavedAction, cx)
store.delete_saved_action(id: Uuid, cx)
store.create_action(title: String, cx)                  // creates SavedAction + Action, pushes to pipeline
store.create_action_from_saved(saved: SavedAction, cx)  // instantiates from template, preserving all attributes
store.create_ephemeral_action(title: String, cx)        // no SavedAction, just a concrete Action

// Pipeline:
store.promote(id: Uuid, cx)                 // backlog → queue
store.demote(id: Uuid, cx)                  // queue → backlog
store.remove_from_pipeline(id: Uuid, cx)
store.complete_action(id: Uuid, cx)         // mark done, remove from pipeline
store.refresh_pipeline(cx)                  // re-score and re-order queue
store.score_entry(entry: &PipelineEntry) -> f32  // 0.0–1.0 fit score

// Mental state:
store.declare_mental_state(id: Uuid, cx)    // set active SavedMentalState by ID
store.clear_declared_state(cx)              // revert to neutral

// Routines:
store.upsert_routine(routine: Routine, cx)
store.delete_routine(id: Uuid, cx)
store.activate_routine(id: Uuid, cx)        // instantiate routine into pipeline
```

#### Async Pattern: `db_op!` Macro

Database mutations follow a consistent pattern: spawn a background task that performs the DB operation, then back on the foreground thread update the cache and emit the appropriate event. The `db_op!` macro encapsulates this so that mutation methods stay concise. Background failures are surfaced by emitting `DatabaseError`.

Pipeline saves are debounced — a running save task is cancelled and replaced when new changes arrive, preventing redundant writes.

### `DragDropStore`

`DragDropStore` manages in-flight drag-and-drop state for the pipeline view. It tracks the currently dragged `PipelineEntry` and provides it to drop targets. It is defined in `stores/drag_drop_store.rs`.

---

## Views

Views are GPUI `Entity<T>` types that implement `Render`. They subscribe to `DatabaseStore` events to stay in sync and call `DatabaseStore` methods to mutate state. Views never hold mutable copies of domain data — the canonical copy always lives in `DatabaseStore`.

### Layout Overview

The application uses a three-column layout managed by `RootView`:

```
┌─────────────┬──────────────────────┬─────────────┐
│ Left Sidebar│     Main Content     │ Right Sidebar│
│             │  (mode-switched)     │             │
└─────────────┴──────────────────────┴─────────────┘
```

The main content area is mode-switched by `MainView` based on `MainViewMode`.

### View Inventory

| View | Description |
|---|---|
| `RootView` | Application root — owns `DatabaseStore`, `MainView`, sidebars, and the overlay slot. Handles `CloseOverlay`. |
| `MainView` | Mode-switching content area. Renders one of: Home, Focus, ActionList, Routines, Test. |
| `FocusView` | Focus Mode — shows 1–3 prioritized actions from the pipeline queue. |
| `ActionListView` | Full action list — displays and manages the pipeline backlog and queue. |
| `RoutinesView` | Routines list — browse, create, and activate routines. |
| `LeftSidebarView` | Left sidebar — navigation and mental state declaration. |
| `RightSidebarView` | Right sidebar — current mental state display and spoon level. |
| `PipelineView` | Inline pipeline view embedded in the action list. |
| `ActionEditor` | Overlay — create or edit a `SavedAction` and its attributes. |
| `TaskListView` | (Deferred) Task list view — not currently active. |
| `TestView` | Developer test harness — only reachable from the Home screen. |

### `MainViewMode`

```rust
pub enum MainViewMode {
    Home,
    Focus,
    ActionList,
    Routines,
    Test,
}
```

Navigation between modes is driven by events emitted from child views. `MainView` subscribes to these events and calls `set_mode(...)`.

---

## Overlay System

Overlays (command palette, task creator, action editor, routine editor) are managed by `RootView` via its `overlay: Option<CurrentOverlay>` field. Only one overlay is shown at a time.

### `CurrentOverlay` Enum

```rust
pub enum CurrentOverlay {
    CommandPalette(Entity<CommandPalette>),
    TaskCreator(Entity<TaskCreator>),
    ActionEditor(Entity<ActionEditor>),
    RoutineEditor(Entity<RoutineEditor>),
}
```

### Opening an Overlay

1. Create the entity in an `open_*` method on `RootView`.
2. Set `self.overlay = Some(CurrentOverlay::MyOverlay(entity))`.
3. Add a match arm in `RootView::render`'s `when_some(self.overlay, ...)` block.

### Closing an Overlay

The `CloseOverlay` action handler on `RootView` sets `self.overlay = None` and calls `cx.focus_self(window)` to return focus to the root. Overlay views close themselves by dispatching `CloseOverlay`:

```rust
window.dispatch_action(Box::new(crate::components::popover::CloseOverlay), cx);
```

### Overlay Shell Pattern

Every overlay `Render` impl must follow this pattern to get the dimmed backdrop, correct focus chain, and escape-to-close behaviour:

1. **Call `popover(inner, cx)` as the outermost wrapper.** This adds the absolute-positioned dimmed backdrop, sets `.key_context("Overlay")`, and dispatches `CloseOverlay` on backdrop click.
2. **Put `.track_focus(&self.focus_handle)` on the inner card element.** Without this, the `"Overlay"` key context is never part of the active focus chain, so `escape` → `CloseOverlay` never fires.
3. **Put `.on_any_mouse_down(|_,_,cx| cx.stop_propagation())` on the inner card.** This prevents clicks inside the card from bubbling up to the backdrop's `on_mouse_down`.

Example skeleton:

```rust
impl Render for MyOverlay {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let inner = v_flex()
            .track_focus(&self.focus_handle)
            .on_any_mouse_down(|_event, _window, cx| {
                cx.stop_propagation();
            })
            .child(/* content */);

        popover(inner, cx)
    }
}
```

The `escape` → `CloseOverlay` binding is registered in `popover::init(cx)` with key context `"Overlay"`. Both only work when `.track_focus()` is present on the inner element, making the overlay part of the active focus chain.

---

## Event-Driven Navigation

Views communicate with their parents through emitted GPUI events rather than direct calls or shared mutable state. This keeps the dependency graph acyclic within the view layer.

### Key Navigation Events

| Event | Emitted by | Handled by |
|---|---|---|
| `NavigateToView { mode }` | `ActionListView` | `MainView` → calls `set_mode` |
| `NavigateFromFocus { mode }` | `FocusView` | `MainView` → calls `set_mode` |
| `NavigateFromRoutines { mode }` | `RoutinesView` | `MainView` → calls `set_mode` |
| `StartActionEditor { action_id }` | `ActionListView`, `RootView` | `RootView` → calls `open_action_editor` |
| `StartRoutineEditor { routine_id }` | `RoutinesView`, `MainView` | `RootView` → calls `open_routine_editor` |

### `StartActionEditor` / `StartRoutineEditor`

These events carry an optional `Uuid` identifying the record to edit. `None` means "create new".

```rust
// Defined in root_view.rs:
pub struct StartActionEditor { pub action_id: Option<Uuid> }

// Defined in routines_view.rs:
pub struct StartRoutineEditor { pub routine_id: Option<Uuid> }
```

Both use `Uuid` directly — do NOT store the ID as `String`. Emit:
```rust
cx.emit(StartActionEditor { action_id: Some(saved_action.id) });
```

---

## Components

Reusable UI pieces that are not full views live in `components/`. They may be stateful (GPUI entities) or stateless (functions returning `impl IntoElement`).

| Component | Description |
|---|---|
| `popover.rs` | Overlay backdrop shell and `CloseOverlay` action. `popover::init(cx)` must be called at startup. |
| `drag_drop.rs` | Drag-and-drop primitives used by the pipeline view. |
| `checkbox.rs` | Custom checkbox component. |
| `slider.rs` | Slider wrapper. |
| `command_palette/` | Command palette overlay — fuzzy-searchable list of registered commands. |
| `custom_list/` | Custom scrollable list component. |
| `gallery/` | Gallery/grid display component. |
| `picker/` | Generic picker overlay. |
| `resizable/` | Resizable panel splitter. |
| `task_creator/` | Task creator overlay — quick action capture. |

---

## Starter Mental State IDs

The eight built-in `SavedMentalState` records have stable hardcoded UUIDs defined in `app_core::starter_states` (re-exported from `app_core::mental_state::starter_states`). Use them to declare states without a database round-trip:

```rust
use app_core::starter_states;

store.declare_mental_state(starter_states::COASTING_ID, cx);    // balanced / neutral
store.declare_mental_state(starter_states::ENERGIZED_ID, cx);
store.declare_mental_state(starter_states::FOCUSED_ID, cx);     // hyperfocused
store.declare_mental_state(starter_states::SCATTERED_ID, cx);   // analysis paralysis
store.declare_mental_state(starter_states::TIRED_ID, cx);
store.declare_mental_state(starter_states::FOGGY_ID, cx);
store.declare_mental_state(starter_states::OVERWHELMED_ID, cx); // overstimulated
store.declare_mental_state(starter_states::FRIED_ID, cx);       // fully depleted / intense emotion
```

---

## Application Design

> For product philosophy, core concepts, features, user needs, inspirations, and implementation roadmap, see the top-level **`DESIGN.md`** at the workspace root.