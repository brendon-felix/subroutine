use anyhow::{Context, Result};
use app_core::{RecurrenceRule, Routine, SavedConstraints, SavedStep, Subroutine, TimesOfDay};
use chrono::{DateTime, NaiveTime, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// --- Shared helpers ---

fn saved_constraints_from_fields(
    valid_times_of_day: Option<i64>,
    deadline: Option<&str>,
    minimum_duration_secs: Option<i64>,
    transition_time_secs: Option<i64>,
    spoons_required: Option<i64>,
    dependencies: Option<&str>,
) -> Result<SavedConstraints> {
    let deadline = deadline
        .map(|s| {
            NaiveTime::parse_from_str(s, "%H:%M:%S")
                .with_context(|| format!("Invalid deadline time '{}'", s))
        })
        .transpose()?;

    let valid_times_of_day = valid_times_of_day
        .map(|bits| {
            TimesOfDay::from_bits(bits as u8)
                .with_context(|| format!("Invalid valid_times_of_day bits '{}'", bits))
        })
        .transpose()?;

    let dependencies = dependencies
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

    Ok(SavedConstraints {
        valid_times_of_day,
        deadline,
        minimum_duration: minimum_duration_secs.map(chrono::Duration::seconds),
        transition_time: transition_time_secs.map(chrono::Duration::seconds),
        spoons_required: spoons_required.map(|v| v as u32),
        dependencies,
    })
}

fn recurrence_from_fields(
    min_interval_secs: Option<i64>,
    max_interval_secs: Option<i64>,
    auto_reschedule: bool,
) -> Option<RecurrenceRule> {
    let has_recurrence =
        min_interval_secs.is_some() || max_interval_secs.is_some() || auto_reschedule;
    if has_recurrence {
        Some(RecurrenceRule {
            min_interval: min_interval_secs.map(chrono::Duration::seconds),
            max_interval: max_interval_secs.map(chrono::Duration::seconds),
            auto_reschedule,
        })
    } else {
        None
    }
}

fn dependencies_to_json(dependencies: &[Uuid]) -> Option<String> {
    if dependencies.is_empty() {
        None
    } else {
        let ids: Vec<String> = dependencies.iter().map(|id| id.to_string()).collect();
        serde_json::to_string(&ids).ok()
    }
}

// --- Routine ---

/// Flat database representation of a Routine. Steps are loaded separately from
/// the routine_steps junction table, which stores a step_type discriminant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineModel {
    pub id: String,
    pub title: String,
    pub content: Option<String>,
    pub created_at: String,
    // SavedConstraints (inline)
    pub valid_times_of_day: Option<i64>,
    pub deadline: Option<String>,
    pub minimum_duration_secs: Option<i64>,
    pub transition_time_secs: Option<i64>,
    pub spoons_required: Option<i64>,
    pub dependencies: Option<String>,
    // RecurrenceRule (flattened)
    pub recurrence_min_interval_secs: Option<i64>,
    pub recurrence_max_interval_secs: Option<i64>,
    pub recurrence_auto_reschedule: bool,
    // Loaded from routine_steps junction table as (step_type, step_id) pairs
    pub steps: Vec<(String, String)>,
}

fn row_to_routine_model(row: &rusqlite::Row) -> rusqlite::Result<RoutineModel> {
    Ok(RoutineModel {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
        valid_times_of_day: row.get(4)?,
        deadline: row.get(5)?,
        minimum_duration_secs: row.get(6)?,
        transition_time_secs: row.get(7)?,
        spoons_required: row.get(8)?,
        dependencies: row.get(9)?,
        recurrence_min_interval_secs: row.get(10)?,
        recurrence_max_interval_secs: row.get(11)?,
        recurrence_auto_reschedule: row.get::<_, i64>(12)? != 0,
        steps: Vec::new(), // populated separately
    })
}

