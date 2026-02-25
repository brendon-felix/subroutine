# CLI Design

This document captures the design decisions and conventions for the `cli` crate.

---

## Purpose

The `cli` crate is a command-line interface for Subroutine, intended primarily for development, debugging, and direct data manipulation. It provides access to the full data model without requiring the graphical application — useful for inspecting database state, scripting, and testing core logic in isolation.

The CLI is not a primary user-facing product. It trades the executive-function-friendly design of the GUI for directness and completeness. Every operation the underlying crates support should eventually be reachable from here.

---

## Architecture

### Entry Point

`main.rs` is the entry point. It calls `database::connect_and_migrate()` once to obtain a locked `DatabaseConnection`, then unlocks it and holds the raw `Connection` for the lifetime of the command invocation. There is no connection pooling or async runtime — each CLI invocation is a single synchronous call.

Subcommand dispatch is a straightforward `match` on the parsed `Commands` enum. Each arm calls the corresponding `handle_*_command` function from the relevant module.

### Module Structure

Each top-level subcommand owns a module. Modules define their `*Command` enum (parsed by clap) and a single public `handle_*_command` dispatcher that calls private functions.

| Module | Subcommand | Responsibility |
|---|---|---|
| `actions.rs` | `actions` | Concrete `Action` CRUD; creation flow; pipeline backlog insertion |
| `saved_actions.rs` | `saved-actions` | `SavedAction` template CRUD |
| `routines.rs` | `routines` | `Routine` CRUD; saved-step management |
| `mental_states.rs` | `mental-states` | `SavedMentalState` CRUD |
| `pipeline.rs` | `pipeline` | Pipeline inspection, refresh, promotion/demotion, instantiation |

### Shared Helpers

A small set of `pub` functions cross module boundaries:

