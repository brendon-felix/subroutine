# Subroutine Design

This document captures the high-level product design for Subroutine — the philosophy, core concepts, features, user needs, and roadmap. It is the single source of truth for *what* the app is and *why* it works the way it does.

For *how* each part of the codebase implements these ideas, see the crate-level design documents:

- `crates/simple-core` — domain types, pipeline logic, scheduling
- `crates/simple-db` — persistence, schema, CRUD conventions
- `crates/desktop/DESIGN.md` — GPUI views, stores, overlay system, UI architecture
- `android/DESIGN.md` — Android architecture and JNI bridge conventions

---

## Core Philosophy

Subroutine is an **executive function prosthetic**, not a traditional task manager. It is a **context-aware action recommender** that accommodates ADHD and executive dysfunction through intelligent automation and flexible structure.

The central insight is that the problem is rarely *knowing* what needs to be done — it's *initiating*, *transitioning between*, and *choosing among* things to do when your executive function is impaired. A system that just lists tasks doesn't help with any of that. Subroutine is designed around the actual failure modes.

### Design Principles

1. **Minimize decision fatigue** — Decisions should be automatic, predetermined, or trivially easy to make. Every additional choice is a tax on a limited resource.
2. **Executive function first** — Every feature accommodates challenges with planning, transitions, emotional regulation, and task initiation. These are not edge cases — they are the primary use case.
3. **Flexible structure over rigid schedules** — Use time windows, not deadlines. Provide semi-flexible routines, not hard commitments. Rigidity creates failure states; flexibility creates recovery paths.
4. **Cohesive system** — Features integrate deeply rather than existing as separate tools bolted together. The pipeline, mental state, routines, and focus mode are all one system.
5. **Calm but rich** — Prioritize focus and emotional safety in the UI while maintaining depth and capability underneath. The surface should never feel overwhelming.
6. **Adapt to the user** — The system learns and personalizes over time. Users are never forced into rigid categories, workflows, or self-descriptions.

---

## Core Concepts

The app is built around five primitives that interact to produce recommendations:

- **Actions** — Things the user might do
- **Context** — The user's current environment, energy level, attention capacity, and mental state
- **States** — Named mental/emotional profiles that affect which actions are appropriate
- **Events** — Scheduled or recurring occurrences (appointments, habits) that anchor the queue
- **Structures** — Ways to group and sequence actions (routines, subroutines, vibe lists)

---

## Actions

Everything the user might do is an `Action`. There is no mandatory type or category field — forcing users to classify their actions adds friction and does not align with the adaptive philosophy. If the recommendation engine eventually needs to distinguish kinds of actions, that distinction should be optional or inferred, never required at creation time.

An action involving multiple steps is better represented as a `Routine` or `Subroutine`.

### Action Attributes

All attributes are optional. The system degrades gracefully when they are absent, defaulting to neutral scores. Filling them in makes recommendations better — it is never a prerequisite.

| Attribute | Purpose |
|---|---|
| **Duration** | Fibonacci-scaled minutes: 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144 |
| **Target time** | When the action should be performed; `None` means backlogged |
| **Target static** | If `true`, the target is pinned and will not be automatically rescheduled |
| **Energy rate** | How draining or energizing per unit time (−2 to +2) |
| **Attention level** | Required focus intensity (1–5) |
| **Transition difficulty** | How hard to start and stop (1–5) |
| **Enjoyment after start** | Motivation once begun (1–5) |
| **Importance** | Priority independent of urgency (1–5) |
| **Urgency growth** | Whether importance grows over time |
| **Environment requirements** | Predefined categories: quiet, social, outdoors, etc. |
| **Location requirements** | Predefined categories: home, work, store, etc. |
| **Preferred time of day** | Morning, afternoon, evening, night |

### Ephemeral vs. Non-Ephemeral Actions

Actions have an `ephemeral` flag:

- **Ephemeral** (`ephemeral: true`) — one-off. Created, done, gone. Completing or discarding it leaves no reusable record.
- **Non-ephemeral** (`ephemeral: false`) — the action record is kept after completion, forming a history that the system can use for recurrence, habit tracking, and learning.

