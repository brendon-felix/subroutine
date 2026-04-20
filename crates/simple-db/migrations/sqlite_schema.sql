PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS actions (
    id                TEXT PRIMARY KEY NOT NULL,
    lineage_id        TEXT NOT NULL,
    origin_routine_id TEXT,
    title             TEXT NOT NULL,
    content           TEXT,
    target            TEXT,
    target_static     INTEGER NOT NULL DEFAULT 0,
    naive_date        TEXT,
    duration_secs     INTEGER,
    recurrence_secs   INTEGER,
    ephemeral         INTEGER NOT NULL DEFAULT 1,
    completed_at      TEXT,
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    synced_at         TEXT,
    deleted           INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS events (
    id              TEXT PRIMARY KEY NOT NULL,
    lineage_id      TEXT NOT NULL,
    title           TEXT NOT NULL,
    content         TEXT,
    time            TEXT NOT NULL,
    duration_secs   INTEGER,
    recurrence_secs INTEGER,
    ephemeral       INTEGER NOT NULL DEFAULT 1,
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    synced_at       TEXT,
    deleted         INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS routines (
    id              TEXT PRIMARY KEY NOT NULL,
    title           TEXT NOT NULL,
    content         TEXT,
    target          TEXT,
    recurrence_secs INTEGER,
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    synced_at       TEXT,
    deleted         INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS routine_steps (
    routine_id    TEXT NOT NULL REFERENCES routines(id),
    position      INTEGER NOT NULL,
    title         TEXT NOT NULL,
    duration_secs INTEGER,
    PRIMARY KEY (routine_id, position)
);

CREATE TABLE IF NOT EXISTS pipeline_entries (
    id         TEXT PRIMARY KEY NOT NULL,
    entry_type TEXT NOT NULL CHECK (entry_type IN ('action', 'event')),
    entry_id   TEXT NOT NULL,
    list       TEXT NOT NULL CHECK (list IN ('backlog', 'queue')),
    position   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS action_completions (
    id           TEXT PRIMARY KEY NOT NULL,
    action_id    TEXT NOT NULL,
    lineage_id   TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    notes        TEXT,
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),
    synced_at    TEXT,
    deleted      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_action_completions_lineage_id
    ON action_completions (lineage_id);

CREATE INDEX IF NOT EXISTS idx_action_completions_action_id
    ON action_completions (action_id);

CREATE INDEX IF NOT EXISTS idx_actions_updated_at     ON actions (updated_at);
CREATE INDEX IF NOT EXISTS idx_events_updated_at      ON events (updated_at);
CREATE INDEX IF NOT EXISTS idx_routines_updated_at    ON routines (updated_at);
CREATE INDEX IF NOT EXISTS idx_completions_updated_at ON action_completions (updated_at);

-- Tracks the last time we pulled from Postgres per table.
-- One row per synced table, created on first sync.
CREATE TABLE IF NOT EXISTS sync_watermarks (
    table_name     TEXT PRIMARY KEY NOT NULL,
    last_pulled_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z'
);
