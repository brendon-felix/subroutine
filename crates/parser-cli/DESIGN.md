# Natural Language Parser — Design Plan

This document describes the intended design for the natural-language input parser used in Subroutine. It covers the parsing model, every recognised specifier, the grammar for each clause, open questions, and a phased implementation plan.

---

## Philosophy

The parser is the primary creation surface for actions and events. The design goal is that a user should be able to type a single line the way they would say it out loud and have the system understand it correctly — without sigils, prefixes, or structured fields.

**Guiding principles:**

- Natural order. Phrases can appear in any order: title first, time first, duration last — it should not matter.
- Loose and forgiving. Minor variations in phrasing (`3pm`, `3 pm`, `at 3pm`, `at 3 in the afternoon`) should all resolve to the same value.
- Graceful degradation. If a clause cannot be recognised it should fall through to become part of the title rather than causing an error — except for the hard constraint on events (a time must be present).
- Extensible. Adding a new specifier should not require restructuring the parser. Each clause is an independent recogniser that can be developed, tested, and switched on independently.
- User-supplied context. Some clauses (locations, tags, people) are resolved against user data, not a fixed vocabulary. The parser produces a `ParseDraft`; resolution against the user's database is a separate post-parse step.

---

## Architecture Overview

```
raw input string
      │
      ▼
  Lexer (logos)
  ─────────────
  Produces a flat Vec<SpannedToken>.
  Tokens: Word, Number, Quoted, Punct (.,), sigils (@ % ~ ! # &).
  Error tokens are folded into Word.
      │
      ▼
  Clause scanner (hand-written, token-index-based)
  ────────────────────────────────────────────────
  Walks the token stream.
  Attempts to match each recognised clause at every position.
  Successful matches are marked consumed; the remaining tokens form the title.
  Clauses may overlap in the token stream (e.g. "at noon" vs "at home").
  The scanner tries longest-match first.
      │
      ▼
  ParseDraft
  ──────────
  Structured intermediate representation.
  Fields: kind, title, when, duration, recurrence, priority, location,
          tags, people, raw, warnings.
      │
      ▼
  Post-parse resolution  (separate layer, not part of this crate)
  ──────────────────────
  Resolves soft tokens against user DB:
    location strings → Location records
    people names     → Contact records
    tag strings      → Tag records
```

### Why not a pure chumsky grammar?

A pure top-down grammar works well for structured inputs where the user follows a known template. Natural language inputs have free word order, implicit connectives (`at`, `on`, `for`, `in`), optional articles (`the`, `a`), and ambiguous tokens (`home` could be a location or a regular word). A clause-scanner approach — try each specifier at each position, take the longest match — handles this more naturally than a single grammar rule while keeping each clause's grammar simple enough to write in chumsky.

Chumsky is still used for the *interior* of each clause (e.g. the time grammar, the duration grammar, the recurrence grammar) where the sub-language is well-defined.

---

## Token Vocabulary (current)

| Token | Pattern | Notes |
|---|---|---|
| `Word` | `[A-Za-z_][A-Za-z0-9_\-:/]*` | Also catches error-folded characters |
| `Number` | `[0-9]+` | Digits only; time suffixes like `am`/`pm` are a separate `Word` |
| `Quoted` | `"[^"]*"` | Double-quoted string — quotes stripped in AST |
| `Punct` | `[.,]` | Dropped during title assembly |
| `At` | `@` | Kept as a reserved sigil stub |
| `Percent` | `%` | Kept as a reserved sigil stub |
| `Tilde` | `~` | Kept as a reserved sigil stub |
| `Bang` | `!` | Kept as a reserved sigil stub |
| `Hash` | `#` | Kept as a reserved sigil stub |
| `Amp` | `&` | Kept as a reserved sigil stub |

### Planned lexer additions

| Token | Pattern | Motivation |
|---|---|---|
| `Time` | `[0-9]{1,2}:[0-9]{2}` or `[0-9]{1,2}(am\|pm)` | Collapse clock into a single token so the clause scanner gets a clean unit |
| `IsoDate` | `[0-9]{4}-[0-9]{2}-[0-9]{2}` | Currently split into 5 tokens; a dedicated token removes the multi-token date hack |
| `OrdinalDay` | `[0-9]{1,2}(st\|nd\|rd\|th)` | Needed for "the 3rd", "21st" |

