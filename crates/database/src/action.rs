use anyhow::{Context, Result};
use app_core::{Action, ActionContext, Constraints, TimesOfDay};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Flat database representation of an Action. All nested types from app-core are
/// flattened into primitive columns that map directly to the actions table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionModel {
    pub id: String,
    pub title: String,
    pub content: Option<String>,
    pub created_at: String,
    pub target_time: Option<String>,
    pub ephemeral: bool,
    pub saved_action_id: Option<String>,
    pub routine_id: Option<String>,
    pub subroutine_id: Option<String>,
    // ActionContext
    pub energy_rate: Option<i64>,
    pub attention_level: Option<i64>,
    pub transition_difficulty: Option<i64>,
    pub importance: Option<i64>,
    // Constraints
    pub valid_times_of_day: Option<i64>,
    pub earliest_start: Option<String>,
    pub deadline: Option<String>,
    pub minimum_duration_secs: Option<i64>,
    pub transition_time_secs: Option<i64>,
    pub spoons_required: Option<i64>,
    pub dependencies: Option<String>,
}

impl From<&Action> for ActionModel {
    fn from(action: &Action) -> Self {
        let dependencies = if action.constraints.dependencies.is_empty() {
            None
        } else {
            let ids: Vec<String> = action
                .constraints
                .dependencies
                .iter()
                .map(|id| id.to_string())
                .collect();
            serde_json::to_string(&ids).ok()
        };

        Self {
            id: action.id.to_string(),
            title: action.title.clone(),
            content: action.content.clone(),
            created_at: action.created_at.to_rfc3339(),
            target_time: action.target_time.map(|t| t.to_rfc3339()),
            ephemeral: action.ephemeral,
            saved_action_id: action.saved_action_id.map(|id| id.to_string()),
            routine_id: action.routine_id.map(|id| id.to_string()),
            subroutine_id: action.subroutine_id.map(|id| id.to_string()),
            energy_rate: action.context.energy_rate.map(|v| v as i64),
            attention_level: action.context.attention_level.map(|v| v as i64),
            transition_difficulty: action.context.transition_difficulty.map(|v| v as i64),
            importance: action.context.importance.map(|v| v as i64),
            valid_times_of_day: action
                .constraints
                .valid_times_of_day
                .map(|t| t.bits() as i64),
            earliest_start: action.constraints.earliest_start.map(|t| t.to_rfc3339()),
            deadline: action.constraints.deadline.map(|t| t.to_rfc3339()),
            minimum_duration_secs: action.constraints.minimum_duration.map(|d| d.num_seconds()),
            transition_time_secs: action.constraints.transition_time.map(|d| d.num_seconds()),
            spoons_required: action.constraints.spoons_required.map(|v| v as i64),
            dependencies,
        }
    }
}

impl TryFrom<ActionModel> for Action {
    type Error = anyhow::Error;