fn fetch_routine_steps(conn: &Connection, routine_id: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT step_type, step_id FROM routine_steps
            WHERE routine_id = ?1
            ORDER BY position ASC
            "#,
        )
        .context("Failed to prepare routine_steps fetch query")?;

    let steps = stmt
        .query_map([routine_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .context("Failed to query routine_steps")?
        .collect::<rusqlite::Result<Vec<(String, String)>>>()
        .context("Failed to map routine_steps rows")?;

    Ok(steps)
}

fn insert_routine_steps(
    conn: &Connection,
    routine_id: &str,
    steps: &[(String, String)],
) -> Result<()> {
    conn.execute(
        "DELETE FROM routine_steps WHERE routine_id = ?1",
        [routine_id],
    )
    .context("Failed to clear routine_steps before insert")?;

    for (position, (step_type, step_id)) in steps.iter().enumerate() {
        conn.execute(
            r#"
            INSERT INTO routine_steps (routine_id, step_type, step_id, position)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            rusqlite::params![routine_id, step_type, step_id, position as i64],
        )
        .context("Failed to insert routine_steps row")?;
    }

    Ok(())
}

impl From<&Routine> for RoutineModel {
    fn from(routine: &Routine) -> Self {
        Self {
            id: routine.id.to_string(),
            title: routine.title.clone(),
            content: routine.content.clone(),
            created_at: routine.created_at.to_rfc3339(),
            valid_times_of_day: routine
                .constraints
                .valid_times_of_day
                .map(|t| t.bits() as i64),
            deadline: routine
                .constraints
                .deadline
                .map(|t| t.format("%H:%M:%S").to_string()),
            minimum_duration_secs: routine
                .constraints
                .minimum_duration
                .map(|d| d.num_seconds()),
            transition_time_secs: routine.constraints.transition_time.map(|d| d.num_seconds()),
            spoons_required: routine.constraints.spoons_required.map(|v| v as i64),
            dependencies: dependencies_to_json(&routine.constraints.dependencies),
            recurrence_min_interval_secs: routine
                .recurrence
                .as_ref()
                .and_then(|r| r.min_interval)
                .map(|d| d.num_seconds()),
            recurrence_max_interval_secs: routine
                .recurrence
                .as_ref()
                .and_then(|r| r.max_interval)
                .map(|d| d.num_seconds()),
            recurrence_auto_reschedule: routine
                .recurrence
                .as_ref()
                .map_or(false, |r| r.auto_reschedule),
            steps: routine
                .steps
                .iter()
                .map(|step| match step {
                    SavedStep::Action(id) => ("action".to_string(), id.to_string()),
                    SavedStep::Event(id) => ("event".to_string(), id.to_string()),
                })
                .collect(),
        }
    }
}

impl TryFrom<RoutineModel> for Routine {
    type Error = anyhow::Error;

    fn try_from(model: RoutineModel) -> Result<Self> {
        let id = Uuid::parse_str(&model.id)
            .with_context(|| format!("Invalid routine id '{}'", model.id))?;

        let created_at = DateTime::parse_from_rfc3339(&model.created_at)
            .with_context(|| format!("Invalid created_at '{}'", model.created_at))?
            .with_timezone(&Utc);

        let constraints = saved_constraints_from_fields(
            model.valid_times_of_day,
            model.deadline.as_deref(),
            model.minimum_duration_secs,
            model.transition_time_secs,
            model.spoons_required,
            model.dependencies.as_deref(),
        )?;

        let recurrence = recurrence_from_fields(
            model.recurrence_min_interval_secs,
            model.recurrence_max_interval_secs,
            model.recurrence_auto_reschedule,
        );

        let steps = model
            .steps
            .into_iter()
            .map(|(step_type, step_id)| {
                let id = Uuid::parse_str(&step_id)
                    .with_context(|| format!("Invalid step_id '{}'", step_id))?;
                match step_type.as_str() {
                    "action" => Ok(SavedStep::Action(id)),
                    "event" => Ok(SavedStep::Event(id)),
                    other => anyhow::bail!("Unknown step_type '{}'", other),
                }
            })
            .collect::<Result<Vec<SavedStep>>>()?;

        Ok(Routine {
            id,
            title: model.title,
            content: model.content,
            created_at,
            constraints,
            recurrence,
            steps,
        })
    }
}

pub fn insert_routine(conn: &Connection, routine: &Routine) -> Result<()> {
    let model = RoutineModel::from(routine);

    conn.execute(
        r#"
            INSERT INTO routines (
                id, title, content, created_at,
                valid_times_of_day, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies,
                recurrence_min_interval_secs, recurrence_max_interval_secs,
                recurrence_auto_reschedule
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
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
            model.created_at,
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
    .context("Failed to insert or update routine")?;

    insert_routine_steps(conn, &model.id, &model.steps)?;

    Ok(())
}

pub fn fetch_routines(conn: &Connection) -> Result<Vec<Routine>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, title, content, created_at,
                valid_times_of_day, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies,
                recurrence_min_interval_secs, recurrence_max_interval_secs,
                recurrence_auto_reschedule
            FROM routines
            ORDER BY created_at DESC
            "#,
        )
        .context("Failed to prepare routine fetch query")?;

    let models = stmt
        .query_map([], |row| row_to_routine_model(row))
        .context("Failed to query routines")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to map routine rows")?;

    models
        .into_iter()
        .map(|mut model| {
            model.steps = fetch_routine_steps(conn, &model.id)?;
            Routine::try_from(model)
        })
        .collect()
}

pub fn fetch_routine_by_id(conn: &Connection, id: Uuid) -> Result<Option<Routine>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, title, content, created_at,
                valid_times_of_day, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies,
                recurrence_min_interval_secs, recurrence_max_interval_secs,
                recurrence_auto_reschedule
            FROM routines
            WHERE id = ?1
            "#,
        )
        .context("Failed to prepare routine fetch by id query")?;

    let model = stmt
        .query_row([id.to_string()], |row| row_to_routine_model(row))
        .optional()
        .context("Failed to fetch routine by id")?;

    model
        .map(|mut model| {
            model.steps = fetch_routine_steps(conn, &model.id)?;
            Routine::try_from(model)
        })
        .transpose()
}

