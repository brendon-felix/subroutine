# app-core Design

This document captures the key concepts, data model decisions, and design rationale for the `app-core` crate.

---

## Core Philosophy

Subroutine is an executive function prosthetic. Every design decision should reduce decision fatigue and accommodate challenges with planning, transitions, and task initiation. The system should adapt to the user — not the other way around.

---

## Saved Types vs. Concrete Types

The data model makes a hard distinction between **saved types** (reusable templates) and **concrete types** (live pipeline entries):

| Saved (template) | Concrete (pipeline entry) |
|---|---|
| `SavedAction` | `Action` |
| `SavedEvent` | `Event` |
| `Routine` | — (replaced by its instantiated steps) |
| `Subroutine` | — (replaced by its instantiated steps) |

**Design rules:**

1. **All concrete actions and events live in the pipeline.** Every `Action` and `Event` that exists is in either the backlog or the queue. There is no separate action list.

2. **Saved types are the reusable templates.** `SavedAction` and `SavedEvent` are the persistent definitions of things the user might do. They are never in the pipeline directly.

3. **`ephemeral` controls whether a template is created.** When `ephemeral = false` (the default), creating an `Action` or `Event` implicitly creates a `SavedAction` or `SavedEvent` as its backing template. When `ephemeral = true`, the concrete instance is one-off — no template is created.

4. **Instantiation materializes saved constraints into concrete constraints.** `SavedConstraints` on a saved type expresses deadlines as times-of-day. `instantiate()` calls `SavedConstraints::materialize(now)` to resolve these into absolute `Constraints` anchored to the current moment.

5. **Concrete types carry provenance.** Each `Action` and `Event` knows which saved template it came from (`saved_action_id`, `saved_event_id`), and optionally which routine or subroutine spawned it (`routine_id`, `subroutine_id`). Behaviorally, provenance is informational only — it does not affect pipeline scoring or constraint checking.

---

## Actions

Everything the user might do is an `Action`. There is no mandatory type or category field. Forcing users to classify their actions adds friction and doesn't align with the "everything is an action" principle. If the recommendation engine eventually needs to distinguish tasks from activities, that distinction should be optional or inferred — never required at creation time.

An action that involves multiple steps is better represented as a `Routine` or `Subroutine`.

### ActionContext

Actions carry optional context attributes that inform scoring and recommendations. None are required — the system degrades gracefully when attributes are absent, defaulting to neutral scores.

- **`energy_rate: Option<i8>`** — how draining or energizing this action is. Range: −2 (very draining) to +2 (very energizing).
- **`attention_level: Option<u8>`** — how much focused attention this action requires. Range: 1 (minimal) to 5 (deep focus).
- **`transition_difficulty: Option<u8>`** — how hard it is to start and stop. High values mean the action resists interruption. Range: 1–5.
- **`importance: Option<u8>`** — how important this action is, independent of urgency. Range: 1–5.

`ActionContext` also appears on `SavedAction`, `SavedEvent`, and is used when scoring instantiated concrete entries.

---

## Events

An `Event` is a concrete, time-anchored occurrence in the pipeline. Like `Action`, every `Event` lives in the backlog or queue.

`SavedEvent` is the reusable template for events — it stores a default duration, `ActionContext`, `SavedConstraints`, an optional `RecurrenceRule`, and an ordered list of `SavedAction` IDs that form the event's agenda. When instantiated, the concrete `Event` receives the resolved `Constraints` and an empty `actions` list; the caller is responsible for instantiating the agenda steps separately and attaching their IDs.

---

## SavedConstraints vs. Constraints

Two constraint types serve different purposes:

### `SavedConstraints`

Used on templates (`SavedAction`, `SavedEvent`, `Routine`, `Subroutine`). Expresses constraints in relative terms:

- `deadline: Option<NaiveTime>` — a time-of-day, not an absolute datetime.
- No `earliest_start` — that is always context-specific.
- Otherwise mirrors `Constraints`.

