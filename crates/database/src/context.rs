use std::fmt;

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

impl fmt::Display for ContextSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();

        if let Some(ref time) = self.time_of_day {
            parts.push(format!("time: {}", time));
        }

        if let Some(ref location) = self.location {
            parts.push(format!("location: {}", location));
        }

        if let Some(ref environment) = self.environment {
            parts.push(format!("env: {}", environment));
        }

        if let Some(ref mental_state) = self.active_mental_state {
            parts.push(format!("mental state: {}", mental_state));
        }

        if let Some(ref device) = self.device {
            parts.push(format!("device: {}", device));
        }

        if let Some(ref day_type) = self.day_type {
            parts.push(format!("day: {}", day_type));
        }

        if parts.is_empty() {
            write!(f, "Context snapshot")?;
        } else {
            write!(f, "Context: {}", parts.join(", "))?;
        }

        if let Some(ref recorded_at) = self.recorded_at {
            write!(f, " (recorded: {})", recorded_at)?;
        }

        Ok(())
    }
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

impl fmt::Display for MentalState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;

        if let Some(ref description) = self.description {
            write!(f, " - {}", description)?;
        }

        Ok(())
    }
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

impl fmt::Display for MentalStateEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Mental state event")?;

        if let Some(intensity) = self.intensity {
            write!(f, " (intensity: {})", intensity)?;
        }

        if let Some(ref recorded_at) = self.recorded_at {
            write!(f, " at {}", recorded_at)?;
        }

        Ok(())
    }
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

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Event")?;

        if let Some(ref note) = self.note {
            write!(f, ": {}", note)?;
        }

        if let Some(ref occurred_at) = self.occurred_at {
            write!(f, " (occurred: {})", occurred_at)?;
        }

        Ok(())
    }
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

// ============================================================================
// Context Snapshot Functions
// ============================================================================

/// Insert a new context snapshot into the database.
pub fn insert_context_snapshot(conn: &Connection, snapshot: &ContextSnapshot) -> Result<String> {
    conn.execute(
        r#"
            INSERT INTO context_snapshots (
                id,
                recorded_at,
                time_of_day,
                device,
                created_at,
                day_type,
                environment,
                location,
                active_mental_state,
                metadata
            )
            VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5, ?6, ?7, ?8, ?9)
        "#,
        (
            &snapshot.id,
            &snapshot.recorded_at,
            &snapshot.time_of_day,
            &snapshot.device,
            &snapshot.day_type,
            &snapshot.environment,
            &snapshot.location,
            &snapshot.active_mental_state,
            &snapshot.metadata,
        ),
    )
    .context("Failed to insert context snapshot")?;

    Ok(snapshot.id.clone())
}

/// Fetch context snapshots from the database, most recent first.
pub fn fetch_context_snapshots(conn: &Connection, limit: usize) -> Result<Vec<ContextSnapshot>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                recorded_at,
                time_of_day,
                device,
                created_at,
                day_type,
                environment,
                location,
                active_mental_state,
                metadata
            FROM context_snapshots
            ORDER BY recorded_at DESC
            LIMIT ?1
        "#,
        )
        .context("Failed to prepare statement for fetching context snapshots")?;

    let snapshots = stmt
        .query_map([limit as i64], |row| {
            Ok(ContextSnapshot {
                id: row.get(0)?,
                recorded_at: row.get(1)?,
                time_of_day: row.get(2)?,
                device: row.get(3)?,
                created_at: row.get(4)?,
                day_type: row.get(5)?,
                environment: row.get(6)?,
                location: row.get(7)?,
                active_mental_state: row.get(8)?,
                metadata: row.get(9)?,
            })
        })
        .context("Failed to query context snapshots")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect context snapshots")?;

    Ok(snapshots)
}

/// Fetch the most recent context snapshot.
pub fn fetch_current_context(conn: &Connection) -> Result<Option<ContextSnapshot>> {
    let mut snapshots = fetch_context_snapshots(conn, 1)?;
    Ok(snapshots.pop())
}

/// Update the energy level in the most recent context snapshot.
/// If no snapshot exists, creates a new one with just the energy level set.
pub fn update_context_energy(conn: &Connection, energy: f64) -> Result<()> {
    let current = fetch_current_context(conn)?;

    if let Some(snapshot) = current {
        // Update metadata with energy level
        let metadata = if let Some(existing_metadata) = &snapshot.metadata {
            let mut json: serde_json::Value =
                serde_json::from_str(existing_metadata).unwrap_or_else(|_| serde_json::json!({}));
            json["energy"] = serde_json::json!(energy);
            serde_json::to_string(&json).unwrap_or_default()
        } else {
            serde_json::json!({"energy": energy}).to_string()
        };

        conn.execute(
            "UPDATE context_snapshots SET metadata = ?1 WHERE id = ?2",
            (&metadata, &snapshot.id),
        )
        .context("Failed to update context energy")?;
    } else {
        // Create new snapshot with energy in metadata
        let metadata = serde_json::json!({"energy": energy}).to_string();
        let mut snapshot = ContextSnapshot::new();
        snapshot.metadata = Some(metadata);
        snapshot.recorded_at = Some(chrono::Utc::now().to_rfc3339());
        insert_context_snapshot(conn, &snapshot)?;
    }

    Ok(())
}

