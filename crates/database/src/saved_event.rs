use anyhow::{Context, Result};
use app_core::{ActionContext, RecurrenceRule, SavedConstraints, SavedEvent, TimesOfDay};
use chrono::NaiveTime;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Flat database representation of a SavedEvent. The ordered list of SavedAction IDs
/// is loaded separately from the saved_event_steps junction table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedEventModel {
    pub id: String,
    pub title: String,
    pub content: Option<String>,
    pub default_duration_secs: i64,
    // ActionContext
    pub energy_rate: Option<i64>,
    pub attention_level: Option<i64>,
    pub transition_difficulty: Option<i64>,
    pub importance: Option<i64>,
    // SavedConstraints
    pub valid_times_of_day: Option<i64>,
    pub deadline: Option<String>,
    pub minimum_duration_secs: Option<i64>,
    pub transition_time_secs: Option<i64>,
    pub spoons_required: Option<i64>,
    pub dependencies: Option<String>,
    // RecurrenceRule (flattened; all None + false = no recurrence)
    pub recurrence_min_interval_secs: Option<i64>,
    pub recurrence_max_interval_secs: Option<i64>,
    pub recurrence_auto_reschedule: bool,
    // Loaded from saved_event_steps junction table
    pub steps: Vec<String>,
}

impl From<&SavedEvent> for SavedEventModel {
    fn from(saved: &SavedEvent) -> Self {
        let dependencies = if saved.constraints.dependencies.is_empty() {
            None
        } else {
            let ids: Vec<String> = saved
                .constraints
                .dependencies
                .iter()
                .map(|id| id.to_string())
                .collect();
            serde_json::to_string(&ids).ok()
        };

        Self {
            id: saved.id.to_string(),
            title: saved.title.clone(),
            content: saved.content.clone(),
            default_duration_secs: saved.default_duration.num_seconds(),
            energy_rate: saved.context.energy_rate.map(|v| v as i64),
            attention_level: saved.context.attention_level.map(|v| v as i64),
            transition_difficulty: saved.context.transition_difficulty.map(|v| v as i64),
            importance: saved.context.importance.map(|v| v as i64),
            valid_times_of_day: saved
                .constraints
                .valid_times_of_day
                .map(|t| t.bits() as i64),
            deadline: saved
                .constraints
                .deadline
                .map(|t| t.format("%H:%M:%S").to_string()),
            minimum_duration_secs: saved.constraints.minimum_duration.map(|d| d.num_seconds()),
            transition_time_secs: saved.constraints.transition_time.map(|d| d.num_seconds()),
            spoons_required: saved.constraints.spoons_required.map(|v| v as i64),
            dependencies,
            recurrence_min_interval_secs: saved
                .recurrence
                .as_ref()
                .and_then(|r| r.min_interval)
                .map(|d| d.num_seconds()),
            recurrence_max_interval_secs: saved
                .recurrence
                .as_ref()
                .and_then(|r| r.max_interval)
                .map(|d| d.num_seconds()),
            recurrence_auto_reschedule: saved
                .recurrence
                .as_ref()
                .map_or(false, |r| r.auto_reschedule),
            steps: saved.steps.iter().map(|id| id.to_string()).collect(),
        }
    }
}

impl TryFrom<SavedEventModel> for SavedEvent {
    type Error = anyhow::Error;