`SavedConstraints::materialize(now: DateTime<Utc>) -> Constraints` converts the template's constraints into concrete, absolute `Constraints` anchored to `now`. The `deadline` NaiveTime resolves to the next upcoming wall-clock occurrence: today's occurrence if it hasn't passed yet, otherwise tomorrow's.

### `Constraints`

Used on concrete pipeline entries (`Action`, `Event`). All values are absolute:

- `deadline: Option<DateTime<Utc>>` — absolute moment.
- `earliest_start: Option<DateTime<Utc>>` — absolute moment.
- `valid_times_of_day`, `minimum_duration`, `transition_time`, `spoons_required`, `dependencies` — carried over directly from `SavedConstraints`.

---

## RecurrenceRule

`RecurrenceRule` lives on saved types (`SavedAction`, `SavedEvent`, `Routine`, `Subroutine`). Concrete types have no recurrence of their own — when a concrete instance is completed, the recurrence rule on its template determines whether a new instance is automatically created.

```rust
pub struct RecurrenceRule {
    pub min_interval: Option<Duration>,  // minimum gap before next instance
    pub max_interval: Option<Duration>,  // max gap before considered overdue
    pub auto_reschedule: bool,           // create next instance automatically on completion
}
```

The `scheduler` module provides `should_reschedule` / `reschedule_action` / `should_reschedule_event` / `reschedule_event` for checking and acting on recurrence rules. The scheduler is a simple function rather than a background task — the app layer calls it explicitly at startup and after completing an action.

---

## Context

`Context` is a lightweight snapshot of the user's current situation. It is constructed fresh from live app state whenever it is needed and passed into constraint checking, scoring, and pipeline refresh. It is never stored on the `Pipeline` itself.

```rust
pub struct Context {
    pub current_time: DateTime<Utc>,
    pub mental_state: MentalState,
}
```

`Context::new(mental_state)` captures `Utc::now()` automatically. The caller is responsible for constructing `MentalState` from current app state before building a `Context`.

> **Note:** `valid_times_of_day` constraint checks currently use the UTC component of `current_time`. This should use local time once timezone support is added to `Context`.

---

## Mental State

Mental state drives which actions are surfaced, in what order, and what interventions are triggered. The system needs to know the user's current state without interrogating them at every turn.

### Derived Spoon Tracking

Spoons (executive function capacity) are tracked automatically as a side effect of normal usage. Each completed action deducts from the running spoon count based on its `energy_rate`. Spoons recover over time at a configurable rate. The user never has to answer "how many spoons do you have?"

**Spoon cost formula:** `net_cost = 1 - energy_rate`

| energy_rate | net cost |
|---|---|
| −2 (very draining) | 3 spoons |
| −1 (draining) | 2 spoons |
| 0 (neutral) | 1 spoon |
| +1 (energizing) | 0 spoons |
| +2 (very energizing) | −1 spoon (restores 1) |

Constants:
- `MAX_SPOONS = 10` — the fully-rested spoon count.
- `SPOON_RECOVERY_RATE = 2.0` — spoons recovered per hour. A fully depleted user recovers to full in 5 hours.

`MentalState::complete_action(&action)` applies the cost. `MentalState::recover_spoons(elapsed)` applies time-based recovery, capped at `MAX_SPOONS`.

### Saved Mental States

Users can create reusable mental state profiles — named snapshots of how they're feeling. When they declare a state ("I'm feeling overwhelmed"), the system applies that profile's values to the current mental state.

Four bipolar axes capture the dimensions most relevant to recommendations, each on a scale of −2 to +2 with 0 as neutral:

- **Attention mode** — scattered (−) to hyperfocused (+)
- **Sensory tolerance** — understimulated (−) to overstimulated (+)
- **Emotional regulation** — dysregulated (−) to regulated (+)
- **Social battery** — drained (−) to charged (+)

These axes were chosen because they each have distinct implications for which actions are appropriate. Spoons alone cannot distinguish "tired and scattered" from "tired but hyperfocused."