pub fn delete_routine(conn: &Connection, id: Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM routine_steps WHERE routine_id = ?1",
        [id.to_string()],
    )
    .with_context(|| format!("Failed to delete routine_steps for routine '{}'", id))?;

    conn.execute("DELETE FROM routines WHERE id = ?1", [id.to_string()])
        .with_context(|| format!("Failed to delete routine '{}'", id))?;

    Ok(())
}

// --- Subroutine ---

/// Flat database representation of a Subroutine. Steps are SavedAction IDs only
/// (subroutines do not contain events), loaded from subroutine_steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubroutineModel {
    pub id: String,
    pub title: String,
    pub content: Option<String>,
    pub created_at: String,
    // SavedConstraints (inline)
    pub valid_times_of_day: Option<i64>,
    pub deadline: Option<String>,
    pub minimum_duration_secs: Option<i64>,
    pub transition_time_secs: Option<i64>,
    pub spoons_required: Option<i64>,
    pub dependencies: Option<String>,
    // RecurrenceRule (flattened)
    pub recurrence_min_interval_secs: Option<i64>,
    pub recurrence_max_interval_secs: Option<i64>,
    pub recurrence_auto_reschedule: bool,
    // Loaded from subroutine_steps junction table
    pub steps: Vec<String>,
}

fn row_to_subroutine_model(row: &rusqlite::Row) -> rusqlite::Result<SubroutineModel> {
    Ok(SubroutineModel {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
        valid_times_of_day: row.get(4)?,
        deadline: row.get(5)?,
        minimum_duration_secs: row.get(6)?,
        transition_time_secs: row.get(7)?,
        spoons_required: row.get(8)?,
        dependencies: row.get(9)?,
        recurrence_min_interval_secs: row.get(10)?,
        recurrence_max_interval_secs: row.get(11)?,
        recurrence_auto_reschedule: row.get::<_, i64>(12)? != 0,
        steps: Vec::new(), // populated separately
    })
}

fn fetch_subroutine_steps(conn: &Connection, subroutine_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT saved_action_id FROM subroutine_steps
            WHERE subroutine_id = ?1
            ORDER BY position ASC
            "#,
        )
        .context("Failed to prepare subroutine_steps fetch query")?;

    let ids = stmt
        .query_map([subroutine_id], |row| row.get(0))
        .context("Failed to query subroutine_steps")?
        .collect::<rusqlite::Result<Vec<String>>>()
        .context("Failed to map subroutine_steps rows")?;

    Ok(ids)
}

fn insert_subroutine_steps(conn: &Connection, subroutine_id: &str, steps: &[String]) -> Result<()> {
    conn.execute(
        "DELETE FROM subroutine_steps WHERE subroutine_id = ?1",
        [subroutine_id],
    )
    .context("Failed to clear subroutine_steps before insert")?;

    for (position, saved_action_id) in steps.iter().enumerate() {
        conn.execute(
            r#"
            INSERT INTO subroutine_steps (subroutine_id, saved_action_id, position)
            VALUES (?1, ?2, ?3)
            "#,
            rusqlite::params![subroutine_id, saved_action_id, position as i64],
        )
        .context("Failed to insert subroutine_steps row")?;
    }

    Ok(())
}

