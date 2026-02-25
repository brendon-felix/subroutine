# database Design

This document captures the design decisions and conventions for the `database` crate.

---

## Purpose

The `database` crate is responsible for persisting and loading the rich data types defined in `app-core`. It knows nothing about business logic or UI — its only job is to faithfully store and retrieve data, and to translate between the flat representations SQLite needs and the structured types the rest of the app uses.

---

## Architecture

### Separation of Concerns

`app-core` defines the canonical, rich data types (`Action`, `SavedAction`, `Routine`, `SavedMentalState`, etc.). The `database` crate depends on `app-core` — never the other way around. `app-core` has no knowledge of persistence.

### Model Structs

Each persisted type has a corresponding `*Model` struct in this crate. Model structs are flat — every field is a primitive that maps directly to a SQLite column (`String`, `Option<String>`, `i64`, `bool`, etc.). They derive `Serialize` and `Deserialize` for potential future use.

Model structs are an internal implementation detail. Callers interact with the CRUD functions using `app-core` types directly.

### Conversion Layer

The translation between model structs and `app-core` types uses standard library traits:

- `From<&RichType> for Model` — infallible. Flattens a rich type into a storable row. Going from a well-typed value to a flat representation cannot fail.
- `TryFrom<Model> for RichType` — fallible, returns `anyhow::Result`. Parses stored primitives back into structured types, surfacing errors if stored data is malformed.

This pattern keeps conversions explicit, compiler-enforced, and ergonomic via `.into()` and `.try_into()`.

---

## Schema Conventions

### Storage Formats

| app-core type | SQLite representation |
|---|---|
| `Uuid` | `TEXT` (hyphenated string) |
| `DateTime<Utc>` | `TEXT` (RFC-3339) |
| `NaiveTime` | `TEXT` (`HH:MM:SS`) |
| `Duration` | `INTEGER` (seconds) |
| `TimesOfDay` (bitflags) | `INTEGER` (raw bits as u8) |
| `Vec<Uuid>` (ordered steps/actions) | Junction table with `position` column |
| `Vec<Uuid>` (dependencies) | `TEXT` (JSON array of UUID strings) |
| `i8` / `u8` | `INTEGER` |
| `bool` | `INTEGER` (`0` or `1`) |
| `Option<T>` | nullable column |

### Two Constraint Formats

The schema distinguishes between **concrete constraints** (on `actions` and `events`) and **saved constraints** (on `saved_actions`, `saved_events`, `routines`, and `subroutines`):

- **Concrete constraints** — `deadline` is stored as `TEXT` (RFC-3339 absolute datetime), `earliest_start` is present.
- **Saved constraints** — `deadline` is stored as `TEXT` (`HH:MM:SS` time-of-day), `earliest_start` is absent. The time-of-day deadline is materialized into an absolute datetime at instantiation time by `SavedConstraints::materialize()`.

Both formats store all other constraint columns (`valid_times_of_day`, `minimum_duration_secs`, `transition_time_secs`, `spoons_required`, `dependencies`) identically.

### RecurrenceRule Columns

`RecurrenceRule` is stored as three flat columns on any table that carries it (`saved_actions`, `saved_events`, `routines`, `subroutines`):

| Column | Type | Meaning |
|---|---|---|
| `recurrence_min_interval_secs` | `INTEGER` (nullable) | Minimum seconds between instances |
| `recurrence_max_interval_secs` | `INTEGER` (nullable) | Maximum seconds before considered overdue |
| `recurrence_auto_reschedule` | `INTEGER NOT NULL DEFAULT 0` | Whether to auto-create next instance on completion |

When all three columns are at their null/zero defaults, `recurrence = None` is reconstructed in code.

### ActionContext Columns

`ActionContext` attributes (`energy_rate`, `attention_level`, `transition_difficulty`, `importance`) are stored as nullable `INTEGER` columns on any table that carries them (`actions`, `saved_actions`, `saved_events`). Absence of a value is represented as `NULL` — the system degrades gracefully when these are unset.

### Provenance Columns

Concrete types carry provenance as nullable `TEXT` UUID columns on their main table:

| Column | Present on | Meaning |
|---|---|---|
| `saved_action_id` | `actions` | Template this action was instantiated from |
| `saved_event_id` | `events` | Template this event was instantiated from |
| `routine_id` | `actions`, `events` | Routine that spawned this entry |
| `subroutine_id` | `actions`, `events` | Subroutine that spawned this entry |

### Separate Tables Per Type

Every distinct `app-core` type has its own table:

| Table | Type |
|---|---|
| `actions` | `Action` |
| `saved_actions` | `SavedAction` |
| `events` | `Event` |
| `saved_events` | `SavedEvent` |
| `routines` | `Routine` |
| `subroutines` | `Subroutine` |
| `saved_mental_states` | `SavedMentalState` |
| `pipeline_entries` | Pipeline position metadata |