Dedicated tokens for the most-fragmented cases will significantly simplify the clause matchers and remove the span-joining workaround. They should be introduced when the corresponding clause is implemented.

---

## Recognised Specifiers

Each section describes what a specifier resolves to in `ParseDraft`, the phrases it must handle, edge cases, and the `ParseDraft` field it populates.

---

### 1. Time / When

**Field:** `when: Option<DateTime<Utc>>`

**Status:** Partially implemented (`@`-sigil form and bare keyword forms).

**Absolute references**

| Input form | Resolved to |
|---|---|
| `today`, `today at 3pm` | Today's date, specified or default time |
| `tomorrow`, `tomorrow at 10am` | Tomorrow's date, specified or default time |
| `tonight` | Today, evening default (e.g. 20:00) |
| `this morning` / `this afternoon` / `this evening` | Today + time-of-day window midpoint |
| `monday`, `next monday`, `this monday` | Nearest future Monday, default time |
| `next week` | Same weekday next week, default time |
| `2025-06-15` | ISO date, default time |
| `2025-06-15 14:30` | ISO date + 24h clock |
| `2025-01-15T08:00:00Z` | RFC-3339 |
| `june 15`, `june 15th` | Month + day, current or next year, default time |
| `the 15th` | 15th of current or next month, default time |
| `3pm`, `3:30pm`, `15:30` | Today (action) or required date (event), specified time |

**Relative references**

| Input form | Resolved to |
|---|---|
| `later` | Today, afternoon window (e.g. 14:00) — action-only soft default |
| `soon` | Today or tomorrow, morning window |
| `in 2 hours` | `now + 2h` |
| `in 30 minutes` | `now + 30min` |
| `in 3 days` | `today + 3 days`, default time |

**Implicit prepositions** — the following words introduce a time clause and should be stripped before parsing the value: `at`, `on`, `by`, `for`, `around`, `~`.

**Default times**

When only a date (no clock) is provided, a configurable per-of-day default is applied:

| Fallback | Time |
|---|---|
| `morning` words | 09:00 |
| `afternoon` words | 14:00 |
| `evening` / `tonight` words | 20:00 |
| bare date or keyword | 09:00 (configurable) |

**Event constraint:** `parse_event_input` must return an error if no time is resolved.

**Ambiguity notes:**
- `friday` — always the *next* Friday from now, never today-if-today-is-Friday, unless `this friday` is used.
- `next monday` when today is Monday — skip to the Monday seven days away.
- `monday at noon` — requires both the day-name clause and the time clause to fire on adjacent tokens; the day-name sets the date part, the time clause sets the clock.

---

### 2. Duration

**Field:** `duration: Option<Duration>`

**Status:** Implemented.

**Forms:**

| Input | Resolved to |
|---|---|
| `30m`, `30min`, `30mins`, `30 minutes` | 30 minutes |
| `1h`, `1hr`, `1hour`, `1 hour` | 1 hour |
| `1h30m`, `1h 30m`, `1:30` | 90 minutes |
| `2 hours` | 2 hours |
| `45` (bare number, context-dependent) | 45 minutes — only when adjacent to a duration-introducing word like `for` |

**Introductory words that signal a duration clause:** `for`, `lasting`, `takes`, `~`.

**Fibonacci rounding (planned):** After parsing, snap the duration to the nearest Fibonacci-scaled minute (1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144 minutes) and record a warning if the input was adjusted. This aligns with the Action attribute design in `DESIGN.md`.

---

### 3. Recurrence

**Field:** `recurrence: Option<RecurrenceSpec>`

**Status:** Grammar implemented in `recurrence_parser`, sigil clause handler stubbed.

**RecurrenceSpec variants (current):**

```rust
EveryDays(i64)          // every N days
EveryWeeks(i64)         // every N weeks
OnWeekdays(WeekdaySet)  // specific set of weekdays
```

**Recognised phrases:**

