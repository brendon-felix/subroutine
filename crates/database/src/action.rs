use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Something that can be performed (Tasks, Routines, Habits, etc.) by the user.
///
/// Actions are templates for Instances, which are specific occurrences of Actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Unique identifier for the action
    pub id: String,
    /// Type of action: "task", "habit", etc
    pub action_type: String,
    /// Title of the action
    pub title: String,
    /// Optional description of the action
    pub description: Option<String>,
    /// Number of minutes using fibonacci buckets (1,2,3,5,8,13,21,34,55,89,144)
    pub duration_bucket: Option<i64>,
    /// Energy rate required for the action (-5 to +5)
    pub energy_rate: Option<i64>,
    /// Attention level required for the action (0 to 5)
    pub attention_level: Option<i64>,
    /// Difficulty of transitioning into the action (0 to 5)
    pub transition_difficulty: Option<i64>,
    /// Enjoyment after starting the action (-5 to +5)
    pub enjoyment_after_start: Option<i64>,
    /// General importance of the action (-5 to +5)
    pub importance: Option<i64>,
    /// Whether the urgency of the action grows over time
    pub urgency_growth: Option<bool>,
    /// ISO-8601 timestamp the row was created (stored as TEXT in SQLite)
    pub created_at: Option<String>,
    /// Preferred times of day expressed as free-form text (e.g. JSON array)
    pub preferred_time_of_day: Option<String>,
    /// Free-form JSON or string blob for feature metadata
    pub metadata: Option<String>,
}

impl Action {
    /// Create a generic action with explicit type and title.
    pub fn new(action_type: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            action_type: action_type.into(),
            title: title.into(),
            description: None,
            duration_bucket: None,
            energy_rate: None,
            attention_level: None,
            transition_difficulty: None,
            enjoyment_after_start: None,
            importance: None,
            urgency_growth: None,
            created_at: None,
            preferred_time_of_day: None,
            metadata: None,
        }
    }

    /// Create a new task-style action with a generated id and defaults.
    pub fn new_task(title: impl Into<String>) -> Self {
        Self::new("task", title)
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn duration_bucket(mut self, duration_bucket: i64) -> Self {
        self.duration_bucket = Some(duration_bucket);
        self
    }

    pub fn energy_rate(mut self, energy_rate: i64) -> Self {
        self.energy_rate = Some(energy_rate);
        self
    }

    pub fn attention_level(mut self, attention_level: i64) -> Self {
        self.attention_level = Some(attention_level);
        self
    }

    pub fn transition_difficulty(mut self, transition_difficulty: i64) -> Self {
        self.transition_difficulty = Some(transition_difficulty);
        self
    }

    pub fn enjoyment_after_start(mut self, enjoyment_after_start: i64) -> Self {
        self.enjoyment_after_start = Some(enjoyment_after_start);
        self
    }

    pub fn urgency_growth(mut self, urgency_growth: bool) -> Self {
        self.urgency_growth = Some(urgency_growth);
        self
    }

    pub fn importance(mut self, importance: i64) -> Self {
        self.importance = Some(importance);
        self
    }
}

/// Additional requirements associated with an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequirement {
    pub id: String,
    pub action_id: String,
    pub requirement_type: String,
    pub value: String,
    pub accessibility_score: Option<i64>,
    pub created_at: Option<String>,
}

impl ActionRequirement {
    pub fn new(
        action_id: impl Into<String>,
        requirement_type: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            action_id: action_id.into(),
            requirement_type: requirement_type.into(),
            value: value.into(),
            accessibility_score: None,
            created_at: None,
        }
    }
}

pub fn insert_action(conn: &Connection, action: &Action) -> Result<()> {
    let urgency_growth = action.urgency_growth.map(|value| if value { 1 } else { 0 });

    conn.execute(
        r#"
            INSERT INTO actions (
                id,
                action_type,
                title,
                description,
                duration_bucket,
                energy_rate,
                attention_level,
                transition_difficulty,
                enjoyment_after_start,
                importance,
                urgency_growth,
                preferred_time_of_day,
                metadata,
                created_at
            )
            VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                COALESCE(?14, datetime('now'))
            )
            ON CONFLICT(id) DO UPDATE SET
                action_type = excluded.action_type,
                title = excluded.title,
                description = excluded.description,
                duration_bucket = excluded.duration_bucket,
                energy_rate = excluded.energy_rate,
                attention_level = excluded.attention_level,
                transition_difficulty = excluded.transition_difficulty,
                enjoyment_after_start = excluded.enjoyment_after_start,
                importance = excluded.importance,
                urgency_growth = excluded.urgency_growth,
                preferred_time_of_day = excluded.preferred_time_of_day,
                metadata = excluded.metadata
        "#,
        (
            &action.id,
            &action.action_type,
            &action.title,
            &action.description,
            action.duration_bucket,
            action.energy_rate,
            action.attention_level,
            action.transition_difficulty,
            action.enjoyment_after_start,
            action.importance,
            urgency_growth,
            &action.preferred_time_of_day,
            &action.metadata,
            &action.created_at,
        ),
    )
    .context("Failed to insert or update action")?;
    Ok(())
}

pub fn fetch_actions(conn: &Connection) -> Result<Vec<Action>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                action_type,
                title,
                description,
                duration_bucket,
                energy_rate,
                attention_level,
                transition_difficulty,
                enjoyment_after_start,
                importance,
                urgency_growth,
                created_at,
                preferred_time_of_day,
                metadata
            FROM actions
            ORDER BY created_at DESC
        "#,
        )
        .context("Failed to prepare action fetch query")?;

    let actions = stmt
        .query_map([], |row| {
            let urgency_growth: Option<i64> = row.get(10)?;
            Ok(Action {
                id: row.get(0)?,
                action_type: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                duration_bucket: row.get(4)?,
                energy_rate: row.get(5)?,
                attention_level: row.get(6)?,
                transition_difficulty: row.get(7)?,
                enjoyment_after_start: row.get(8)?,
                importance: row.get(9)?,
                urgency_growth: urgency_growth.map(|v| v != 0),
                created_at: row.get(11)?,
                preferred_time_of_day: row.get(12)?,
                metadata: row.get(13)?,
            })
        })
        .context("Failed to query actions")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to map action rows")?;

    Ok(actions)
}

pub fn delete_action(conn: &Connection, action_id: &str) -> Result<()> {
    conn.execute(
        r#"
            DELETE FROM actions
            WHERE id = ?1
        "#,
        [action_id],
    )
    .with_context(|| format!("Failed to delete action '{}'", action_id))?;
    Ok(())
}