The default for quick captures is ephemeral. The system encourages marking recurring or repeatable things as non-ephemeral.

### Recurrence

Recurring actions are handled through duplication, not templates. When a non-ephemeral action with a `recurrence` interval is completed, a new `Action` is spawned with a new `id` but the same `lineage_id`. The new instance inherits the parent's attributes and has its `target` set to `completion_time + recurrence`.

The `lineage_id` ties all recurrences of an action together. This allows the system to:

- Display completion history across all instances of a recurring action
- Compute streak information
- Adjust recurrence intervals over time based on actual completion patterns
- Identify which routine originally spawned an action (via `origin_routine_id`)

This model is simpler than a separate template layer: there is one type, one table, and one code path for all actions. The cost is that updating a recurrence definition requires updating the live instance (which carries forward to all future recurrences via the next spawn).

### Action Creation Questions

The UI surfaces these questions progressively to fill in attributes without overwhelming the user:

**Scope:** "About how long will this take?" · "Are there parts that could be done separately?"

**Starting:** "How hard is this to begin?" · "Is there a clear way to start and stop?"

**Energy:** "How draining or energizing is this?" · "How enjoyable once you've started?"

**Priority:** "How important is this?" · "Does it get more important over time?"

**Context:** "Which environments suit this?" · "What time of day do you prefer this?"

---

## Mental State

Mental state is the most important input to the recommendation engine. The system needs to know the user's current state without interrogating them constantly.

### Four Bipolar Axes

Mental state is expressed on four independent axes, each running from −2 to +2 with 0 as neutral:

| Axis | − end | + end |
|---|---|---|
| **Attention mode** | Scattered, difficulty focusing | Hyperfocused, absorbed |
| **Sensory tolerance** | Understimulated, seeking input | Overstimulated, overwhelmed by input |
| **Emotional regulation** | Dysregulated, reactive | Regulated, grounded |
| **Social battery** | Drained, need solitude | Charged, open to interaction |

These axes were chosen because they each have distinct, non-overlapping implications for which actions are appropriate. Spoons alone cannot distinguish "tired and scattered" from "tired but hyperfocused." The combination of these four axes and spoon level gives the system enough resolution to make meaningfully different recommendations for meaningfully different states.

### Spoon Tracking

Spoons (executive function capacity) are tracked automatically as a side effect of normal usage. Each completed action deducts from the running count based on its energy cost. Spoons recover over time. The user never has to answer "how many spoons do you have?"

The spoon system serves two purposes: it prevents the queue from filling with high-demand actions when the user is depleted, and it makes energizing actions more valuable when spoons are low.

### Saved Mental States

Rather than asking users to dial in four axes every time they open the app, the system provides named profiles — `SavedMentalState` records that snapshot a complete set of axis values. Declaring a state is a single tap: "I'm feeling overwhelmed."

A starter set of eight named states is seeded into every installation:

| Name | Character |
|---|---|
| **Coasting** | Balanced, neutral on all axes — the default resting state |
| **Energized** | High energy, good regulation, ready for almost anything |
| **Focused** | Hyperfocused attention mode — good for deep work, needs careful transitions |
| **Scattered** | Low attention, analysis paralysis — needs simple, obvious next steps |
| **Tired** | Low spoons, reduced energy — lighter actions preferred |
| **Foggy** | Scattered attention and reduced regulation — needs gentle scaffolding |
| **Overwhelmed** | High sensory load, low regulation — needs reduced input and simple choices |
| **Fried** | Depleted across the board, intense emotion — immediate de-escalation needed |

Users can modify or delete any of these and create their own. The eight starters have stable hardcoded IDs so they remain identifiable even after modification.

### Mental State Navigation

Declaring a mental state is also a navigation gesture. The system responds to certain declared states with tailored interventions:

- **Scattered / Analysis paralysis** → Brain Interrupt + Decision Maker
- **Overstimulated** → Stimulation check + Environment suggestions + Wisdom Nuggets
- **Overwhelmed** → Brain Interrupt + Emotion Decoder + Special interest suggestions
- **Hyperfocused** → Transition Primer (gradual attention shift before switching tasks)
- **Dysregulated / Intense emotion** → Emotion Decoder + Stack Trace