- `actions::resolve_action(conn, identifier) -> Result<Action>` — resolves a concrete pipeline `Action` by full UUID, UUID prefix, or title prefix. Used by `pipeline.rs` for `promote`, `demote`, and `remove`.
- `saved_actions::resolve_saved_action(conn, identifier) -> Result<SavedAction>` — resolves a `SavedAction` template by full UUID, UUID prefix, or title prefix. Used by `routines.rs` (step management) and `pipeline.rs` (`add`).
- `actions::parse_datetime(s) -> Result<DateTime<Utc>>` — flexible absolute datetime parser (see [Input Formats](#input-formats)).
- `actions::parse_time_of_day(s) -> Result<NaiveTime>` — flexible time-of-day parser. Shared with `saved_actions.rs` for `--deadline` and `--target-time` inputs.

---

## Identifier Resolution

Every subcommand that references an entity accepts a flexible identifier string rather than requiring a full UUID. The resolution order is the same in every `resolve_*` function:

1. **Full UUID** — if `Uuid::parse_str` succeeds, a direct `fetch_*_by_id` lookup is performed and no further matching is attempted.
2. **UUID prefix** — the identifier is compared against the leading characters of each entity's UUID string. A unique match is required; ambiguous prefixes produce an error with a prompt to use more characters.
3. **Title/name prefix** — case-insensitive prefix match against the entity's `title` or `name` field. A unique match is required.

Eight characters of the UUID (the first hex group) are shown in all list and confirmation output, giving enough context to use prefix resolution unambiguously in most cases. For example, `subroutine-cli actions show b42b7c7a` is sufficient when no other action's UUID starts with those eight characters.

---

## Action Creation Flow

`actions create` follows the saved-type-first instantiation model from `app-core`:

**Non-ephemeral (default):**
1. A `SavedAction` template is constructed from the provided flags and persisted via `database::insert_saved_action`.
2. `saved_action.instantiate()` is called to produce a concrete `Action` with `saved_action_id` set and constraints materialized to the current moment.
3. Any absolute deadline provided via `--deadline` is written directly onto the concrete `Action`'s `Constraints`, overriding the materialized value. This is correct: `SavedConstraints` expresses deadlines as times-of-day, but `actions create` accepts absolute datetimes.
4. The concrete `Action` is persisted and pushed to the pipeline backlog. Both operations happen within the same command.

**Ephemeral (`--ephemeral` flag):**
1. An `Action` is constructed directly with `ephemeral: true` and no backing `SavedAction`.
2. It is persisted and pushed to the pipeline backlog in one step.

In both cases `actions create` is a single atomic command: after it completes the action exists in both the database and the pipeline backlog. There is no separate "add to pipeline" step for newly created actions.

`pipeline add <saved-action>` serves a distinct purpose: it instantiates an existing `SavedAction` template into a fresh concrete `Action` and pushes it to the backlog. This is the correct path for manually re-running a recurring template or putting a routine step's template directly into the pipeline without going through the routine.

---

## Routine Step Management

Routine steps are `SavedStep` enum values, each referencing a saved template by UUID:

- `SavedStep::Action(Uuid)` — references a `SavedAction`
- `SavedStep::Event(Uuid)` — references a `SavedEvent`

`routines add-step` and `routines remove-step` therefore accept a **saved action** identifier (resolved via `resolve_saved_action`), not a concrete action. Steps are always added as `SavedStep::Action` variants from the CLI. `SavedStep::Event` variants can be added programmatically but are not currently exposed as a CLI subcommand.

`routines show` displays each step by fetching the referenced saved template by ID and printing its title. If a step's template has been deleted, the step is shown as `<missing saved action: {uuid}>` rather than producing an error — the routine data itself is still intact.

---

## Pipeline Commands

| Command | What it does |
|---|---|
| `pipeline show` | Prints the queue (ordered, numbered) and the backlog (unordered). `--scores` and `--mental-state` flags add score annotations. |
| `pipeline refresh` | Calls `Pipeline::refresh` to auto-promote and demote based on scoring, then saves. Reports how many entries moved in each direction. |
| `pipeline add <saved-action>` | Instantiates a `SavedAction` into a new concrete `Action`, persists it, and pushes it to the backlog. |
| `pipeline remove <action>` | Removes a concrete action from wherever it is in the pipeline. If in the queue, it is demoted first, then filtered out. The action record in the database is not deleted. |
| `pipeline promote <action>` | Moves an action from the backlog into the queue. |
| `pipeline demote <action>` | Moves an action from the queue back into the backlog. |

`remove`, `promote`, and `demote` all resolve the target by concrete action identifier (UUID prefix or title prefix) via `resolve_action`. They operate on whatever is currently in the pipeline; they do not touch the saved templates.

### Scoring in `show` and `refresh`

Both commands accept `--mental-state <identifier>` to build a `Context` using a named `SavedMentalState`. Without it, a default neutral `MentalState` is used. `--spoons <n>` (available on `refresh`) seeds the spoon count.

Scores are computed via `app_core::score` and shown as two decimal places. Entries that fail constraint checking score 0.0.

---

## Input Formats

### Absolute Datetimes (`actions --deadline`)

Formats are tried in this order. All timezone-free formats are treated as UTC.

| Format | Example |
|---|---|
| RFC-3339 with offset | `2026-03-01T18:00:00Z`, `2026-03-01T18:00:00+05:00` |
| Date + time (space-separated) | `2026-03-01 18:00`, `2026-03-01 18:00:00` |
| Date + time (T-separated, no offset) | `2026-03-01T18:00`, `2026-03-01T18:00:00` |
| Date only (midnight) | `2026-03-01` |
| Time today | `18:00`, `6pm`, `6:30am`, `14` |

### Times of Day (`saved-actions --deadline`, `saved-actions --target-time`)

| Format | Example | Notes |
|---|---|---|
| 24-hour HH:MM | `18:00`, `08:00`, `8:00` | |
| 24-hour HH:MM:SS | `18:00:00` | |
| Bare hour (24-hour) | `14`, `8` | Minutes default to 0 |
| 12-hour with am/pm | `6pm`, `6am`, `6:30pm`, `6:30am` | Case-insensitive; `12am` = midnight, `12pm` = noon |

The 12-hour am/pm formats are parsed manually rather than relying on chrono's `%p` format specifier, which only accepts uppercase `AM`/`PM` during parsing.

### Durations (`saved-actions --recur-min`, `saved-actions --recur-max`)

A number followed by a unit suffix: `7d` (days), `24h` (hours), `90m` (minutes), `3600s` (seconds). No spaces between number and unit.

---

## Conventions

- **No panics.** All fallible operations use `?` to propagate errors through `anyhow::Result`. Index access is avoided; iterator methods are used instead.
- **Confirmation output is consistent.** Every mutating command prints a single confirmation line identifying the affected entity by title and short UUID: `Created action 'Morning run' (b42b7c7a) and added to pipeline backlog.`
- **Empty states are friendly.** Every `list` command prints a short message when there is nothing to show, rather than producing no output.
- **Validation before mutation.** Range checks on `--importance`, `--energy`, `--attention`, and `--transition` run before any database writes.
- **The database is not assumed to be consistent.** Missing referenced entities (e.g. a routine step whose saved action has been deleted) produce informative output rather than errors, matching the `database` crate's design of not enforcing foreign key relationships for these cases.