| Input | Spec |
|---|---|
| `daily`, `every day` | `EveryDays(1)` |
| `weekly`, `every week` | `EveryWeeks(1)` |
| `weekdays`, `every weekday` | `OnWeekdays({Mon–Fri})` |
| `weekends`, `every weekend` | `OnWeekdays({Sat, Sun})` |
| `every 3 days` | `EveryDays(3)` |
| `every 2 weeks` | `EveryWeeks(2)` |
| `every monday` | `OnWeekdays({Mon})` |
| `every thu, fri` | `OnWeekdays({Thu, Fri})` |
| `mon,wed,fri` | `OnWeekdays({Mon, Wed, Fri})` |

**Introductory word:** `every` — always signals a recurrence clause.

**Planned RecurrenceSpec additions:**

```rust
EveryMonths(i64)        // every N months — "every 3 months", "quarterly"
OnMonthDay(u8)          // e.g. "on the 1st", "on the 15th of each month"
```

**Combo example:** `every thursday at 6pm for 45min` — recurrence clause fires on `every thursday`, time clause fires on `at 6pm`, duration clause fires on `for 45min`.

---

### 4. Location

**Field:** `location: Option<String>`

**Status:** Field present in `ParseDraft`; clause handler stubbed.

**Resolution model:** The parser does not know the user's location list. It produces a raw string (`"home"`, `"the store"`, `"work"`). A post-parse resolver matches this against the user's `Location` records using case-insensitive prefix match or fuzzy match.

**Introductory patterns:** `at <location>`, `@ <location>`, `in <location>` — but only when the word following `at`/`in` matches a known location name **or** is a quoted string. This prevents `at 3pm` from being misread as a location.

**Ambiguity resolution:** `at` is shared between time and location. Resolution order:
1. Try to parse the token(s) after `at` as a time. If successful → time clause.
2. Try to match as a known location. If successful → location clause.
3. Otherwise → falls through to title.

This means location matching requires the user's location list to be passed into the parser at parse time as a `&[&str]` hint. This is a **planned** change to the `parse_action_input` / `parse_event_input` signatures.

**Planned signature:**

```rust
pub struct ParseContext<'a> {
    pub locations: &'a [&'a str],
    pub tags:      &'a [&'a str],
    pub people:    &'a [&'a str],
}

pub fn parse_action_input(input: &str, ctx: &ParseContext) -> Result<ParseDraft>
pub fn parse_event_input(input: &str, ctx: &ParseContext) -> Result<ParseDraft>
```

---

### 5. Tags

**Field:** `tags: Vec<String>`

**Status:** Field present in `ParseDraft`; clause handler stubbed.

**Resolution model:** Same as location — raw strings resolved post-parse.

**Recognition pattern:** Tags appear as `#word`, `#"multi word"`, or as a bare word that matches a known tag name from the `ParseContext`. Bare-word tag matching requires the context hint to avoid false positives.

**Examples:**

| Input | Result |
|---|---|
| `#work` | `["work"]` |
| `#"deep work"` | `["deep work"]` |
| (bare `work` with `"work"` in known tags) | `["work"]` |

---

### 6. Priority

**Field:** `priority: Option<Priority>`

**Status:** `Priority` enum defined; clause handler stubbed.

**Priority enum:**

```rust
enum Priority { Low, Medium, High }
```

**Recognised phrases:**

| Input | Resolved to |
|---|---|
| `urgent`, `asap`, `high priority`, `critical`, `!high`, `!!` | `High` |
| `normal priority`, `medium priority`, `!medium` | `Medium` |
| `low priority`, `not urgent`, `whenever`, `!low` | `Low` |

**Introductory patterns:** `priority`, `!` (sigil form), or standalone adjectives adjacent to no other known clause.

---

### 7. People / Attendees

**Field:** `people: Vec<String>` *(planned — not yet in `ParseDraft`)*

**Resolution model:** Like location and tags — raw names resolved post-parse against the user's contacts.

**Recognition patterns:**

| Input | Extracted |
|---|---|
| `with Isabel` | `["Isabel"]` |
| `with Bob and Alice` | `["Bob", "Alice"]` |
| `with the team` | special token `"team"` (group) |