---

## The Pipeline

The Pipeline is the central system through which all actions flow. It acts as both a backlog manager and a context-aware recommendation engine. Everything the user might do passes through it.

### Two Lists

**Backlog** — A semi-ordered pool of `Action`s that should eventually get done. New captures land here by default (with no `target` time set). The system draws from it to populate the queue.

**Queue** — The active, time-ordered list of entries the user can act on right now. Each entry has a target time. This is what Focus Mode and the main pipeline view show. Queue entries are either `Action`s or `Event`s.

### Scheduling Model

The queue is time-ordered, not score-ordered. Each action in the queue has a `target` time. The pipeline distinguishes between two kinds of scheduled entries:

- **Static** (`target_static: true`) — pinned to a specific time. The system will not move these automatically. Conflicts are reported as warnings, not silently resolved.
- **Non-static** (`target_static: false`) — scheduled at the next available slot. When a static entry or event is inserted and causes a conflict, non-static actions are displaced to the next free slot. Groups of consecutive non-static actions are kept together and rescheduled as a block.

`refresh()` is the periodic maintenance operation. It:
- Removes expired events and spawns their next recurrence if applicable
- Detects missed non-static actions (those whose target has passed without completion) and reschedules them
- Preserves the relative ordering and grouping of rescheduled action blocks

`expedite_actions()` pulls all non-static actions earlier when the queue has free time — used after completing an action to backfill the gap.

### Score-Based Promotion and Demotion

*(Planned for V2 — not yet implemented)*

Each backlog entry will receive a score between 0.0 and 1.0 representing how well it fits the current context and mental state. Entries above a configurable threshold are promoted to the queue; entries that fall below it are demoted back to the backlog. The threshold defaults to 0.5.

Scoring is driven by four factors in V2:

| Factor | Weight | What it captures |
|---|---|---|
| Importance | 40% | How much this action matters, regardless of when |
| Attention fit | 30% | How well the action's attention demand matches current attention mode |
| Deadline urgency | 20% | How soon this must be done — scales as deadline approaches |
| Energy fit | 10% | How well the action's energy profile matches current spoon level |

The user can always manually promote or demote any entry regardless of score.

### Pipeline Entry Types

**Currently implemented:**

- **Action** — A single concrete thing to do. Lives in either the backlog or the queue.
- **Event** — A scheduled occurrence at a specific time. Always in the queue. Can recur via the same `lineage_id` + `recurrence` mechanism as actions.

**Planned:**

- **Routine** — A user-defined ordered sequence of steps. Acts as a placeholder in the queue until the user activates it, at which point it is replaced by its instantiated steps as individual actions.
- **Subroutine** — A context-selected cluster of actions drawn from a pool. Like a routine placeholder, but the specific steps selected depend on the current context at activation time.
- **Transition** — Buffer time between entries. Computed automatically and not stored — ephemeral UI-only entries.

### Transitions

*(Planned for V2)*

Transitions are inserted automatically between queue entries. Their content is derived from the adjacent entries — e.g. "wrap up X, prepare for Y." They provide a softer handoff between tasks and help with the task-switching difficulty common in monotropic minds.

Users can create one-off transition overrides. These are recorded and will eventually inform the automatic transition generator.

---

## Core Features

### Focus Mode

The minimal, calm interface for actually doing things. Shows 1–3 actions at a time drawn from the top of the queue.

**Why:** The biggest barrier to starting is often the visible mass of everything that needs to be done. Showing one thing at a time removes that. Smooth animations and a clean visual surface make it feel safe to open.

### Routines

Ordered sequences of steps that can be added to the pipeline as a single unit and then activated to produce their individual actions. Routines can be scheduled (fixed start time) or flexible (run when activated). A `Routine` has a `title`, optional `target` time, an ordered list of `RoutineStep`s (each with a title and optional duration), and an optional `recurrence` interval.

Key capability: time auto-proportioning. Give a routine a start and end time and it allocates duration across steps proportionally, adjusting all subsequent steps when one runs long.