    fn try_from(model: ActionModel) -> Result<Self> {
        let id = Uuid::parse_str(&model.id)
            .with_context(|| format!("Invalid action id '{}'", model.id))?;

        let saved_action_id = model
            .saved_action_id
            .as_deref()
            .map(|s| Uuid::parse_str(s).with_context(|| format!("Invalid saved_action_id '{}'", s)))
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

        let target_time = model
            .target_time
            .as_deref()
            .map(|s| {
                DateTime::parse_from_rfc3339(s)
                    .with_context(|| format!("Invalid target_time '{}'", s))
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .transpose()?;

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

        Ok(Action {
            id,
            title: model.title,
            content: model.content,
            created_at,
            target_time,
            ephemeral: model.ephemeral,
            saved_action_id,
            routine_id,
            subroutine_id,
            context: ActionContext {
                energy_rate: model.energy_rate.map(|v| v as i8),
                attention_level: model.attention_level.map(|v| v as u8),
                transition_difficulty: model.transition_difficulty.map(|v| v as u8),
                importance: model.importance.map(|v| v as u8),
            },
            constraints: Constraints {
                valid_times_of_day,
                earliest_start,
                deadline,
                minimum_duration: model.minimum_duration_secs.map(chrono::Duration::seconds),
                transition_time: model.transition_time_secs.map(chrono::Duration::seconds),
                spoons_required: model.spoons_required.map(|v| v as u32),
                dependencies,
            },
        })
    }
}

fn row_to_model(row: &rusqlite::Row) -> rusqlite::Result<ActionModel> {
    Ok(ActionModel {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
        target_time: row.get(4)?,
        ephemeral: row.get::<_, i64>(5)? != 0,
        saved_action_id: row.get(6)?,
        routine_id: row.get(7)?,
        subroutine_id: row.get(8)?,
        energy_rate: row.get(9)?,
        attention_level: row.get(10)?,
        transition_difficulty: row.get(11)?,
        importance: row.get(12)?,
        valid_times_of_day: row.get(13)?,
        earliest_start: row.get(14)?,
        deadline: row.get(15)?,
        minimum_duration_secs: row.get(16)?,
        transition_time_secs: row.get(17)?,
        spoons_required: row.get(18)?,
        dependencies: row.get(19)?,
    })
}

pub fn insert_action(conn: &Connection, action: &Action) -> Result<()> {
    let model = ActionModel::from(action);
    conn.execute(
        r#"
            INSERT INTO actions (
                id, title, content, created_at, target_time, ephemeral,
                saved_action_id, routine_id, subroutine_id,
                energy_rate, attention_level, transition_difficulty, importance,
                valid_times_of_day, earliest_start, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies
            )
            VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18, ?19, ?20
            )
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
                target_time = excluded.target_time,
                ephemeral = excluded.ephemeral,
                saved_action_id = excluded.saved_action_id,
                routine_id = excluded.routine_id,
                subroutine_id = excluded.subroutine_id,
                energy_rate = excluded.energy_rate,
                attention_level = excluded.attention_level,
                transition_difficulty = excluded.transition_difficulty,
                importance = excluded.importance,
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
            model.target_time,
            model.ephemeral as i64,
            model.saved_action_id,
            model.routine_id,
            model.subroutine_id,
            model.energy_rate,
            model.attention_level,
            model.transition_difficulty,
            model.importance,
            model.valid_times_of_day,
            model.earliest_start,
            model.deadline,
            model.minimum_duration_secs,
            model.transition_time_secs,
            model.spoons_required,
            model.dependencies,
        ],
    )
    .context("Failed to insert or update action")?;
    Ok(())
}

pub fn fetch_actions(conn: &Connection) -> Result<Vec<Action>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, title, content, created_at, target_time, ephemeral,
                saved_action_id, routine_id, subroutine_id,
                energy_rate, attention_level, transition_difficulty, importance,
                valid_times_of_day, earliest_start, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies
            FROM actions
            ORDER BY created_at DESC
            "#,
        )
        .context("Failed to prepare action fetch query")?;

    let actions = stmt
        .query_map([], |row| row_to_model(row))
        .context("Failed to query actions")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to map action rows")?
        .into_iter()
        .map(Action::try_from)
        .collect::<Result<Vec<_>>>()
        .context("Failed to convert action models")?;

    Ok(actions)
}

pub fn fetch_action_by_id(conn: &Connection, id: Uuid) -> Result<Option<Action>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, title, content, created_at, target_time, ephemeral,
                saved_action_id, routine_id, subroutine_id,
                energy_rate, attention_level, transition_difficulty, importance,
                valid_times_of_day, earliest_start, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies
            FROM actions
            WHERE id = ?1
            "#,
        )
        .context("Failed to prepare action fetch by id query")?;

    let model = stmt
        .query_row([id.to_string()], |row| row_to_model(row))
        .optional()
        .context("Failed to fetch action by id")?;

    model.map(Action::try_from).transpose()
}

pub fn delete_action(conn: &Connection, id: Uuid) -> Result<()> {
    conn.execute("DELETE FROM actions WHERE id = ?1", [id.to_string()])
        .with_context(|| format!("Failed to delete action '{}'", id))?;
    Ok(())
}