A starter set of eight named states is provided (Coasting, Energized, Focused, Scattered, Tired, Foggy, Overwhelmed, Fried) with hardcoded UUIDs so they remain identifiable and replaceable across sessions. The user can modify or delete any of them.

---

## Constraint Checking

`check_constraints(actionable, context, completed_ids) -> bool` is a hard gate. An entry that fails constraints scores zero and is never surfaced, regardless of how well it fits the user's mental state.

Checks performed:

- **`valid_times_of_day`** — the current time must fall within one of the permitted windows.
- **`earliest_start`** — the current time must be at or past this value.
- **`spoons_required`** — `remaining_spoons` must be sufficient.
- **`dependencies`** — all dependency UUIDs must be present in `completed_ids`.

**`deadline` is intentionally not a constraint check.** A past deadline means the action is overdue and should score higher, not be hidden. Deadline proximity is handled in scoring.

`minimum_duration` and `transition_time` are scheduling hints, not hard gates for surfacing.

The `Actionable` trait's `constraints()` method returns `Constraints` by value. For `Action` and `Event` this is a clone. For `Routine` and `Subroutine`, which hold `SavedConstraints`, it calls `materialize(Utc::now())` — so pipeline scoring and constraint checking always operate on fully materialized, absolute constraints.

---

## Scoring

`score(entry, context, completed_ids) -> ScoreBreakdown` returns a weighted total in 0.0–1.0 along with a per-factor breakdown. Constraint checking is the first step — entries that fail return a zero score immediately.

`ScoreBreakdown` preserves the per-factor breakdown even though the pipeline only needs `total` today. This makes a future "why is this recommended?" UI possible without redesigning anything.

### Scoring Factors (V1)

Factors are defined as `ScoringFactor` structs in a static `DEFAULT_FACTORS` list. Adding a new factor is a matter of adding one entry to that list.

| Factor | Weight | Logic |
|---|---|---|
| `importance` | 0.4 | `(importance − 1) / 4`, defaults to 0.5 if unset |
| `attention_fit` | 0.3 | maps `attention_mode` (−2..+2) to a preferred attention level (1..5); scores closeness |
| `deadline_urgency` | 0.2 | 0.0 if no deadline; scales to 1.0 as deadline approaches within 30 days; Events use `event.time` |
| `energy_fit` | 0.1 | neutral (0.5) when spoons are full; penalizes draining actions as spoons deplete |

Weights sum to 1.0.

`deadline_urgency` returns 0.0 (not 0.5) when no deadline is set — it acts as a bonus for time-sensitive entries rather than penalizing entries without deadlines. For `Routine` and `Subroutine`, the deadline is derived from their materialized constraints.

Non-`Action` entries (Routines, Subroutines, Events) receive a neutral 0.5 on all action-context factors since they do not directly carry `ActionContext`. They are not penalized relative to each other; they simply don't benefit from those signals.

---

## The Pipeline

The Pipeline is the central system through which all actions flow. It acts as both a backlog manager and a recommendation engine. Everything the user might do passes through it.

### Two Lists

**Backlog** — a semi-ordered pool of entries that should eventually get done. Things are added here when the user captures them. The system draws from the backlog to populate the queue.

**Queue** — the active, fully ordered list of entries the user can act on now. These are the items surfaced in Focus Mode and the main pipeline view. The queue is ordered by score descending.

### Score-Based Promotion and Demotion

`Pipeline::refresh(context, completed_ids)` drives automatic promotion and demotion:

1. Queue entries scoring below `promotion_threshold` are demoted to the backlog.
2. Backlog entries scoring at or above `promotion_threshold` are promoted to the queue.
3. The queue is re-sorted by score descending.

Demotion runs before promotion so that a newly demoted entry is not immediately re-promoted in the same pass.

The threshold defaults to `0.5` and is user-configurable. A lower threshold keeps more items in the queue; a stricter threshold keeps fewer, higher-confidence items.

The user can also manually promote or demote items regardless of score via `Pipeline::promote(id)` and `Pipeline::demote(id)`.

