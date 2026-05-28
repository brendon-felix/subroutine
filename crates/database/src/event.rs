use anyhow::{Context, Result};
use app_core::{Constraints, Event, TimesOfDay};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Flat database representation of an Event. The ordered list of action IDs
/// is loaded separately from the event_actions junction table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventModel {
    pub id: String,
    pub title: String,
    pub content: Option<String>,
    pub created_at: String,
    pub time: String,
    pub duration_secs: i64,
    pub ephemeral: bool,
    pub saved_event_id: Option<String>,
    pub routine_id: Option<String>,
    pub subroutine_id: Option<String>,
    // Constraints (inline)
    pub valid_times_of_day: Option<i64>,
    pub earliest_start: Option<String>,
    pub deadline: Option<String>,
    pub minimum_duration_secs: Option<i64>,
    pub transition_time_secs: Option<i64>,
    pub spoons_required: Option<i64>,
    pub dependencies: Option<String>,
    // Loaded from event_actions junction table
    pub actions: Vec<String>,
}

impl From<&Event> for EventModel {
    fn from(event: &Event) -> Self {
        let dependencies = if event.constraints.dependencies.is_empty() {
            None
        } else {
            let ids: Vec<String> = event
                .constraints
                .dependencies
                .iter()
                .map(|id| id.to_string())
                .collect();
            serde_json::to_string(&ids).ok()
        };

        Self {
            id: event.id.to_string(),
            title: event.title.clone(),
            content: event.content.clone(),
            created_at: event.created_at.to_rfc3339(),
            time: event.time.to_rfc3339(),
            duration_secs: event.duration.num_seconds(),
            ephemeral: event.ephemeral,
            saved_event_id: event.saved_event_id.map(|id| id.to_string()),
            routine_id: event.routine_id.map(|id| id.to_string()),
            subroutine_id: event.subroutine_id.map(|id| id.to_string()),
            valid_times_of_day: event
                .constraints
                .valid_times_of_day
                .map(|t| t.bits() as i64),
            earliest_start: event.constraints.earliest_start.map(|t| t.to_rfc3339()),
            deadline: event.constraints.deadline.map(|t| t.to_rfc3339()),
            minimum_duration_secs: event.constraints.minimum_duration.map(|d| d.num_seconds()),
            transition_time_secs: event.constraints.transition_time.map(|d| d.num_seconds()),
            spoons_required: event.constraints.spoons_required.map(|v| v as i64),
            dependencies,
            actions: event.actions.iter().map(|id| id.to_string()).collect(),
        }
    }
}

impl TryFrom<EventModel> for Event {
    type Error = anyhow::Error;