impl From<&Subroutine> for SubroutineModel {
    fn from(subroutine: &Subroutine) -> Self {
        Self {
            id: subroutine.id.to_string(),
            title: subroutine.title.clone(),
            content: subroutine.content.clone(),
            created_at: subroutine.created_at.to_rfc3339(),
            valid_times_of_day: subroutine
                .constraints
                .valid_times_of_day
                .map(|t| t.bits() as i64),
            deadline: subroutine
                .constraints
                .deadline
                .map(|t| t.format("%H:%M:%S").to_string()),
            minimum_duration_secs: subroutine
                .constraints
                .minimum_duration
                .map(|d| d.num_seconds()),
            transition_time_secs: subroutine
                .constraints
                .transition_time
                .map(|d| d.num_seconds()),
            spoons_required: subroutine.constraints.spoons_required.map(|v| v as i64),
            dependencies: dependencies_to_json(&subroutine.constraints.dependencies),
            recurrence_min_interval_secs: subroutine
                .recurrence
                .as_ref()
                .and_then(|r| r.min_interval)
                .map(|d| d.num_seconds()),
            recurrence_max_interval_secs: subroutine
                .recurrence
                .as_ref()
                .and_then(|r| r.max_interval)
                .map(|d| d.num_seconds()),
            recurrence_auto_reschedule: subroutine
                .recurrence
                .as_ref()
                .map_or(false, |r| r.auto_reschedule),
            steps: subroutine.steps.iter().map(|id| id.to_string()).collect(),
        }
    }
}

impl TryFrom<SubroutineModel> for Subroutine {
    type Error = anyhow::Error;

    fn try_from(model: SubroutineModel) -> Result<Self> {
        let id = Uuid::parse_str(&model.id)
            .with_context(|| format!("Invalid subroutine id '{}'", model.id))?;

        let created_at = DateTime::parse_from_rfc3339(&model.created_at)
            .with_context(|| format!("Invalid created_at '{}'", model.created_at))?
            .with_timezone(&Utc);

        let constraints = saved_constraints_from_fields(
            model.valid_times_of_day,
            model.deadline.as_deref(),
            model.minimum_duration_secs,
            model.transition_time_secs,
            model.spoons_required,
            model.dependencies.as_deref(),
        )?;

        let recurrence = recurrence_from_fields(
            model.recurrence_min_interval_secs,
            model.recurrence_max_interval_secs,
            model.recurrence_auto_reschedule,
        );

        let steps = model
            .steps
            .iter()
            .map(|s| Uuid::parse_str(s).with_context(|| format!("Invalid step UUID '{}'", s)))
            .collect::<Result<Vec<Uuid>>>()?;

        Ok(Subroutine {
            id,
            title: model.title,
            content: model.content,
            created_at,
            constraints,
            recurrence,
            steps,
        })
    }
}

pub fn insert_subroutine(conn: &Connection, subroutine: &Subroutine) -> Result<()> {
    let model = SubroutineModel::from(subroutine);

    conn.execute(
        r#"
            INSERT INTO subroutines (
                id, title, content, created_at,
                valid_times_of_day, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies,
                recurrence_min_interval_secs, recurrence_max_interval_secs,
                recurrence_auto_reschedule
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
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
            model.created_at,
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
    .context("Failed to insert or update subroutine")?;

    insert_subroutine_steps(conn, &model.id, &model.steps)?;

    Ok(())
}

pub fn fetch_subroutines(conn: &Connection) -> Result<Vec<Subroutine>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, title, content, created_at,
                valid_times_of_day, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies,
                recurrence_min_interval_secs, recurrence_max_interval_secs,
                recurrence_auto_reschedule
            FROM subroutines
            ORDER BY created_at DESC
            "#,
        )
        .context("Failed to prepare subroutine fetch query")?;

    let models = stmt
        .query_map([], |row| row_to_subroutine_model(row))
        .context("Failed to query subroutines")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to map subroutine rows")?;

    models
        .into_iter()
        .map(|mut model| {
            model.steps = fetch_subroutine_steps(conn, &model.id)?;
            Subroutine::try_from(model)
        })
        .collect()
}

pub fn fetch_subroutine_by_id(conn: &Connection, id: Uuid) -> Result<Option<Subroutine>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id, title, content, created_at,
                valid_times_of_day, deadline,
                minimum_duration_secs, transition_time_secs, spoons_required,
                dependencies,
                recurrence_min_interval_secs, recurrence_max_interval_secs,
                recurrence_auto_reschedule
            FROM subroutines
            WHERE id = ?1
            "#,
        )
        .context("Failed to prepare subroutine fetch by id query")?;

    let model = stmt
        .query_row([id.to_string()], |row| row_to_subroutine_model(row))
        .optional()
        .context("Failed to fetch subroutine by id")?;

    model
        .map(|mut model| {
            model.steps = fetch_subroutine_steps(conn, &model.id)?;
            Subroutine::try_from(model)
        })
        .transpose()
}

pub fn delete_subroutine(conn: &Connection, id: Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM subroutine_steps WHERE subroutine_id = ?1",
        [id.to_string()],
    )
    .with_context(|| format!("Failed to delete subroutine_steps for subroutine '{}'", id))?;

    conn.execute("DELETE FROM subroutines WHERE id = ?1", [id.to_string()])
        .with_context(|| format!("Failed to delete subroutine '{}'", id))?;

    Ok(())
}
