CREATE TABLE IF NOT EXISTS actions (
    id                TEXT PRIMARY KEY NOT NULL,
    lineage_id        TEXT NOT NULL,
    origin_routine_id TEXT,
    title             TEXT NOT NULL,
    content           TEXT,
    target            TEXT,
    target_static     INTEGER NOT NULL DEFAULT 0,
    naive_date        TEXT,
    duration_secs     BIGINT,
    recurrence_secs   BIGINT,
    ephemeral         INTEGER NOT NULL DEFAULT 1,
    completed_at      TEXT,
    updated_at        TEXT NOT NULL DEFAULT TO_CHAR(NOW() AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    synced_at         TEXT,
    deleted           INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS events (
    id              TEXT PRIMARY KEY NOT NULL,
    lineage_id      TEXT NOT NULL,
    title           TEXT NOT NULL,
    content         TEXT,
    time            TEXT NOT NULL,
    duration_secs   BIGINT,
    recurrence_secs BIGINT,
    ephemeral       INTEGER NOT NULL DEFAULT 1,
    updated_at      TEXT NOT NULL DEFAULT TO_CHAR(NOW() AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    synced_at       TEXT,
    deleted         INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS routines (
    id              TEXT PRIMARY KEY NOT NULL,
    title           TEXT NOT NULL,
    content         TEXT,
    target          TEXT,
    recurrence_secs BIGINT,
    updated_at      TEXT NOT NULL DEFAULT TO_CHAR(NOW() AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    synced_at       TEXT,
    deleted         INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS routine_steps (
    routine_id    TEXT NOT NULL REFERENCES routines(id),
    position      INTEGER NOT NULL,
    title         TEXT NOT NULL,
    duration_secs BIGINT,
    PRIMARY KEY (routine_id, position)
);

CREATE TABLE IF NOT EXISTS action_completions (
    id           TEXT PRIMARY KEY NOT NULL,
    action_id    TEXT NOT NULL,
    lineage_id   TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    notes        TEXT,
    updated_at   TEXT NOT NULL DEFAULT TO_CHAR(NOW() AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    synced_at    TEXT,
    deleted      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_actions_updated_at     ON actions (updated_at);
CREATE INDEX IF NOT EXISTS idx_events_updated_at      ON events (updated_at);
CREATE INDEX IF NOT EXISTS idx_routines_updated_at    ON routines (updated_at);
CREATE INDEX IF NOT EXISTS idx_completions_updated_at ON action_completions (updated_at);
