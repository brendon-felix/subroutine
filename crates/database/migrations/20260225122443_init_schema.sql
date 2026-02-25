PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS actions (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    created_at TEXT NOT NULL,
    target_time TEXT,
    ephemeral INTEGER NOT NULL DEFAULT 0,
    saved_action_id TEXT,
    routine_id TEXT,
    subroutine_id TEXT,

    -- ActionContext
    energy_rate INTEGER,
    attention_level INTEGER,
    transition_difficulty INTEGER,
    importance INTEGER,

    -- Constraints (inline, absolute datetimes)
    valid_times_of_day INTEGER,
    earliest_start TEXT,
    deadline TEXT,
    minimum_duration_secs INTEGER,
    transition_time_secs INTEGER,
    spoons_required INTEGER,
    dependencies TEXT -- JSON array of UUID strings
);

CREATE TABLE IF NOT EXISTS saved_actions (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    target_time TEXT, -- HH:MM:SS preferred time of day

    -- ActionContext
    energy_rate INTEGER,
    attention_level INTEGER,
    transition_difficulty INTEGER,
    importance INTEGER,

    -- SavedConstraints (inline, time-of-day based)
    valid_times_of_day INTEGER,
    deadline TEXT,              -- HH:MM:SS time-of-day, not an absolute datetime
    minimum_duration_secs INTEGER,
    transition_time_secs INTEGER,
    spoons_required INTEGER,
    dependencies TEXT,          -- JSON array of UUID strings

    -- RecurrenceRule (NULL columns = no recurrence)
    recurrence_min_interval_secs INTEGER,
    recurrence_max_interval_secs INTEGER,
    recurrence_auto_reschedule INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS saved_mental_states (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    attention_mode INTEGER NOT NULL DEFAULT 0,
    sensory_tolerance INTEGER NOT NULL DEFAULT 0,
    emotional_regulation INTEGER NOT NULL DEFAULT 0,
    social_battery INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    created_at TEXT NOT NULL,
    time TEXT NOT NULL,
    duration_secs INTEGER NOT NULL DEFAULT 0,
    ephemeral INTEGER NOT NULL DEFAULT 0,
    saved_event_id TEXT,
    routine_id TEXT,
    subroutine_id TEXT,

    -- Constraints (inline, absolute datetimes)
    valid_times_of_day INTEGER,
    earliest_start TEXT,
    deadline TEXT,
    minimum_duration_secs INTEGER,
    transition_time_secs INTEGER,
    spoons_required INTEGER,
    dependencies TEXT -- JSON array of UUID strings
);

CREATE TABLE IF NOT EXISTS event_actions (
    event_id TEXT NOT NULL,
    action_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (event_id, action_id)
);

CREATE TABLE IF NOT EXISTS saved_events (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    default_duration_secs INTEGER NOT NULL DEFAULT 0,

    -- ActionContext
    energy_rate INTEGER,
    attention_level INTEGER,
    transition_difficulty INTEGER,
    importance INTEGER,

    -- SavedConstraints (inline, time-of-day based)
    valid_times_of_day INTEGER,
    deadline TEXT,              -- HH:MM:SS time-of-day
    minimum_duration_secs INTEGER,
    transition_time_secs INTEGER,
    spoons_required INTEGER,
    dependencies TEXT,          -- JSON array of UUID strings

    -- RecurrenceRule (NULL columns = no recurrence)
    recurrence_min_interval_secs INTEGER,
    recurrence_max_interval_secs INTEGER,
    recurrence_auto_reschedule INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS saved_event_steps (
    saved_event_id TEXT NOT NULL,
    saved_action_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (saved_event_id, saved_action_id)
);

CREATE TABLE IF NOT EXISTS routines (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    created_at TEXT NOT NULL,

    -- SavedConstraints (inline, time-of-day based)
    valid_times_of_day INTEGER,
    deadline TEXT,              -- HH:MM:SS time-of-day
    minimum_duration_secs INTEGER,
    transition_time_secs INTEGER,
    spoons_required INTEGER,
    dependencies TEXT,          -- JSON array of UUID strings

    -- RecurrenceRule
    recurrence_min_interval_secs INTEGER,
    recurrence_max_interval_secs INTEGER,
    recurrence_auto_reschedule INTEGER NOT NULL DEFAULT 0
);

-- Steps for a Routine. Each step references either a saved_action or a saved_event.
CREATE TABLE IF NOT EXISTS routine_steps (
    routine_id TEXT NOT NULL,
    step_type TEXT NOT NULL CHECK (step_type IN ('action', 'event')),
    step_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (routine_id, step_id)
);

CREATE TABLE IF NOT EXISTS subroutines (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    created_at TEXT NOT NULL,

    -- SavedConstraints (inline, time-of-day based)
    valid_times_of_day INTEGER,
    deadline TEXT,              -- HH:MM:SS time-of-day
    minimum_duration_secs INTEGER,
    transition_time_secs INTEGER,
    spoons_required INTEGER,
    dependencies TEXT,          -- JSON array of UUID strings

    -- RecurrenceRule
    recurrence_min_interval_secs INTEGER,
    recurrence_max_interval_secs INTEGER,
    recurrence_auto_reschedule INTEGER NOT NULL DEFAULT 0
);

-- Steps for a Subroutine. Only saved actions (no events) can be subroutine steps.
CREATE TABLE IF NOT EXISTS subroutine_steps (
    subroutine_id TEXT NOT NULL,
    saved_action_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (subroutine_id, saved_action_id)
);

-- Tracks which entries are in the backlog vs queue and their position within each list.
-- entry_type identifies which table entry_id refers to: 'action', 'event', 'routine', 'subroutine'.
-- Transition entries are ephemeral and are never stored here.
CREATE TABLE IF NOT EXISTS pipeline_entries (
    id TEXT PRIMARY KEY NOT NULL,
    entry_type TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    list TEXT NOT NULL CHECK (list IN ('backlog', 'queue')),
    position INTEGER NOT NULL
);
