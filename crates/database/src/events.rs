use anyhow::{Context as _, Result};
use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents different types of events that can occur in the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    Suggested,
    Accepted,
    Completed,
    Skipped,
    Snoozed,
    Abandoned,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::Suggested => "suggested",
            EventType::Accepted => "accepted",
            EventType::Completed => "completed",
            EventType::Skipped => "skipped",
            EventType::Snoozed => "snoozed",
            EventType::Abandoned => "abandoned",
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "suggested" => Ok(EventType::Suggested),
            "accepted" => Ok(EventType::Accepted),
            "completed" => Ok(EventType::Completed),
            "skipped" => Ok(EventType::Skipped),
            "snoozed" => Ok(EventType::Snoozed),
            "abandoned" => Ok(EventType::Abandoned),
            _ => anyhow::bail!("Invalid event type: {}", s),
        }
    }

    pub fn all() -> &'static [EventType] {
        &[
            EventType::Suggested,
            EventType::Accepted,
            EventType::Completed,
            EventType::Skipped,
            EventType::Snoozed,
            EventType::Abandoned,
        ]
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Represents a tracked event in the system (for learning and patterns)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedEvent {
    pub id: String,
    pub event_type: EventType,
    pub instance_id: Option<String>,
    pub action_id: Option<String>,
    pub occurred_at: String,
    pub note: Option<String>,
    pub metadata: Option<String>,
    pub created_at: String,
}

