use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension};
use simple_core::Event;
use uuid::Uuid;

use crate::{DatabaseError, Result};

struct EventRow {
    id: String,
    lineage_id: String,
    title: String,
    content: Option<String>,
    time: String,
    duration_secs: Option<i64>,
    recurrence_secs: Option<i64>,
    ephemeral: i64,
}

impl<'a> TryFrom<&'a rusqlite::Row<'a>> for EventRow {
    type Error = rusqlite::Error;

    fn try_from(row: &'a rusqlite::Row<'a>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            lineage_id: row.get(1)?,
            title: row.get(2)?,
            content: row.get(3)?,
            time: row.get(4)?,
            duration_secs: row.get(5)?,
            recurrence_secs: row.get(6)?,
            ephemeral: row.get(7)?,
        })
    }
}

impl TryFrom<EventRow> for Event {
    type Error = DatabaseError;

    fn try_from(row: EventRow) -> Result<Self> {
        let id =
            Uuid::parse_str(&row.id).map_err(|e| DatabaseError::invalid_uuid(0, &row.id, e))?;

        let lineage_id = Uuid::parse_str(&row.lineage_id)
            .map_err(|e| DatabaseError::invalid_uuid(1, &row.lineage_id, e))?;

        let time = DateTime::parse_from_rfc3339(&row.time)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| DatabaseError::invalid_datetime(4, &row.time, e))?;

        Ok(Event {
            id,
            lineage_id,
            title: row.title,
            content: row.content,
            time,
            duration: row.duration_secs.map(Duration::seconds),
            recurrence: row.recurrence_secs.map(Duration::seconds),
            ephemeral: row.ephemeral != 0,
        })
    }
}

impl From<&Event> for EventRow {
    fn from(event: &Event) -> Self {
        Self {
            id: event.id.to_string(),
            lineage_id: event.lineage_id.to_string(),
            title: event.title.clone(),
            content: event.content.clone(),
            time: event.time.to_rfc3339(),
            duration_secs: event.duration.map(|d| d.num_seconds()),
            recurrence_secs: event.recurrence.map(|d| d.num_seconds()),
            ephemeral: event.ephemeral as i64,
        }
    }
}

fn row_to_event(row: &rusqlite::Row) -> rusqlite::Result<Event> {
    let event_row = EventRow::try_from(row)?;
    Event::try_from(event_row).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

pub fn upsert_event(conn: &Connection, event: &Event) -> Result<()> {
    let row = EventRow::from(event);
    conn.execute(
        r#"
        INSERT INTO events (
            id, lineage_id, title, content, time, duration_secs, recurrence_secs, ephemeral
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(id) DO UPDATE SET
            lineage_id      = excluded.lineage_id,
            title           = excluded.title,
            content         = excluded.content,
            time            = excluded.time,
            duration_secs   = excluded.duration_secs,
            recurrence_secs = excluded.recurrence_secs,
            ephemeral       = excluded.ephemeral
        "#,
        rusqlite::params![
            row.id,
            row.lineage_id,
            row.title,
            row.content,
            row.time,
            row.duration_secs,
            row.recurrence_secs,
            row.ephemeral,
        ],
    )
    .map_err(|e| DatabaseError::sqlite("Failed to insert or update event", e))?;
    Ok(())
}

pub fn fetch_events(conn: &Connection) -> Result<Vec<Event>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, lineage_id, title, content, time, duration_secs, recurrence_secs, ephemeral
            FROM events
            ORDER BY time ASC
            "#,
        )
        .map_err(|e| DatabaseError::sqlite("Failed to prepare event fetch query", e))?;

    let events = stmt
        .query_map([], row_to_event)
        .map_err(|e| DatabaseError::sqlite("Failed to query events", e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| DatabaseError::sqlite("Failed to map event rows", e))?;

    Ok(events)
}

pub fn fetch_event_by_id(conn: &Connection, id: Uuid) -> Result<Option<Event>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, lineage_id, title, content, time, duration_secs, recurrence_secs, ephemeral
            FROM events
            WHERE id = ?1
            "#,
        )
        .map_err(|e| DatabaseError::sqlite("Failed to prepare event fetch by id query", e))?;

    stmt.query_row([id.to_string()], row_to_event)
        .optional()
        .map_err(|e| DatabaseError::sqlite("Failed to fetch event by id", e))
}

pub fn delete_event(conn: &Connection, id: Uuid) -> Result<()> {
    conn.execute("DELETE FROM events WHERE id = ?1", [id.to_string()])
        .map_err(|e| DatabaseError::sqlite(format!("Failed to delete event '{}'", id), e))?;
    Ok(())
}