    fn try_from(model: SavedEventModel) -> Result<Self> {
        let id = Uuid::parse_str(&model.id)
            .with_context(|| format!("Invalid saved_event id '{}'", model.id))?;

        let deadline = model
            .deadline
            .as_deref()
            .map(|s| {
                NaiveTime::parse_from_str(s, "%H:%M:%S")
                    .with_context(|| format!("Invalid deadline time '{}'", s))
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

        let recurrence = {
            let has_recurrence = model.recurrence_min_interval_secs.is_some()
                || model.recurrence_max_interval_secs.is_some()
                || model.recurrence_auto_reschedule;
            if has_recurrence {
                Some(RecurrenceRule {
                    min_interval: model
                        .recurrence_min_interval_secs
                        .map(chrono::Duration::seconds),
                    max_interval: model
                        .recurrence_max_interval_secs
                        .map(chrono::Duration::seconds),
                    auto_reschedule: model.recurrence_auto_reschedule,
                })
            } else {
                None
            }
        };

        let steps = model
            .steps
            .iter()
            .map(|s| Uuid::parse_str(s).with_context(|| format!("Invalid step UUID '{}'", s)))
            .collect::<Result<Vec<Uuid>>>()?;

        Ok(SavedEvent {
            id,
            title: model.title,
            content: model.content,
            default_duration: chrono::Duration::seconds(model.default_duration_secs),
            context: ActionContext {
                energy_rate: model.energy_rate.map(|v| v as i8),
                attention_level: model.attention_level.map(|v| v as u8),
                transition_difficulty: model.transition_difficulty.map(|v| v as u8),
                importance: model.importance.map(|v| v as u8),
            },
            constraints: SavedConstraints {
                valid_times_of_day,
                deadline,
                minimum_duration: model.minimum_duration_secs.map(chrono::Duration::seconds),
                transition_time: model.transition_time_secs.map(chrono::Duration::seconds),
                spoons_required: model.spoons_required.map(|v| v as u32),
                dependencies,
            },
            recurrence,
            steps,
        })
    }
}

fn row_to_model(row: &rusqlite::Row) -> rusqlite::Result<SavedEventModel> {
    Ok(SavedEventModel {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        default_duration_secs: row.get(3)?,
        energy_rate: row.get(4)?,
        attention_level: row.get(5)?,
        transition_difficulty: row.get(6)?,
        importance: row.get(7)?,
        valid_times_of_day: row.get(8)?,
        deadline: row.get(9)?,
        minimum_duration_secs: row.get(10)?,
        transition_time_secs: row.get(11)?,
        spoons_required: row.get(12)?,
        dependencies: row.get(13)?,
        recurrence_min_interval_secs: row.get(14)?,
        recurrence_max_interval_secs: row.get(15)?,
        recurrence_auto_reschedule: row.get::<_, i64>(16)? != 0,
        steps: Vec::new(), // populated separately
    })
}

fn fetch_step_ids(conn: &Connection, saved_event_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT saved_action_id FROM saved_event_steps
            WHERE saved_event_id = $1
            ORDER BY position ASC
            "#,
        )
        .context("Failed to prepare saved_event_steps fetch query")?;

    let ids = stmt
        .query_map([saved_event_id], |row| row.get(0))
        .context("Failed to query saved_event_steps")?
        .collect::<rusqlite::Result<Vec<String>>>()
        .context("Failed to map saved_event_steps rows")?;

    Ok(ids)
}

fn insert_step_ids(conn: &Connection, saved_event_id: &str, steps: &[String]) -> Result<()> {
    conn.execute(
        "DELETE FROM saved_event_steps WHERE saved_event_id = $1",
        [saved_event_id],
    )
    .context("Failed to clear saved_event_steps before insert")?;

    for (position, saved_action_id) in steps.iter().enumerate() {
        conn.execute(
            r#"
            INSERT INTO saved_event_steps (saved_event_id, saved_action_id, position)
            VALUES ($1, $2, $3)
            "#,
            rusqlite::params![saved_event_id, saved_action_id, position as i64],
        )
        .context("Failed to insert saved_event_steps row")?;
    }

    Ok(())
}

pub fn insert_saved_event(conn: &Connection, saved: &SavedEvent) -> Result<()> {
    let model = SavedEventModel::from(saved);

    conn.execute(
        r#"
            INSERT INTO saved_events (
                id, title, content, default_duration_secs,
                energy_rate, attention_level, transition_difficulty, importance,
                valid_times_of_day, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies,
                recurrence_min_interval_secs, recurrence_max_interval_secs,
                recurrence_auto_reschedule
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17
            )
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
                default_duration_secs = excluded.default_duration_secs,
                energy_rate = excluded.energy_rate,
                attention_level = excluded.attention_level,
                transition_difficulty = excluded.transition_difficulty,
                importance = excluded.importance,
                valid_times_of_day = excluded.valid_times_of_day,
                deadline = excluded.deadline,
                minimum_duration_secs = excluded.minimum_duration_secs,
                transition_time_secs = excluded.transition_time_secs,
                spoons_required = excluded.spoons_required,
                dependencies = excluded.dependencies,
                recurrence_min_interval_secs = excluded.recurrence_min_interval_secs,
                recurrence_max_interval_secs = excluded.recurrence_max_interval_secs,
                recurrence_auto_reschedule = excluded.recurrence_auto_reschedule
        "#,
        rusqlite::params![
            model.id,
            model.title,
            model.content,
            model.default_duration_secs,
            model.energy_rate,
            model.attention_level,
            model.transition_difficulty,
            model.importance,
            model.valid_times_of_day,
            model.deadline,
            model.minimum_duration_secs,
            model.transition_time_secs,
            model.spoons_required,
            model.dependencies,
            model.recurrence_min_interval_secs,
            model.recurrence_max_interval_secs,
            model.recurrence_auto_reschedule as i64,
        ],
    )
    .context("Failed to insert or update saved event")?;

    insert_step_ids(conn, &model.id, &model.steps)?;

    Ok(())
}

pub fn fetch_saved_events(conn: &Connection) -> Result<Vec<SavedEvent>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, title, content, default_duration_secs,
                energy_rate, attention_level, transition_difficulty, importance,
                valid_times_of_day, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies,
                recurrence_min_interval_secs, recurrence_max_interval_secs,
                recurrence_auto_reschedule
            FROM saved_events
            ORDER BY title ASC
            "#,
        )
        .context("Failed to prepare saved event fetch query")?;

    let models = stmt
        .query_map([], |row| row_to_model(row))
        .context("Failed to query saved events")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to map saved event rows")?;

    models
        .into_iter()
        .map(|mut model| {
            model.steps = fetch_step_ids(conn, &model.id)?;
            SavedEvent::try_from(model)
        })
        .collect()
}

pub fn fetch_saved_event_by_id(conn: &Connection, id: Uuid) -> Result<Option<SavedEvent>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, title, content, default_duration_secs,
                energy_rate, attention_level, transition_difficulty, importance,
                valid_times_of_day, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies,
                recurrence_min_interval_secs, recurrence_max_interval_secs,
                recurrence_auto_reschedule
            FROM saved_events
            WHERE id = $1
            "#,
        )
        .context("Failed to prepare saved event fetch by id query")?;

    let model = stmt
        .query_row([id.to_string()], |row| row_to_model(row))
        .optional()
        .context("Failed to fetch saved event by id")?;

    model
        .map(|mut model| {
            model.steps = fetch_step_ids(conn, &model.id)?;
            SavedEvent::try_from(model)
        })
        .transpose()
}

pub fn delete_saved_event(conn: &Connection, id: Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM saved_event_steps WHERE saved_event_id = $1",
        [id.to_string()],
    )
    .with_context(|| {
        format!(
            "Failed to delete saved_event_steps for saved event '{}'",
            id
        )
    })?;

    conn.execute("DELETE FROM saved_events WHERE id = $1", [id.to_string()])
        .with_context(|| format!("Failed to delete saved event '{}'", id))?;

    Ok(())
}
