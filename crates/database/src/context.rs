use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Snapshot of the user's context at a given time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub id: String,
    pub recorded_at: Option<String>,
    pub time_of_day: Option<String>,
    pub device: Option<String>,
    pub created_at: Option<String>,
    pub day_type: Option<String>,
    pub environment: Option<String>,
    pub location: Option<String>,
    pub active_mental_state: Option<String>,
    pub metadata: Option<String>,
}

impl ContextSnapshot {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            recorded_at: None,
            time_of_day: None,
            device: None,
            created_at: None,
            day_type: None,
            environment: None,
            location: None,
            active_mental_state: None,
            metadata: None,
        }
    }
}

/// Mental states that can be recorded or inferred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentalState {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: Option<String>,
}

impl MentalState {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: None,
            created_at: None,
        }
    }
}

/// Occurrences (events) of mental states (user-declared or inferred).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentalStateEvent {
    pub id: String,
    pub mental_state_id: String,
    pub intensity: Option<i64>,
    pub recorded_at: Option<String>,
    pub context_snapshot_id: Option<String>,
}

impl MentalStateEvent {
    pub fn new(mental_state_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            mental_state_id: mental_state_id.into(),
            intensity: None,
            recorded_at: None,
            context_snapshot_id: None,
        }
    }
}

/// Events recorded against actions or instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub instance_id: Option<String>,
    pub action_id: Option<String>,
    pub occurred_at: Option<String>,
    pub note: Option<String>,
    pub created_at: Option<String>,
    pub metadata: Option<String>,
}

impl Event {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            instance_id: None,
            action_id: None,
            occurred_at: None,
            note: None,
            created_at: None,
            metadata: None,
        }
    }
}

pub fn insert_event(
    conn: &Connection,
    note: &str,
    action_id: Option<&str>,
    instance_id: Option<&str>,
) -> Result<()> {
    let event_id = Uuid::new_v4().to_string();

    conn.execute(
        r#"
            INSERT INTO events (
                id,
                instance_id,
                action_id,
                occurred_at,
                note,
                created_at,
                metadata
            )
            VALUES (
                ?1,
                ?2,
                ?3,
                datetime('now'),
                ?4,
                datetime('now'),
                NULL
            )
        "#,
        (&event_id, instance_id, action_id, note),
    )
    .context("Failed to insert event")?;
    Ok(())
}
