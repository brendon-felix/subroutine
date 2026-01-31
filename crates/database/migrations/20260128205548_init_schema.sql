PRAGMA foreign_keys = OFF;

-- Ensure base/core tables exist so this migration is standalone.
-- These definitions are minimal and compatible with later ALTERs in this file.
CREATE TABLE IF NOT EXISTS actions (
  id TEXT PRIMARY KEY,
  action_type TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT,
  duration_bucket INTEGER,
  energy_rate INTEGER,
  attention_level INTEGER,
  transition_difficulty INTEGER,
  enjoyment_after_start INTEGER,
  importance INTEGER,
  urgency_growth INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  preferred_time_of_day TEXT,
  metadata TEXT
);

CREATE INDEX idx_actions_title ON actions(title);

CREATE TABLE IF NOT EXISTS context_snapshots (
  id TEXT PRIMARY KEY,
  recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
  time_of_day TEXT,
  device TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_context_snapshots_recorded_at ON context_snapshots(recorded_at);

CREATE TABLE IF NOT EXISTS instances (
  id TEXT PRIMARY KEY,
  action_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'scheduled',
  scheduled_start TEXT,
  scheduled_end TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (action_id) REFERENCES actions(id) ON DELETE CASCADE
);

CREATE INDEX idx_instances_action_id_base ON instances(action_id);
CREATE INDEX idx_instances_status_base ON instances(status);

CREATE TABLE IF NOT EXISTS events (
  id TEXT PRIMARY KEY,
  instance_id TEXT,
  action_id TEXT,
  occurred_at TEXT NOT NULL DEFAULT (datetime('now')),
  note TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE,
  FOREIGN KEY (action_id) REFERENCES actions(id) ON DELETE SET NULL
);

CREATE INDEX idx_events_instance_id_base ON events(instance_id);
CREATE INDEX idx_events_action_id_base ON events(action_id);
CREATE INDEX idx_events_occurred_at_base ON events(occurred_at);

CREATE TABLE IF NOT EXISTS pipeline_items (
  id TEXT PRIMARY KEY,
  instance_id TEXT,
  action_title TEXT,
  position INTEGER,
  pipeline_id TEXT,
  item_type TEXT,
  data TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (instance_id) REFERENCES instances(id) ON DELETE CASCADE
);

CREATE INDEX idx_pipeline_items_instance_id ON pipeline_items(instance_id);

CREATE TABLE IF NOT EXISTS pipelines (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- The rest of the migration: create new tables and add columns to existing ones.
-- Action requirements: multi-valued requirements for an action (environment/location/etc).
CREATE TABLE IF NOT EXISTS action_requirements (
  id TEXT PRIMARY KEY,
  action_id TEXT NOT NULL,
  requirement_type TEXT NOT NULL, -- 'environment' | 'location' | 'device' | 'other'
  value TEXT NOT NULL,
  accessibility_score INTEGER, -- optional score indicating how accessible the requirement is
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (action_id) REFERENCES actions(id) ON DELETE CASCADE
);

CREATE INDEX idx_action_requirements_action_id ON action_requirements(action_id);
CREATE INDEX idx_action_requirements_type ON action_requirements(requirement_type);

-- Routines: templates grouping actions into sequences (ordered or randomizable).
CREATE TABLE IF NOT EXISTS routines (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  is_sequential INTEGER NOT NULL DEFAULT 1, -- boolean-ish (0/1)
  allow_randomization INTEGER NOT NULL DEFAULT 0,
  default_start_time TEXT, -- '08:00' etc (optional)
  default_end_time TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Steps belonging to routines. step_order is used when is_sequential = 1.
CREATE TABLE IF NOT EXISTS routine_steps (
  id TEXT PRIMARY KEY,
  routine_id TEXT NOT NULL,
  action_id TEXT NOT NULL,
  step_order INTEGER NOT NULL DEFAULT 0,
  min_duration_bucket INTEGER,
  max_duration_bucket INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (routine_id) REFERENCES routines(id) ON DELETE CASCADE,
  FOREIGN KEY (action_id) REFERENCES actions(id) ON DELETE RESTRICT
);

CREATE INDEX idx_routine_steps_routine_id ON routine_steps(routine_id);
CREATE INDEX idx_routine_steps_action_id ON routine_steps(action_id);

-- Mental states that can be recorded or inferred.
CREATE TABLE IF NOT EXISTS mental_states (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Occurrences (events) of mental states (user-declared or inferred).
CREATE TABLE IF NOT EXISTS mental_state_events (
  id TEXT PRIMARY KEY,
  mental_state_id TEXT NOT NULL,
  intensity INTEGER, -- 1..5
  recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
  context_snapshot_id TEXT, -- optional link to a context snapshot
  FOREIGN KEY (mental_state_id) REFERENCES mental_states(id) ON DELETE CASCADE,
  FOREIGN KEY (context_snapshot_id) REFERENCES context_snapshots(id) ON DELETE SET NULL
);

CREATE INDEX idx_mental_state_events_state_id ON mental_state_events(mental_state_id);

-- Add richer context snapshot columns. SQLite's ALTER TABLE supports ADD COLUMN but not IF NOT EXISTS
-- on some older SQLite versions, so we add columns unconditionally (this will fail if column exists).
-- The migration is expected to run once on a clean DB; if you need idempotency, run checks in Rust.
ALTER TABLE context_snapshots ADD COLUMN day_type TEXT;
ALTER TABLE context_snapshots ADD COLUMN environment TEXT; -- serialized JSON or comma list
ALTER TABLE context_snapshots ADD COLUMN location TEXT;    -- serialized JSON or comma list
ALTER TABLE context_snapshots ADD COLUMN active_mental_state TEXT; -- mental_states.id or free text
ALTER TABLE context_snapshots ADD COLUMN metadata TEXT; -- free-form JSON metadata

-- Instances: add source and time-window columns to support flexible scheduling.
ALTER TABLE instances ADD COLUMN source TEXT; -- 'manual' | 'routine' | 'suggested' | 'generated'
ALTER TABLE instances ADD COLUMN earliest_start TEXT;
ALTER TABLE instances ADD COLUMN latest_end TEXT;
ALTER TABLE instances ADD COLUMN metadata TEXT; -- JSON for future extensions

-- Actions columns `preferred_time_of_day` and `metadata` are created in the `actions` table definition above,
-- so separate ALTER TABLE statements are not necessary in this migration.

-- Events: allow optional metadata column to store extra info from the UI/learning engine.
ALTER TABLE events ADD COLUMN metadata TEXT;

-- Pipeline items: add metadata so we can store transient UI hints or reason traces.
ALTER TABLE pipeline_items ADD COLUMN metadata TEXT;

-- Add small helper indexes for common lookups (some may already exist from base creation).
CREATE INDEX idx_instances_action_id ON instances(action_id);
CREATE INDEX idx_instances_status ON instances(status);
CREATE INDEX idx_events_instance_id ON events(instance_id);
CREATE INDEX idx_events_action_id ON events(action_id);
CREATE INDEX idx_events_occurred_at ON events(occurred_at);

PRAGMA foreign_keys = ON;