### PipelineEntry Enum

Pipeline entries use a concrete enum rather than trait objects. The set of actionable types is defined by the app, so the closed-world assumption of an enum is correct. This also enables cloning, pattern matching, and serialization.

Variants:
- **Action** — a concrete single thing to do, always backed by a `SavedAction` unless `ephemeral = true`.
- **Event** — a concrete scheduled occurrence at a specific time, always backed by a `SavedEvent` unless `ephemeral = true`.
- **Routine** — a pipeline placeholder representing the intent to run a user-defined sequence. When activated, it is removed from the pipeline and replaced by its instantiated `Action` and `Event` steps.
- **Subroutine** — a pipeline placeholder representing the intent to run a context-selected cluster of actions. When activated, it is removed and replaced by the contextually appropriate subset of its steps.
- **Transition** — buffer time between items; does not implement `Actionable`.

`Routine` and `Subroutine` remain as `PipelineEntry` variants so they can be scored and positioned in the queue while awaiting activation. Their `Actionable` implementation materializes their `SavedConstraints` at call time via `self.constraints.materialize(Utc::now())`.

### Routines

A `Routine` stores an ordered list of `SavedStep` values, where each step is either `SavedStep::Action(Uuid)` or `SavedStep::Event(Uuid)`. The IDs reference `SavedAction` and `SavedEvent` templates.

`Routine::instantiate(saved_actions, saved_events) -> Vec<PipelineEntry>` creates one concrete entry per step, tagging each with the routine's ID in the `routine_id` provenance field. The caller removes the `PipelineEntry::Routine` from the pipeline and pushes the resulting entries into the backlog or queue.

### Subroutines

A `Subroutine` stores a pool of `SavedAction` IDs — the set of actions that tend to be done together. The list is not an ordered sequence; it is the pool to draw from.

`Subroutine::instantiate(saved_actions, context, threshold) -> Vec<PipelineEntry>` instantiates each step temporarily, scores it against the current context, and includes only those meeting or exceeding `threshold`. Each included entry is tagged with `subroutine_id`. The caller removes the `PipelineEntry::Subroutine` from the pipeline and pushes the filtered entries.

### Transitions

Transitions are ephemeral — they are computed and inserted automatically rather than stored as first-class entries. Their content is derived from the adjacent actionables (e.g. "wrap up X, prepare for Y").

Transitions are skipped during `refresh` — they are unscored and cannot be demoted. They are also excluded from pipeline persistence.

---

## Instantiation Flows

### Manual action creation (non-ephemeral)

1. App creates a `SavedAction`, persists it via `insert_saved_action`.
2. App calls `saved_action.instantiate()` → `Action` with `saved_action_id` set.
3. App pushes `PipelineEntry::Action(action)` to the pipeline backlog, persists both.

### Manual action creation (ephemeral)

1. App creates an `Action` directly with `ephemeral = true`, `saved_action_id = None`.
2. App pushes to pipeline backlog and persists — no `SavedAction` is written.

### Recurring action auto-rescheduling

1. User completes an action whose template has `recurrence.auto_reschedule = true`.
2. App calls `should_reschedule(saved, last_completed, now)` — checks `min_interval`.
3. If true, app calls `reschedule_action(saved)` → new `Action`, pushes to backlog.

### Instantiating a routine

1. `PipelineEntry::Routine(routine)` is in the queue; user activates it.
2. App fetches the needed `SavedAction`/`SavedEvent` templates from the database.
3. App calls `routine.instantiate(saved_actions, saved_events)` → `Vec<PipelineEntry>`.
4. App removes the routine placeholder, pushes the concrete entries into the pipeline.

### Instantiating a subroutine

1. `PipelineEntry::Subroutine(subroutine)` is in the queue; user activates it.
2. App fetches saved actions, builds a `Context`.
3. App calls `subroutine.instantiate(saved_actions, &context, threshold)` → filtered `Vec<PipelineEntry>`.
4. App removes the subroutine placeholder, pushes the selected entries.