**Introductory word:** `with`.

---

### 8. Time-of-Day Preference

**Field:** `time_of_day: Option<TimeOfDay>` *(planned — not yet in `ParseDraft`)*

Captures a soft scheduling preference when no concrete time is given.

```rust
enum TimeOfDay { Morning, Afternoon, Evening, Night }
```

| Input | Resolved to |
|---|---|
| `this morning`, `in the morning` | `Morning` |
| `this afternoon`, `later today` | `Afternoon` |
| `tonight`, `this evening` | `Evening` |
| `later` (bare) | `Afternoon` (soft default) |

---

### 9. Environment

**Field:** `environment: Option<String>` *(planned)*

Captures requirements like `outside`, `quiet`, `social`.

**Recognition patterns:** `outside`, `outdoors`, `at home`, `somewhere quiet`, `in a quiet place`.

Resolution is against a fixed vocabulary (unlike location, which is user-defined):

```
Quiet, Social, Outdoors, Home, Work, Anywhere
```

---

## Title Extraction

Everything not consumed by a recognised clause becomes the title. Rules:

1. Consumed tokens are marked; unmarked `Word`, `Number`, and `Quoted` tokens are joined in original order with single spaces.
2. `Punct` tokens are dropped.
3. Sigil tokens (`@`, `%`, etc.) are dropped if they appear without a successful clause match.
4. The title is trimmed. If empty after trimming, the parse fails with `"could not determine title"`.
5. Common stop words that typically introduce clauses but were not consumed (`at`, `on`, `for`, `by`, `with`) may optionally be stripped from the title boundaries (planned — requires care to avoid over-stripping).

---

## Clause Interaction and Ordering

Clauses can appear in any order. The scanner makes one pass over the token stream, attempting all clause matchers at each position. When multiple matchers could start at the same position, priority is:

1. Longer match wins (most-specific first).
2. Among equal length: time > location > recurrence > duration > priority > tags > people.

**Shared introductory words that require disambiguation:**

| Word | Could introduce |
|---|---|
| `at` | time (`at 3pm`), location (`at home`), environment (`at work`) |
| `on` | time (`on tuesday`), recurrence (`on weekdays`) |
| `for` | duration (`for 2 hours`), beneficiary (`for Isabel`) |
| `in` | time (`in 3 days`), location (`in the office`), environment (`in a quiet space`) |
| `every` | recurrence — unambiguous |
| `with` | people — unambiguous |

Disambiguation strategy: always try the time interpretation first; then location (using context hint); then fall through.

---

## Worked Examples

These are the canonical inputs the parser must handle, with expected `ParseDraft` outputs.

**Actions:**

```
"Go to the store 3pm tomorrow"
→ title: "Go to the store"
  when:  tomorrow 15:00
```

```
"Ask that person something later"
→ title: "Ask that person something"
  when:  today 14:00  (soft "later" default)
```

```
"Do that thing this afternoon"
→ title: "Do that thing"
  when:  today 14:00  (afternoon window midpoint)
```

```
"Do that activity every thu, fri at noon for 2 hours"
→ title:       "Do that activity"
  recurrence:  OnWeekdays({Thu, Fri})
  when:        next Thu or Fri at 12:00
  duration:    2h
```

```
"Read that book every monday at 6pm for 45min"
→ title:       "Read that book"
  recurrence:  OnWeekdays({Mon})
  when:        next Monday 18:00
  duration:    45min
```

```
"Do that activity at home today"  (home in user's location list)
→ title:    "Do that activity"
  location: "home"
  when:     today 09:00
```

**Events:**

```
"Team meeting 10a at work every tuesday for 1h"  (work in user's location list)
→ title:       "Team meeting"
  when:        next Tuesday 10:00
  location:    "work"
  recurrence:  OnWeekdays({Tue})
  duration:    1h
```

```
"Concert 6pm next thu for 2 hours"
→ title:     "Concert"
  when:      next Thursday 18:00
  duration:  2h
```