When a routine is activated, each step is instantiated as an `Action` with `origin_routine_id` set to the routine's `id`, linking the spawned actions back to their source.

### Subroutines

Like routines, but the step list is a pool rather than an ordered sequence. At activation time, each step is scored against the current context and only those meeting the threshold are included. Used for things like "morning hygiene" where the specific steps appropriate on any given day depend on context.

### Decision Maker

Eliminates choice paralysis by making the decision for the user. The user provides a category and a list of options; an LLM selects from the list (or generates a new option if nothing fits). Used by Brain Interrupt, Vibe Lists, and any feature where "just pick one" is the right answer.

### Vibe Lists

Category-based quick pickers for predefined choices. The user sets up lists ("Meals," "Ways to recenter," "Hygiene tasks," "Exercise options") and the app picks from them on demand. Reduces "what should I..." paralysis for frequently repeated categories.

**Example categories:**
- **Meals:** Mac and cheese, pasta, chicken parmesan, Wendy's
- **Recenter:** Mindfulness, flow state work, drawing, guitar
- **Hygiene:** Shower, brush teeth, tidy desk, wash dishes
- **Exercise:** Longboarding, skatepark, dancing, bodyweight

### Idea Generator

LLM-powered brainstorming and action breakdown. Break a vague action into concrete steps (like Goblin Tools "Magic ToDo"), generate routine suggestions, name things, or get unstuck on creative problems.

### Brain Interrupt

An immediate pattern break for thought loops, analysis paralysis, and catastrophic thinking. Suggests an environment change, optionally launches an external app (music, video), and uses the Decision Maker to eliminate "but which environment?" as a decision. Breaks the rumination cycle with a physical, external interruption.

The name is intentional — it is the cognitive equivalent of turning a stuck process off and on again.

### Wisdom Nuggets

Context-aware reminders of helpful principles and accommodations. Delivered at the right moment — triggered by mental state, time of day, or user request — rather than cluttering the interface at all times.

Example nuggets:
- "Your needs are value-neutral and deserve proper accommodations."
- "Difficulty starting ≠ not wanting to do it."
- "Taking breaks is productive, not lazy."
- "Done is better than perfect. Especially right now."

### Transition Primer

A countdown-style system for easing the brain out of one context and into another. Provides advance notice of an upcoming transition, shifts attention gradually, and uses relaxing visual and audio cues to help disengage from hyperfocus safely. Particularly important for monotropic users for whom abrupt context switches are genuinely disorienting.

### Emotion Decoder

Helps the user identify and name what they are feeling. Asks guided questions, surfaces candidate emotions, and provides a relevant Wisdom Nugget. The first step toward understanding an emotional state is giving it a name — this feature does that without requiring the user to already know.

Can feed directly into Stack Trace for deeper analysis.

### Stack Trace

Traces the emotional history that led to the current state. Starting from the Emotion Decoder's output, it asks "why?" iteratively, rewinding through the mental state sequence to identify the triggering event or accumulated stress. Helps the user understand their own patterns over time.

The name is a deliberate metaphor: when something breaks, you look at the call stack to find the root cause.

---

## User Needs Being Addressed

The primary goal is **reducing decision fatigue at every level**. Every feature can be traced back to one or more of these concrete failure modes:

**Task initiation difficulty**
- Focus Mode removes the overwhelming view of everything at once
- Transition Primer creates a ramp into tasks rather than a cliff
- Brain Interrupt resets a stuck state

**Transitions between tasks**
- Automatic transition entries in the queue
- Transition Primer before context switches
- Routine time auto-proportioning handles overruns gracefully

**Analysis paralysis / choice overload**
- Decision Maker eliminates the choice entirely
- Vibe Lists pre-narrow the option space
- The pipeline surfaces one best option rather than asking the user to pick

**Overwhelm and sensory overload**
- Focus Mode's minimal UI
- Mental state navigation to immediate interventions
- Reduced visible complexity throughout

**Emotional dysregulation**
- Emotion Decoder to name the state
- Stack Trace to understand the cause
- Wisdom Nuggets for grounding
- Special interest suggestions when overwhelmed