impl TrackedEvent {
    fn from_row(row: &Row) -> Result<Self, rusqlite::Error> {
        let event_type_str: String = row.get("event_type")?;
        let event_type =
            EventType::from_str(&event_type_str).map_err(|_| rusqlite::Error::InvalidQuery)?;

        Ok(TrackedEvent {
            id: row.get("id")?,
            event_type,
            instance_id: row.get("instance_id")?,
            action_id: row.get("action_id")?,
            occurred_at: row.get("occurred_at")?,
            note: row.get("note")?,
            metadata: row.get("metadata")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// Insert a new tracked event
pub fn insert_tracked_event(
    conn: &Connection,
    event_type: EventType,
    instance_id: Option<&str>,
    action_id: Option<&str>,
    note: Option<&str>,
) -> Result<TrackedEvent> {
    let id = Uuid::new_v4().to_string();
    let event_type_str = event_type.as_str();

    conn.execute(
        "INSERT INTO events (id, event_type, instance_id, action_id, note)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, event_type_str, instance_id, action_id, note],
    )
    .context("Failed to insert event")?;

    fetch_tracked_event_by_id(conn, &id)
}

/// Fetch a tracked event by its ID
pub fn fetch_tracked_event_by_id(conn: &Connection, id: &str) -> Result<TrackedEvent> {
    let event = conn
        .query_row(
            "SELECT id, event_type, instance_id, action_id, occurred_at, note, metadata, created_at
             FROM events
             WHERE id = ?1",
            params![id],
            TrackedEvent::from_row,
        )
        .context("Failed to fetch event by ID")?;

    Ok(event)
}

/// Fetch tracked events with optional filters
pub fn fetch_tracked_events(
    conn: &Connection,
    instance_id: Option<&str>,
    action_id: Option<&str>,
    event_type: Option<EventType>,
    limit: Option<usize>,
) -> Result<Vec<TrackedEvent>> {
    let mut query = String::from(
        "SELECT id, event_type, instance_id, action_id, occurred_at, note, metadata, created_at
         FROM events
         WHERE 1=1",
    );

    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(iid) = instance_id {
        query.push_str(" AND instance_id = ?");
        params_vec.push(Box::new(iid.to_string()));
    }

    if let Some(aid) = action_id {
        query.push_str(" AND action_id = ?");
        params_vec.push(Box::new(aid.to_string()));
    }

    if let Some(et) = event_type {
        query.push_str(" AND event_type = ?");
        params_vec.push(Box::new(et.as_str().to_string()));
    }

    query.push_str(" ORDER BY occurred_at DESC");

    if let Some(lim) = limit {
        query.push_str(" LIMIT ?");
        params_vec.push(Box::new(lim as i64));
    }

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn
        .prepare(&query)
        .context("Failed to prepare events query")?;

    let events = stmt
        .query_map(params_refs.as_slice(), TrackedEvent::from_row)
        .context("Failed to query events")?
        .collect::<Result<Vec<TrackedEvent>, _>>()
        .context("Failed to collect events")?;

    Ok(events)
}

/// Fetch tracked events for a specific instance
pub fn fetch_tracked_events_for_instance(
    conn: &Connection,
    instance_id: &str,
    limit: Option<usize>,
) -> Result<Vec<TrackedEvent>> {
    fetch_tracked_events(conn, Some(instance_id), None, None, limit)
}

/// Fetch tracked events for a specific action
pub fn fetch_tracked_events_for_action(
    conn: &Connection,
    action_id: &str,
    event_type: Option<EventType>,
    limit: Option<usize>,
) -> Result<Vec<TrackedEvent>> {
    fetch_tracked_events(conn, None, Some(action_id), event_type, limit)
}

/// Fetch recent tracked events
pub fn fetch_recent_tracked_events(conn: &Connection, limit: usize) -> Result<Vec<TrackedEvent>> {
    fetch_tracked_events(conn, None, None, None, Some(limit))
}

/// Event pattern statistics for an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPatternStats {
    pub action_id: String,
    pub action_title: String,
    pub total_events: i64,
    pub completed_count: i64,
    pub skipped_count: i64,
    pub snoozed_count: i64,
    pub abandoned_count: i64,
    pub completion_rate: f64,
}

/// Fetch event pattern statistics for actions
pub fn fetch_event_patterns(
    conn: &Connection,
    action_id: Option<&str>,
    limit: Option<usize>,
) -> Result<Vec<EventPatternStats>> {
    let mut query = String::from(
        "SELECT
            a.id as action_id,
            a.title as action_title,
            COUNT(*) as total_events,
            SUM(CASE WHEN e.event_type = 'completed' THEN 1 ELSE 0 END) as completed_count,
            SUM(CASE WHEN e.event_type = 'skipped' THEN 1 ELSE 0 END) as skipped_count,
            SUM(CASE WHEN e.event_type = 'snoozed' THEN 1 ELSE 0 END) as snoozed_count,
            SUM(CASE WHEN e.event_type = 'abandoned' THEN 1 ELSE 0 END) as abandoned_count,
            CAST(SUM(CASE WHEN e.event_type = 'completed' THEN 1 ELSE 0 END) AS FLOAT) / COUNT(*) as completion_rate
         FROM events e
         JOIN actions a ON e.action_id = a.id
         WHERE e.event_type IN ('completed', 'skipped', 'snoozed', 'abandoned')"
    );

    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(aid) = action_id {
        query.push_str(" AND a.id = ?");
        params_vec.push(Box::new(aid.to_string()));
    }

    query.push_str(" GROUP BY a.id, a.title ORDER BY total_events DESC");

    if let Some(lim) = limit {
        query.push_str(" LIMIT ?");
        params_vec.push(Box::new(lim as i64));
    }

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn
        .prepare(&query)
        .context("Failed to prepare event patterns query")?;

    let patterns = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(EventPatternStats {
                action_id: row.get("action_id")?,
                action_title: row.get("action_title")?,
                total_events: row.get("total_events")?,
                completed_count: row.get("completed_count")?,
                skipped_count: row.get("skipped_count")?,
                snoozed_count: row.get("snoozed_count")?,
                abandoned_count: row.get("abandoned_count")?,
                completion_rate: row.get("completion_rate")?,
            })
        })
        .context("Failed to query event patterns")?
        .collect::<Result<Vec<EventPatternStats>, _>>()
        .context("Failed to collect event patterns")?;

    Ok(patterns)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, Instance, migrations};