```
"Date with Isabel at 8pm"
→ title:     "Date"
  people:    ["Isabel"]
  when:      today 20:00
  duration:  1h  (event default when no duration specified)
```

---

## Constraints and Validation

### Event-specific

- A time **must** be resolved for `parse_event_input` to succeed. If only a date is given with no clock, the event is invalid (unlike actions, which accept bare dates).
- If no date is given but a time is, default to **today** (not an error).
- If neither date nor time can be resolved, return `Err("event requires a time")`.

### Duration defaults

- Events with no duration parsed default to **1 hour**.
- Actions with no duration parsed leave `duration: None` (no default).

### Recurrence + time interaction

When recurrence is present and includes specific weekdays, the `when` field is populated with the *next occurrence* of one of those weekdays. If `at <time>` is also present, that time is applied to the computed date. If no time is given, the default time (09:00) is used.

---

## Sigil Forms (Optional Shorthand)

The sigil characters (`@`, `%`, `~`, `!`, `#`, `&`) are reserved as an optional expert shorthand layer. They are **not required** and the system never produces them in output — they exist only as an alternative input form for users who prefer speed over prose.

Each sigil is an alias for the corresponding natural-language clause:

| Sigil | Natural equivalent | Example |
|---|---|---|
| `@` | time clause | `@3pm`, `@tomorrow`, `@2025-06-15` |
| `~` | duration clause | `~30m`, `~1h` |
| `%` | recurrence clause | `%daily`, `%every monday` |
| `!` | priority clause | `!high`, `!low` |
| `#` | tag clause | `#work`, `#"deep work"` |
| `&` | location clause | `&home`, `&"coffee shop"` |

Sigil forms bypass the natural-language recogniser for their clause — they are unambiguous and need no disambiguation. They should remain permanently available as a power-user interface and for programmatic input.

**Implementation note:** The sigil handlers (`parse_at_clause`, `parse_tilde_clause`, and the stubs for the rest) will be re-activated once the corresponding natural-language clause is implemented. Both paths populate the same `ParseDraft` field.

---

## Implementation Phases

### Phase 1 — Time expressions (natural language)

The highest-value addition. Most of the worked examples become valid after this phase.

**What to implement:**
- Lexer: add dedicated `Time` token (`3pm`, `15:30`, `10:00`)
- Lexer: add dedicated `IsoDate` token (`2025-06-15`) to remove the multi-token ISO date hack
- Clause scanner: `at <time>`, bare `<time>`, `<time> <day>`, `<day> <time>`, `tonight`, `this morning/afternoon/evening`
- Named day recogniser: `monday`–`sunday` (short and long), `next <day>`, `this <day>`
- Relative time: `in N hours/minutes/days`, `later`, `soon`
- Month + day: `june 15`, `june 15th`, `the 15th`
- Wire natural-language time into `parse_at_clause` as a fallback path; keep the existing `@`-sigil form working

**Worked examples unlocked:** all six in the section above.

**ParseDraft changes:** none — `when` already exists.

---

### Phase 2 — Recurrence (natural language)

Connect the already-complete `recurrence_parser` grammar to the natural-language scanner.

**What to implement:**
- Recognise `every <...>` anywhere in the input (not just after a `%` sigil)
- Handle combined `every <day(s)> at <time>` by wiring recurrence + time clauses together
- Add `EveryMonths(i64)` and `OnMonthDay(u8)` to `RecurrenceSpec`
- Handle `quarterly`, `monthly`, `biweekly`

**Worked examples unlocked:** the `every thu, fri` and `every monday` examples.

**ParseDraft changes:** `RecurrenceSpec` additions only.

---

### Phase 3 — Location