**Morning and routine consistency**
- Transition Primer for waking up
- Saved routines for predictable sequences
- Evening prep routines to reduce next-morning decisions

**Meal planning and cooking**
- Vibe Lists for meal ideas
- Routine support for grocery trips and cooking steps
- Overstimulation detection mid-task

**Hyperfocus management**
- Transition Primer to disengage safely
- Timer features showing how long the user has been working
- Queue transitions that signal what comes next

---

## Inspirations

These apps informed the design. Features marked ✅ are specifically targeted for adoption; ⚠️ are considered with reservations.

**Tiimo:**
- ✅ Reusable routines as actions with sub-steps
- ✅ Day sections (Morning / Day / Evening)
- ✅ Separate "Anytime" backlog with priority sections and drag-and-drop
- ✅ Playlist-style compact timer (play/pause, tracks routines and actions)
- ✅ LLM "Suggest Breakdown" for sub-steps

**RoutineFlow:**
- ✅ Excellent onboarding questions that build action attributes naturally
- ✅ Clean, calm interface (monospace font for durations)
- ✅ Scheduled vs. Flexible mode toggle
- ✅ Routine habit tracking
- ✅ **"Add 1 min" button** — reschedules all subsequent routine steps (critical feature)
- ✅ Reschedule, Reset, "Do later" controls during active routines
- ✅ "Next Step" preview below the current action
- ✅ Routine finished ETA display
- ✅ Post-completion screen suggesting duration adjustments based on actual time taken
- ✅ Native banner showing step progress
- ⚠️ Chill lo-fi background music (optional, configurable)
- ⚠️ Voice reminders (useful, lower priority)

**Goblin Tools:**
- ✅ Magic ToDo — break a vague task into concrete, sized steps via LLM (basis for Idea Generator)

**Thruday:**
- ⚠️ Mood tracking (useful signal; privacy implications to consider carefully)

---

## Implementation Roadmap

### V1 — MVP

The minimal version that is genuinely useful as a daily driver.

- Basic action CRUD (create, read, update, delete, complete)
- `ActionCompletion` records written on completion; non-ephemeral actions spawn their next recurrence via `lineage_id`
- Pipeline with backlog and queue, manual promotion/demotion
- `refresh()` to reschedule missed actions and expire old events
- Focus Mode (1–3 actions)
- Simple routines (sequential, step instantiation into actions with `origin_routine_id`)
- Batch Mode (quick multi-action capture)
- Action attributes: duration, energy rate, attention level, importance
- Saved mental states with starter set; manual state declaration
- Derived spoon tracking
- Satisfying completion animations

### V2 — Context-Aware

The version where the system starts adapting to the user.

- Action attributes: environment, location, transition difficulty, time-of-day preference
- Context snapshots (time, environment detection)
- Score-based auto-promotion and demotion from backlog to queue
- Configurable promotion threshold
- "What should I do next?" surface
- Timeline view with drag-and-drop reordering
- Auto-generated transition entries between queue items

### V3 — Intelligence

The version that uses LLMs and learned behavior to reduce overhead further.

- Idea Generator (LLM action breakdown)
- Decision Maker (LLM choice picker)
- Subroutines (context-selected action clusters)
- Adaptive notifications based on usage patterns
- Brain Interrupt with environment suggestions
- Transition override recording for future learning
- Recurrence interval adjustment based on actual completion history via `lineage_id`

### V4 — Mental State Support

The version that handles the hard emotional and cognitive moments.

- Emotion Decoder
- Stack Trace (emotional history analysis)
- Wisdom Nuggets
- Transition Primer
- Mental state navigation (declared state → automatic intervention)
- Vibe Lists
- Mental state handlers (subroutines triggered by declared state)

### V5 — Polish and Extension

The version that is a complete, polished product.

- Sound effects library
- Lo-fi music integration
- Voice reminders
- Extensive personalization (themes, layouts, typography)
- Insights and statistics dashboard
- Post-routine completion feedback and duration adjustment suggestions
- Native mobile apps with banner notifications and background state tracking