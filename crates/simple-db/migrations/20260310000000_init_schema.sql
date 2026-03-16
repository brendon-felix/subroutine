PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS actions (
    id TEXT PRIMARY KEY NOT NULL,
    lineage_id TEXT NOT NULL,
    origin_routine_id TEXT,
    title TEXT NOT NULL,
    content TEXT,
    target TEXT,
    target_static INTEGER NOT NULL DEFAULT 0,
    duration_secs INTEGER,
    recurrence_secs INTEGER,
    ephemeral INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY NOT NULL,
    lineage_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    time TEXT NOT NULL,
    duration_secs INTEGER,
    recurrence_secs INTEGER,
    ephemeral INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS routines (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    content TEXT,
    target TEXT,
    recurrence_secs INTEGER
);

CREATE TABLE IF NOT EXISTS routine_steps (
    routine_id TEXT NOT NULL REFERENCES routines(id),
    position INTEGER NOT NULL,
    title TEXT NOT NULL,
    duration_secs INTEGER,
    PRIMARY KEY (routine_id, position)
);

-- Tracks which items are in the backlog vs queue and their position.
-- entry_type is one of: 'action', 'event'
CREATE TABLE IF NOT EXISTS pipeline_entries (
    id TEXT PRIMARY KEY NOT NULL,
    entry_type TEXT NOT NULL CHECK (entry_type IN ('action', 'event')),
    entry_id TEXT NOT NULL,
    list TEXT NOT NULL CHECK (list IN ('backlog', 'queue')),
    position INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS action_completions (
    id TEXT PRIMARY KEY NOT NULL,
    action_id TEXT NOT NULL,
    lineage_id TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_action_completions_lineage_id
    ON action_completions (lineage_id);

CREATE INDEX IF NOT EXISTS idx_action_completions_action_id
    ON action_completions (action_id);