### Junction Tables

Ordered lists of UUIDs are stored in dedicated junction tables with a `position` column rather than as JSON arrays. This keeps them queryable and avoids deserializing a JSON blob just to read a list.

| Parent | Junction table | Step column(s) |
|---|---|---|
| `Event` | `event_actions` | `action_id` — concrete `Action` IDs |
| `SavedEvent` | `saved_event_steps` | `saved_action_id` — `SavedAction` IDs |
| `Routine` | `routine_steps` | `step_type TEXT CHECK ('action', 'event')` + `step_id` |
| `Subroutine` | `subroutine_steps` | `saved_action_id` — `SavedAction` IDs only |

The `routine_steps` table uses a `step_type` discriminant column because a routine's steps can be either `SavedAction` or `SavedEvent` references. This maps to the `SavedStep` enum in `app-core`.

Steps are loaded eagerly in a second query and attached to the model before conversion. On insert, junction rows are deleted and rewritten from scratch — diffing is unnecessary complexity for realistic list sizes.

### Pipeline Entries

The `pipeline_entries` table stores which entries are in the backlog vs queue and their position within each list. It does not store `Transition` entries — transitions are ephemeral and computed at runtime. The `entry_type` column identifies which table the `entry_id` refers to (`'action'`, `'event'`, `'routine'`, `'subroutine'`); this relationship is validated in code rather than enforced by SQLite foreign keys. The `list` column is constrained to `'backlog'` or `'queue'` via a `CHECK` constraint.

`save_pipeline` replaces all rows with a full rewrite inside a single transaction. `load_pipeline` reconstructs the `Pipeline` by fetching each referenced entry by type and ID, then pushing to backlog and promoting to queue in position order.

### Saved Mental States

`SavedMentalState` maps cleanly to a flat table. The four bipolar axis values (`attention_mode`, `sensory_tolerance`, `emotional_regulation`, `social_battery`) are stored as `INTEGER` in the range −2 to +2. The starter set uses hardcoded UUIDs defined in `app-core`; this crate seeds them on first run if they are not already present.

---

## Module Structure

| Module | Responsibility |
|---|---|
| `action.rs` | `Action` CRUD — `ActionModel`, `insert_action`, `fetch_actions`, `fetch_action_by_id`, `delete_action` |
| `saved_action.rs` | `SavedAction` CRUD — `SavedActionModel`, full CRUD |
| `event.rs` | `Event` CRUD — `EventModel` with `event_actions` junction, full CRUD |
| `saved_event.rs` | `SavedEvent` CRUD — `SavedEventModel` with `saved_event_steps` junction, full CRUD |
| `routine.rs` | `Routine` and `Subroutine` CRUD — separate `RoutineModel` and `SubroutineModel`; typed step handling |
| `saved_mental_state.rs` | `SavedMentalState` CRUD and starter state seeding |
| `pipeline.rs` | `save_pipeline` and `load_pipeline` |
| `lib.rs` | Connection setup, WAL mode, migration application, seeding |

`Routine` and `Subroutine` share a file because they are structurally similar and neither is large enough to warrant its own module. They use separate model structs (`RoutineModel` and `SubroutineModel`) because their step representations differ: `RoutineModel.steps` is `Vec<(String, String)>` (step_type, step_id) while `SubroutineModel.steps` is `Vec<String>` (saved_action_id only).

---

## CRUD Conventions

- Insert functions use `ON CONFLICT(id) DO UPDATE` (upsert) so that saving an existing record updates it rather than failing.
- Fetch functions return `app-core` types directly, not model structs. Conversion errors propagate to the caller via `anyhow::Result`.
- Delete functions take a `Uuid` rather than a string to keep the interface type-safe.
- Delete functions for types with junction tables manually delete child rows first, since `ON DELETE CASCADE` is not declared in the schema.
- A private `row_to_model` function handles mapping `rusqlite::Row` to a model struct, keeping query functions concise.

---

## Seeding

`seed_starter_mental_states` is called automatically from `connect_and_migrate`. It checks each hardcoded starter state UUID individually and inserts only the missing ones. Existing rows — including user-modified versions of starter states — are left untouched. This means the upsert in `insert_saved_mental_state` and the existence-check in `seed_starter_mental_states` serve different purposes and are both correct.

---

## Migrations

Migrations are embedded at compile time via `include_str!` and applied using `rusqlite_migration`. Each migration file is named with a timestamp prefix for ordering. The `connect_and_migrate` function in `lib.rs` is the standard entry point — it opens the connection, enables WAL mode and foreign keys, applies any pending migrations, and seeds starter mental states before returning.

The schema is defined in a single consolidated migration (`20260225122443_init_schema.sql`). Because this project is in early development with no production data to preserve, schema changes are incorporated into this file rather than added as incremental migrations. A new incremental migration file should only be introduced once the schema reaches a stable, deployed state.