-- Add event_type column to events table to track different types of events
-- for learning and pattern tracking

PRAGMA foreign_keys = OFF;

ALTER TABLE events ADD COLUMN event_type TEXT NOT NULL DEFAULT 'completed';

-- Create index for efficient event type queries
CREATE INDEX idx_events_event_type ON events(event_type);

-- Create composite index for common queries (instance + type)
CREATE INDEX idx_events_instance_type ON events(instance_id, event_type);

-- Create composite index for action pattern queries (action + type + time)
CREATE INDEX idx_events_action_type_time ON events(action_id, event_type, occurred_at);

PRAGMA foreign_keys = ON;