/// Update the attention level in the most recent context snapshot.
/// If no snapshot exists, creates a new one with just the attention level set.
pub fn update_context_attention(conn: &Connection, attention: f64) -> Result<()> {
    let current = fetch_current_context(conn)?;

    if let Some(snapshot) = current {
        // Update metadata with attention level
        let metadata = if let Some(existing_metadata) = &snapshot.metadata {
            let mut json: serde_json::Value =
                serde_json::from_str(existing_metadata).unwrap_or_else(|_| serde_json::json!({}));
            json["attention"] = serde_json::json!(attention);
            serde_json::to_string(&json).unwrap_or_default()
        } else {
            serde_json::json!({"attention": attention}).to_string()
        };

        conn.execute(
            "UPDATE context_snapshots SET metadata = ?1 WHERE id = ?2",
            (&metadata, &snapshot.id),
        )
        .context("Failed to update context attention")?;
    } else {
        // Create new snapshot with attention in metadata
        let metadata = serde_json::json!({"attention": attention}).to_string();
        let mut snapshot = ContextSnapshot::new();
        snapshot.metadata = Some(metadata);
        snapshot.recorded_at = Some(chrono::Utc::now().to_rfc3339());
        insert_context_snapshot(conn, &snapshot)?;
    }

    Ok(())
}

// ============================================================================
// Mental State Functions
// ============================================================================

/// Insert a new mental state definition into the database.
pub fn insert_mental_state(conn: &Connection, state: &MentalState) -> Result<String> {
    conn.execute(
        r#"
            INSERT INTO mental_states (
                id,
                name,
                description,
                created_at
            )
            VALUES (?1, ?2, ?3, datetime('now'))
        "#,
        (&state.id, &state.name, &state.description),
    )
    .context("Failed to insert mental state")?;

    Ok(state.id.clone())
}

/// Fetch all mental state definitions.
pub fn fetch_mental_states(conn: &Connection) -> Result<Vec<MentalState>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, name, description, created_at
            FROM mental_states
            ORDER BY name
        "#,
        )
        .context("Failed to prepare statement for fetching mental states")?;

    let states = stmt
        .query_map([], |row| {
            Ok(MentalState {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .context("Failed to query mental states")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect mental states")?;

    Ok(states)
}

/// Insert a new mental state event (recording of a mental state at a point in time).
pub fn insert_mental_state_event(conn: &Connection, event: &MentalStateEvent) -> Result<String> {
    conn.execute(
        r#"
            INSERT INTO mental_state_events (
                id,
                mental_state_id,
                intensity,
                recorded_at,
                context_snapshot_id
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        (
            &event.id,
            &event.mental_state_id,
            &event.intensity,
            &event.recorded_at,
            &event.context_snapshot_id,
        ),
    )
    .context("Failed to insert mental state event")?;

    Ok(event.id.clone())
}

/// Fetch mental state events, most recent first.
pub fn fetch_mental_state_events(conn: &Connection, limit: usize) -> Result<Vec<MentalStateEvent>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                mental_state_id,
                intensity,
                recorded_at,
                context_snapshot_id
            FROM mental_state_events
            ORDER BY recorded_at DESC
            LIMIT ?1
        "#,
        )
        .context("Failed to prepare statement for fetching mental state events")?;

    let events = stmt
        .query_map([limit as i64], |row| {
            Ok(MentalStateEvent {
                id: row.get(0)?,
                mental_state_id: row.get(1)?,
                intensity: row.get(2)?,
                recorded_at: row.get(3)?,
                context_snapshot_id: row.get(4)?,
            })
        })
        .context("Failed to query mental state events")?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to collect mental state events")?;

    Ok(events)
}

/// Fetch the most recent mental state event.
pub fn fetch_current_mental_state(conn: &Connection) -> Result<Option<MentalState>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                ms.id,
                ms.name,
                ms.description,
                ms.created_at
            FROM mental_state_events mse
            JOIN mental_states ms ON mse.mental_state_id = ms.id
            ORDER BY mse.recorded_at DESC
            LIMIT 1
        "#,
        )
        .context("Failed to prepare statement for fetching current mental state")?;

    let mut rows = stmt
        .query_map([], |row| {
            Ok(MentalState {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .context("Failed to query current mental state")?;

    match rows.next() {
        Some(result) => Ok(Some(result.context("Failed to parse mental state")?)),
        None => Ok(None),
    }
}