    fn try_from(model: EventModel) -> Result<Self> {
        let id = Uuid::parse_str(&model.id)
            .with_context(|| format!("Invalid event id '{}'", model.id))?;

        let saved_event_id = model
            .saved_event_id
            .as_deref()
            .map(|s| Uuid::parse_str(s).with_context(|| format!("Invalid saved_event_id '{}'", s)))
            .transpose()?;

        let routine_id = model
            .routine_id
            .as_deref()
            .map(|s| Uuid::parse_str(s).with_context(|| format!("Invalid routine_id '{}'", s)))
            .transpose()?;

        let subroutine_id = model
            .subroutine_id
            .as_deref()
            .map(|s| Uuid::parse_str(s).with_context(|| format!("Invalid subroutine_id '{}'", s)))
            .transpose()?;

        let created_at = DateTime::parse_from_rfc3339(&model.created_at)
            .with_context(|| format!("Invalid created_at '{}'", model.created_at))?
            .with_timezone(&Utc);

        let time = DateTime::parse_from_rfc3339(&model.time)
            .with_context(|| format!("Invalid time '{}'", model.time))?
            .with_timezone(&Utc);

        let earliest_start = model
            .earliest_start
            .as_deref()
            .map(|s| {
                DateTime::parse_from_rfc3339(s)
                    .with_context(|| format!("Invalid earliest_start '{}'", s))
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .transpose()?;

        let deadline = model
            .deadline
            .as_deref()
            .map(|s| {
                DateTime::parse_from_rfc3339(s)
                    .with_context(|| format!("Invalid deadline '{}'", s))
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .transpose()?;

        let valid_times_of_day = model
            .valid_times_of_day
            .map(|bits| {
                TimesOfDay::from_bits(bits as u8)
                    .with_context(|| format!("Invalid valid_times_of_day bits '{}'", bits))
            })
            .transpose()?;

        let dependencies = model
            .dependencies
            .as_deref()
            .map(|s| {
                serde_json::from_str::<Vec<String>>(s)
                    .context("Invalid dependencies JSON")?
                    .into_iter()
                    .map(|id| {
                        Uuid::parse_str(&id)
                            .with_context(|| format!("Invalid dependency UUID '{}'", id))
                    })
                    .collect::<Result<Vec<Uuid>>>()
            })
            .transpose()?
            .unwrap_or_default();

        let actions = model
            .actions
            .iter()
            .map(|s| Uuid::parse_str(s).with_context(|| format!("Invalid action UUID '{}'", s)))
            .collect::<Result<Vec<Uuid>>>()?;

        Ok(Event {
            id,
            title: model.title,
            content: model.content,
            created_at,
            time,
            duration: chrono::Duration::seconds(model.duration_secs),
            constraints: Constraints {
                valid_times_of_day,
                earliest_start,
                deadline,
                minimum_duration: model.minimum_duration_secs.map(chrono::Duration::seconds),
                transition_time: model.transition_time_secs.map(chrono::Duration::seconds),
                spoons_required: model.spoons_required.map(|v| v as u32),
                dependencies,
            },
            ephemeral: model.ephemeral,
            saved_event_id,
            routine_id,
            subroutine_id,
            actions,
        })
    }
}

fn row_to_model(row: &rusqlite::Row) -> rusqlite::Result<EventModel> {
    Ok(EventModel {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
        time: row.get(4)?,
        duration_secs: row.get(5)?,
        ephemeral: row.get::<_, i64>(6)? != 0,
        saved_event_id: row.get(7)?,
        routine_id: row.get(8)?,
        subroutine_id: row.get(9)?,
        valid_times_of_day: row.get(10)?,
        earliest_start: row.get(11)?,
        deadline: row.get(12)?,
        minimum_duration_secs: row.get(13)?,
        transition_time_secs: row.get(14)?,
        spoons_required: row.get(15)?,
        dependencies: row.get(16)?,
        actions: Vec::new(), // populated separately
    })
}

fn fetch_event_action_ids(conn: &Connection, event_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT action_id FROM event_actions
            WHERE event_id = $1
            ORDER BY position ASC
            "#,
        )
        .context("Failed to prepare event_actions fetch query")?;

    let ids = stmt
        .query_map([event_id], |row| row.get(0))
        .context("Failed to query event_actions")?
        .collect::<rusqlite::Result<Vec<String>>>()
        .context("Failed to map event_action rows")?;

    Ok(ids)
}

fn insert_event_action_ids(conn: &Connection, event_id: &str, actions: &[String]) -> Result<()> {
    conn.execute("DELETE FROM event_actions WHERE event_id = $1", [event_id])
        .context("Failed to clear event_actions before insert")?;

    for (position, action_id) in actions.iter().enumerate() {
        conn.execute(
            r#"
            INSERT INTO event_actions (event_id, action_id, position)
            VALUES ($1, $2, $3)
            "#,
            rusqlite::params![event_id, action_id, position as i64],
        )
        .context("Failed to insert event_action row")?;
    }

    Ok(())
}

pub fn insert_event(conn: &Connection, event: &Event) -> Result<()> {
    let model = EventModel::from(event);

    conn.execute(
        r#"
            INSERT INTO events (
                id, title, content, created_at, time, duration_secs,
                ephemeral, saved_event_id, routine_id, subroutine_id,
                valid_times_of_day, earliest_start, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
                time = excluded.time,
                duration_secs = excluded.duration_secs,
                ephemeral = excluded.ephemeral,
                saved_event_id = excluded.saved_event_id,
                routine_id = excluded.routine_id,
                subroutine_id = excluded.subroutine_id,
                valid_times_of_day = excluded.valid_times_of_day,
                earliest_start = excluded.earliest_start,
                deadline = excluded.deadline,
                minimum_duration_secs = excluded.minimum_duration_secs,
                transition_time_secs = excluded.transition_time_secs,
                spoons_required = excluded.spoons_required,
                dependencies = excluded.dependencies
        "#,
        rusqlite::params![
            model.id,
            model.title,
            model.content,
            model.created_at,
            model.time,
            model.duration_secs,
            model.ephemeral as i64,
            model.saved_event_id,
            model.routine_id,
            model.subroutine_id,
            model.valid_times_of_day,
            model.earliest_start,
            model.deadline,
            model.minimum_duration_secs,
            model.transition_time_secs,
            model.spoons_required,
            model.dependencies,
        ],
    )
    .context("Failed to insert or update event")?;

    insert_event_action_ids(conn, &model.id, &model.actions)?;

    Ok(())
}

pub fn fetch_events(conn: &Connection) -> Result<Vec<Event>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, title, content, created_at, time, duration_secs,
                ephemeral, saved_event_id, routine_id, subroutine_id,
                valid_times_of_day, earliest_start, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies
            FROM events
            ORDER BY time ASC
            "#,
        )
        .context("Failed to prepare event fetch query")?;

    let models = stmt
        .query_map([], |row| row_to_model(row))
        .context("Failed to query events")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to map event rows")?;

    models
        .into_iter()
        .map(|mut model| {
            model.actions = fetch_event_action_ids(conn, &model.id)?;
            Event::try_from(model)
        })
        .collect()
}

pub fn fetch_event_by_id(conn: &Connection, id: Uuid) -> Result<Option<Event>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, title, content, created_at, time, duration_secs,
                ephemeral, saved_event_id, routine_id, subroutine_id,
                valid_times_of_day, earliest_start, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies
            FROM events
            WHERE id = $1
            "#,
        )
        .context("Failed to prepare event fetch by id query")?;

    let model = stmt
        .query_row([id.to_string()], |row| row_to_model(row))
        .optional()
        .context("Failed to fetch event by id")?;

    model
        .map(|mut model| {
            model.actions = fetch_event_action_ids(conn, &model.id)?;
            Event::try_from(model)
        })
        .transpose()
}

pub fn delete_event(conn: &Connection, id: Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM event_actions WHERE event_id = $1",
        [id.to_string()],
    )
    .with_context(|| format!("Failed to delete event_actions for event '{}'", id))?;

    conn.execute("DELETE FROM events WHERE id = $1", [id.to_string()])
        .with_context(|| format!("Failed to delete event '{}'", id))?;

    Ok(())
}