    fn setup_test_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations().to_latest(&mut conn).unwrap();
        conn
    }

    fn create_test_action(conn: &Connection, id: &str, title: &str) -> Action {
        conn.execute(
            "INSERT INTO actions (id, action_type, title) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, "task", title],
        )
        .unwrap();
        Action {
            id: id.to_string(),
            action_type: "task".to_string(),
            title: title.to_string(),
            description: None,
            duration_bucket: None,
            energy_rate: None,
            attention_level: None,
            transition_difficulty: None,
            enjoyment_after_start: None,
            importance: None,
            urgency_growth: None,
            created_at: Some("2026-01-01 00:00:00".to_string()),
            preferred_time_of_day: None,
            metadata: None,
        }
    }

    fn create_test_instance(conn: &Connection, id: &str, action_id: &str) -> Instance {
        conn.execute(
            "INSERT INTO instances (id, action_id, status) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, action_id, "scheduled"],
        )
        .unwrap();
        Instance {
            id: id.to_string(),
            action_id: action_id.to_string(),
            status: "scheduled".to_string(),
            scheduled_start: None,
            scheduled_end: None,
            created_at: Some("2026-01-01 00:00:00".to_string()),
            source: None,
            earliest_start: None,
            latest_end: None,
            metadata: None,
        }
    }

    #[test]
    fn test_event_type_conversion() {
        assert_eq!(EventType::Completed.as_str(), "completed");
        assert_eq!(
            EventType::from_str("completed").unwrap(),
            EventType::Completed
        );
        assert_eq!(
            EventType::from_str("COMPLETED").unwrap(),
            EventType::Completed
        );
        assert!(EventType::from_str("invalid").is_err());
    }

    #[test]
    fn test_insert_and_fetch_event() {
        let conn = setup_test_db();

        // Create test action and instance
        create_test_action(&conn, "action-456", "Test Action");
        create_test_instance(&conn, "instance-123", "action-456");

        let event = insert_tracked_event(
            &conn,
            EventType::Completed,
            Some("instance-123"),
            Some("action-456"),
            Some("Test note"),
        )
        .unwrap();

        assert_eq!(event.event_type, EventType::Completed);
        assert_eq!(event.instance_id.as_deref(), Some("instance-123"));
        assert_eq!(event.action_id.as_deref(), Some("action-456"));
        assert_eq!(event.note.as_deref(), Some("Test note"));

        let fetched = fetch_tracked_event_by_id(&conn, &event.id).unwrap();
        assert_eq!(fetched.id, event.id);
        assert_eq!(fetched.event_type, EventType::Completed);
    }

    #[test]
    fn test_fetch_events_with_filters() {
        let conn = setup_test_db();

        // Create test actions and instances
        create_test_action(&conn, "a1", "Action 1");
        create_test_action(&conn, "a2", "Action 2");
        create_test_instance(&conn, "i1", "a1");
        create_test_instance(&conn, "i2", "a1");
        create_test_instance(&conn, "i3", "a2");

        insert_tracked_event(&conn, EventType::Completed, Some("i1"), Some("a1"), None).unwrap();
        insert_tracked_event(&conn, EventType::Skipped, Some("i2"), Some("a1"), None).unwrap();
        insert_tracked_event(&conn, EventType::Completed, Some("i3"), Some("a2"), None).unwrap();

        let all = fetch_tracked_events(&conn, None, None, None, None).unwrap();
        assert_eq!(all.len(), 3);

        let for_action = fetch_tracked_events(&conn, None, Some("a1"), None, None).unwrap();
        assert_eq!(for_action.len(), 2);

        let completed =
            fetch_tracked_events(&conn, None, None, Some(EventType::Completed), None).unwrap();
        assert_eq!(completed.len(), 2);

        let limited = fetch_tracked_events(&conn, None, None, None, Some(1)).unwrap();
        assert_eq!(limited.len(), 1);
    }
}