**What to implement:**
- `ParseContext` struct with location hints passed into the two entry points
- `at <location>` / `in <location>` recogniser using the context list
- Disambiguation: try time first, then location
- Post-parse resolution stub (return raw string; resolution is caller's responsibility)

**Worked examples unlocked:** `"at home today"`, `"at work every tuesday"`.

**ParseDraft changes:** `location` field already exists.

---

### Phase 4 — Duration (natural language)

Extend the existing duration clause to fire on natural-language signals, not just the `~` sigil.

**What to implement:**
- Recognise `for <duration>`, `lasting <duration>`, bare `<duration>` adjacent to a time
- Handle mixed forms: `1h30m`, `1h 30min`, `1:30`
- Implement Fibonacci rounding with a warning
- Event duration default: 1 hour when no duration is parsed for an event

**Worked examples unlocked:** the `for 2 hours`, `for 45min`, `for 1h` examples.

---

### Phase 5 — People / Attendees

**What to implement:**
- `people: Vec<String>` field on `ParseDraft`
- `with <name(s)>` recogniser
- `and`-separated list: `with Bob and Alice`

**Worked examples unlocked:** `"Date with Isabel at 8pm"`.

---

### Phase 6 — Tags and Priority (natural language)

**What to implement:**
- Bare-word tag recogniser using `ParseContext.tags` hint
- `#tag` sigil form (already stubbed — just needs the handler wired up)
- Priority phrase recogniser: `urgent`, `low priority`, etc.
- `!priority` sigil form (already stubbed)

---

### Phase 7 — Time-of-day preference, environment, future attributes

**What to implement:**
- `time_of_day: Option<TimeOfDay>` on `ParseDraft`
- `environment: Option<String>` on `ParseDraft`
- Recognisers for `this morning/afternoon/evening/tonight` (promoting from soft-time defaults into an explicit field)
- Environment vocabulary: `outside`, `somewhere quiet`, `at home`, `in a quiet place`
- Attention-level phrases: `needs focus`, `mindless task`, `requires concentration`

---

## Future / Speculative Features

### Soft constraints
Phrases like `preferably before noon`, `if possible on a weekday`, `ideally outside`. These produce lower-confidence clause values that the scheduler can use but will not enforce strictly. Represented as a parallel `preferences` field on `ParseDraft`.

### Duration ranges
`30 to 45 minutes`, `1–2 hours`. Stored as `(min_duration, max_duration)` and used to pick the Fibonacci midpoint while retaining the range for the scheduling engine.

### Deadline vs. target
`by friday`, `no later than 5pm` — produces a `deadline: Option<DateTime<Utc>>` distinct from `when` (the preferred start time).

### Reminder offset
`remind me 30 minutes before`, `with a 10-minute warning`. Produces a `reminder_offset: Option<Duration>` field.

### Importance phrases
`really important`, `not a big deal`, `can wait`. Maps to the `importance` (1–5) attribute on `Action`.

### Energy phrases
`exhausting`, `easy`, `relaxing`, `low effort`. Maps to the `energy_rate` attribute.

### Attention phrases
`needs focus`, `mindless`, `requires concentration`. Maps to `attention_level`.

### Named time slots
User-defined named times: `at lunch`, `after dinner`, `during my morning block`. Resolved from a user-managed `TimeSlot` table, similar to locations.

### Natural recurrence with ordinals
`first monday of the month`, `last friday of the month`, `every other week`. Requires `EveryNthWeekday` variant in `RecurrenceSpec`.

### Multi-sentence input
`Go to the store. Pick up milk and eggs. 30 minutes, tomorrow afternoon.` — parse as one action with a multi-sentence title, pulling specifiers from any sentence.

### Correction and amendment
`actually, make that 2 hours` — amend the most recently created entity's duration. Requires parser state / history awareness.

### Sigil combinations
Power users who want maximum brevity can mix sigils and natural language freely:
`Deep work @9am ~2h every weekday !high #work`.
Both forms are fully equivalent; the sigil form simply takes priority when present.

---

## Testing Strategy

Each clause has its own test module. Tests are written against `parse_recurrence_str`, `parse_duration_str` (planned public export), and the two top-level entry points.

For each clause, the test matrix covers:
1. All documented input forms (positive cases)
2. Position independence — clause at start, middle, and end of input
3. Title is not contaminated by consumed clause tokens
4. Adjacent clauses do not interfere with each other
5. Invalid input produces a useful error
6. Context-dependent clauses are tested with and without context hints

Integration tests use the worked examples from this document as a fixed corpus. When a new phase is implemented, its worked examples are promoted from `#[ignore]` to active tests.