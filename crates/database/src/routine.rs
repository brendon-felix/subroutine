use std::fmt;

use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::action::fetch_action_by_id;
use crate::instance::{Instance, insert_instance};
use crate::pipeline::{
    DEFAULT_PIPELINE_ID, PipelineItem, insert_pipeline_item, next_pipeline_position,
};

/// Template grouping actions into sequences (ordered or randomizable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_sequential: bool,
    pub allow_randomization: bool,
    pub default_start_time: Option<String>,
    pub default_end_time: Option<String>,
    pub created_at: Option<String>,
}

impl fmt::Display for Routine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;

        if let Some(ref description) = self.description {
            write!(f, " - {}", description)?;
        }

        let mut details = Vec::new();

        if self.is_sequential {
            details.push("sequential");
        } else {
            details.push("parallel");
        }

        if self.allow_randomization {
            details.push("randomizable");
        }

        if self.default_start_time.is_some() {
            details.push("scheduled");
        }

        if !details.is_empty() {
            write!(f, " ({})", details.join(", "))?;
        }

        Ok(())
    }
}

impl Routine {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: None,
            is_sequential: true,
            allow_randomization: false,
            default_start_time: None,
            default_end_time: None,
            created_at: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn is_sequential(mut self, is_sequential: bool) -> Self {
        self.is_sequential = is_sequential;
        self
    }

    pub fn allow_randomization(mut self, allow_randomization: bool) -> Self {
        self.allow_randomization = allow_randomization;
        self
    }

    pub fn default_start_time(mut self, time: impl Into<String>) -> Self {
        self.default_start_time = Some(time.into());
        self
    }

    pub fn default_end_time(mut self, time: impl Into<String>) -> Self {
        self.default_end_time = Some(time.into());
        self
    }
}

/// Steps belonging to routines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineStep {
    pub id: String,
    pub routine_id: String,
    pub action_id: String,
    pub step_order: i64,
    pub min_duration_bucket: Option<i64>,
    pub max_duration_bucket: Option<i64>,
    pub created_at: Option<String>,
    /// Populated when fetching steps with action details
    pub action_title: Option<String>,
}

impl fmt::Display for RoutineStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Step {}", self.step_order)?;

        if let Some(ref title) = self.action_title {
            write!(f, ": {}", title)?;
        }

        let mut details = Vec::new();

        if let Some(min) = self.min_duration_bucket {
            if let Some(max) = self.max_duration_bucket {
                details.push(format!("duration: {}-{}min", min, max));
            } else {
                details.push(format!("min: {}min", min));
            }
        } else if let Some(max) = self.max_duration_bucket {
            details.push(format!("max: {}min", max));
        }

        if !details.is_empty() {
            write!(f, " ({})", details.join(", "))?;
        }

        Ok(())
    }
}

impl RoutineStep {
    pub fn new(
        routine_id: impl Into<String>,
        action_id: impl Into<String>,
        step_order: i64,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            routine_id: routine_id.into(),
            action_id: action_id.into(),
            step_order,
            min_duration_bucket: None,
            max_duration_bucket: None,
            created_at: None,
            action_title: None,
        }
    }

    pub fn min_duration_bucket(mut self, min: i64) -> Self {
        self.min_duration_bucket = Some(min);
        self
    }

    pub fn max_duration_bucket(mut self, max: i64) -> Self {
        self.max_duration_bucket = Some(max);
        self
    }
}

/// Insert or update a routine.
pub fn insert_routine(conn: &Connection, routine: &Routine) -> Result<()> {
    let is_sequential = if routine.is_sequential { 1 } else { 0 };
    let allow_randomization = if routine.allow_randomization { 1 } else { 0 };

    conn.execute(
        r#"
            INSERT INTO routines (
                id,
                name,
                description,
                is_sequential,
                allow_randomization,
                default_start_time,
                default_end_time,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE(?8, datetime('now')))
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                is_sequential = excluded.is_sequential,
                allow_randomization = excluded.allow_randomization,
                default_start_time = excluded.default_start_time,
                default_end_time = excluded.default_end_time
        "#,
        (
            &routine.id,
            &routine.name,
            &routine.description,
            is_sequential,
            allow_randomization,
            &routine.default_start_time,
            &routine.default_end_time,
            &routine.created_at,
        ),
    )
    .context("Failed to insert or update routine")?;

    Ok(())
}

/// Fetch all routines ordered by created_at DESC.
pub fn fetch_routines(conn: &Connection) -> Result<Vec<Routine>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                name,
                description,
                is_sequential,
                allow_randomization,
                default_start_time,
                default_end_time,
                created_at
            FROM routines
            ORDER BY created_at DESC
        "#,
        )
        .context("Failed to prepare routines fetch query")?;

    let routines = stmt
        .query_map([], |row| {
            let is_sequential: i64 = row.get(3)?;
            let allow_randomization: i64 = row.get(4)?;
            Ok(Routine {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                is_sequential: is_sequential != 0,
                allow_randomization: allow_randomization != 0,
                default_start_time: row.get(5)?,
                default_end_time: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .context("Failed to query routines")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to map routine rows")?;

    Ok(routines)
}

/// Fetch a single routine by ID.
pub fn fetch_routine_by_id(conn: &Connection, routine_id: &str) -> Result<Option<Routine>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                name,
                description,
                is_sequential,
                allow_randomization,
                default_start_time,
                default_end_time,
                created_at
            FROM routines
            WHERE id = ?1
        "#,
        )
        .context("Failed to prepare routine fetch query")?;

    let mut rows = stmt
        .query_map([routine_id], |row| {
            let is_sequential: i64 = row.get(3)?;
            let allow_randomization: i64 = row.get(4)?;
            Ok(Routine {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                is_sequential: is_sequential != 0,
                allow_randomization: allow_randomization != 0,
                default_start_time: row.get(5)?,
                default_end_time: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .context("Failed to query routine by ID")?;

    match rows.next() {
        Some(result) => Ok(Some(result.context("Failed to map routine row")?)),
        None => Ok(None),
    }
}

/// Delete a routine by ID. Steps cascade automatically due to foreign key constraint.
pub fn delete_routine(conn: &Connection, routine_id: &str) -> Result<()> {
    let deleted = conn
        .execute(
            r#"
            DELETE FROM routines
            WHERE id = ?1
        "#,
            [routine_id],
        )
        .with_context(|| format!("Failed to delete routine '{}'", routine_id))?;

    if deleted == 0 {
        return Err(anyhow::anyhow!("Routine '{}' not found", routine_id));
    }

    Ok(())
}

/// Get the next step_order value for a routine (max + 1, or 1 if no steps exist).
pub fn next_routine_step_order(conn: &Connection, routine_id: &str) -> Result<i64> {
    let max_order: Option<i64> = conn
        .query_row(
            r#"
            SELECT MAX(step_order)
            FROM routine_steps
            WHERE routine_id = ?1
        "#,
            [routine_id],
            |row| row.get(0),
        )
        .context("Failed to query max step_order")?;

    Ok(max_order.unwrap_or(0) + 1)
}

/// Shift step orders at or after a given position by a delta (usually +1 or -1).
pub fn shift_routine_steps(
    conn: &Connection,
    routine_id: &str,
    from_position: i64,
    delta: i64,
) -> Result<()> {
    conn.execute(
        r#"
            UPDATE routine_steps
            SET step_order = step_order + ?3
            WHERE routine_id = ?1 AND step_order >= ?2
        "#,
        (routine_id, from_position, delta),
    )
    .context("Failed to shift routine steps")?;

    Ok(())
}

/// Insert a routine step. If position is specified, shifts existing steps.
pub fn insert_routine_step(conn: &Connection, step: &RoutineStep) -> Result<()> {
    conn.execute(
        r#"
            INSERT INTO routine_steps (
                id,
                routine_id,
                action_id,
                step_order,
                min_duration_bucket,
                max_duration_bucket,
                created_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE(?7, datetime('now')))
        "#,
        (
            &step.id,
            &step.routine_id,
            &step.action_id,
            step.step_order,
            step.min_duration_bucket,
            step.max_duration_bucket,
            &step.created_at,
        ),
    )
    .context("Failed to insert routine step")?;

    Ok(())
}

/// Fetch all steps for a routine, ordered by step_order, with action titles.
pub fn fetch_routine_steps(conn: &Connection, routine_id: &str) -> Result<Vec<RoutineStep>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                rs.id,
                rs.routine_id,
                rs.action_id,
                rs.step_order,
                rs.min_duration_bucket,
                rs.max_duration_bucket,
                rs.created_at,
                a.title
            FROM routine_steps rs
            LEFT JOIN actions a ON rs.action_id = a.id
            WHERE rs.routine_id = ?1
            ORDER BY rs.step_order ASC
        "#,
        )
        .context("Failed to prepare routine steps fetch query")?;

    let steps = stmt
        .query_map([routine_id], |row| {
            Ok(RoutineStep {
                id: row.get(0)?,
                routine_id: row.get(1)?,
                action_id: row.get(2)?,
                step_order: row.get(3)?,
                min_duration_bucket: row.get(4)?,
                max_duration_bucket: row.get(5)?,
                created_at: row.get(6)?,
                action_title: row.get(7)?,
            })
        })
        .context("Failed to query routine steps")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to map routine step rows")?;

    Ok(steps)
}

/// Fetch a single routine step by ID.
pub fn fetch_routine_step_by_id(conn: &Connection, step_id: &str) -> Result<Option<RoutineStep>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                rs.id,
                rs.routine_id,
                rs.action_id,
                rs.step_order,
                rs.min_duration_bucket,
                rs.max_duration_bucket,
                rs.created_at,
                a.title
            FROM routine_steps rs
            LEFT JOIN actions a ON rs.action_id = a.id
            WHERE rs.id = ?1
        "#,
        )
        .context("Failed to prepare routine step fetch query")?;

    let mut rows = stmt
        .query_map([step_id], |row| {
            Ok(RoutineStep {
                id: row.get(0)?,
                routine_id: row.get(1)?,
                action_id: row.get(2)?,
                step_order: row.get(3)?,
                min_duration_bucket: row.get(4)?,
                max_duration_bucket: row.get(5)?,
                created_at: row.get(6)?,
                action_title: row.get(7)?,
            })
        })
        .context("Failed to query routine step by ID")?;

    match rows.next() {
        Some(result) => Ok(Some(result.context("Failed to map routine step row")?)),
        None => Ok(None),
    }
}

/// Find a routine step by routine_id and step_order.
pub fn fetch_routine_step_by_order(
    conn: &Connection,
    routine_id: &str,
    step_order: i64,
) -> Result<Option<RoutineStep>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                rs.id,
                rs.routine_id,
                rs.action_id,
                rs.step_order,
                rs.min_duration_bucket,
                rs.max_duration_bucket,
                rs.created_at,
                a.title
            FROM routine_steps rs
            LEFT JOIN actions a ON rs.action_id = a.id
            WHERE rs.routine_id = ?1 AND rs.step_order = ?2
        "#,
        )
        .context("Failed to prepare routine step fetch query")?;

    let mut rows = stmt
        .query_map((routine_id, step_order), |row| {
            Ok(RoutineStep {
                id: row.get(0)?,
                routine_id: row.get(1)?,
                action_id: row.get(2)?,
                step_order: row.get(3)?,
                min_duration_bucket: row.get(4)?,
                max_duration_bucket: row.get(5)?,
                created_at: row.get(6)?,
                action_title: row.get(7)?,
            })
        })
        .context("Failed to query routine step by order")?;

    match rows.next() {
        Some(result) => Ok(Some(result.context("Failed to map routine step row")?)),
        None => Ok(None),
    }
}

/// Delete a routine step by ID and re-order remaining steps.
pub fn delete_routine_step(conn: &Connection, step_id: &str) -> Result<()> {
    // First get the step to know its routine_id and step_order
    let step = fetch_routine_step_by_id(conn, step_id)?
        .ok_or_else(|| anyhow::anyhow!("Routine step '{}' not found", step_id))?;

    // Delete the step
    conn.execute(
        r#"
            DELETE FROM routine_steps
            WHERE id = ?1
        "#,
        [step_id],
    )
    .with_context(|| format!("Failed to delete routine step '{}'", step_id))?;

    // Shift remaining steps down to fill the gap
    shift_routine_steps(conn, &step.routine_id, step.step_order + 1, -1)?;

    Ok(())
}

/// Delete a routine step by routine_id and step_order.
pub fn delete_routine_step_by_order(
    conn: &Connection,
    routine_id: &str,
    step_order: i64,
) -> Result<()> {
    let step = fetch_routine_step_by_order(conn, routine_id, step_order)?
        .ok_or_else(|| anyhow::anyhow!("Step {} not found in routine", step_order))?;

    delete_routine_step(conn, &step.id)
}

/// Count the number of steps in a routine.
pub fn count_routine_steps(conn: &Connection, routine_id: &str) -> Result<i64> {
    let count: i64 = conn
        .query_row(
            r#"
            SELECT COUNT(*)
            FROM routine_steps
            WHERE routine_id = ?1
        "#,
            [routine_id],
            |row| row.get(0),
        )
        .context("Failed to count routine steps")?;

    Ok(count)
}

/// Result of instantiating a routine into the pipeline.
#[derive(Debug, Clone)]
pub struct RoutineInstantiationResult {
    /// The routine that was instantiated
    pub routine: Routine,
    /// The instances created, paired with their pipeline items and action titles
    pub created_items: Vec<(Instance, PipelineItem, String)>,
    /// Whether randomization was applied
    pub was_randomized: bool,
}

/// Options for instantiating a routine.
#[derive(Debug, Clone, Default)]
pub struct InstantiateRoutineOptions {
    /// Override randomization setting (if None, uses routine's allow_randomization)
    pub randomize: Option<bool>,
    /// Starting position in the pipeline (if None, appends to end)
    pub start_position: Option<i64>,
    /// Pipeline ID to add items to (if None, uses default pipeline)
    pub pipeline_id: Option<String>,
}

/// Instantiate a routine by creating instances for all its steps and adding them to the pipeline.
///
/// This function:
/// 1. Fetches all steps for the routine
/// 2. Optionally randomizes the step order (based on routine settings or override)
/// 3. Creates an Instance for each step's action
/// 4. Adds each instance to the pipeline at sequential positions
/// 5. Returns information about what was created
pub fn instantiate_routine(
    conn: &Connection,
    routine: &Routine,
    options: InstantiateRoutineOptions,
) -> Result<RoutineInstantiationResult> {
    let mut steps = fetch_routine_steps(conn, &routine.id)?;

    if steps.is_empty() {
        return Ok(RoutineInstantiationResult {
            routine: routine.clone(),
            created_items: Vec::new(),
            was_randomized: false,
        });
    }

    let should_randomize = options.randomize.unwrap_or(routine.allow_randomization);
    if should_randomize {
        let mut rng = rand::rng();
        steps.shuffle(&mut rng);
    }

    let pipeline_id = options
        .pipeline_id
        .as_deref()
        .unwrap_or(DEFAULT_PIPELINE_ID);

    let mut current_position = match options.start_position {
        Some(pos) => pos,
        None => next_pipeline_position(conn, pipeline_id)?,
    };

    let mut created_items = Vec::with_capacity(steps.len());

    for step in &steps {
        let action = fetch_action_by_id(conn, &step.action_id)?
            .ok_or_else(|| anyhow::anyhow!("Action {} not found for step", step.action_id))?;

        let mut instance = Instance::new(&action.id);
        instance.source = Some("routine".to_string());

        insert_instance(conn, &instance)?;

        let pipeline_item = PipelineItem::new_for_instance(
            pipeline_id,
            &instance.id,
            &action.title,
            current_position,
        );
        insert_pipeline_item(conn, &pipeline_item)?;

        created_items.push((instance, pipeline_item, action.title.clone()));
        current_position += 1;
    }

    Ok(RoutineInstantiationResult {
        routine: routine.clone(),
        created_items,
        was_randomized: should_randomize,
    })
}

/// Instantiate a routine by ID.
pub fn instantiate_routine_by_id(
    conn: &Connection,
    routine_id: &str,
    options: InstantiateRoutineOptions,
) -> Result<RoutineInstantiationResult> {
    let routine = fetch_routine_by_id(conn, routine_id)?
        .ok_or_else(|| anyhow::anyhow!("Routine '{}' not found", routine_id))?;

    instantiate_routine(conn, &routine, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, insert_action};
    use rusqlite::Connection;
    use rusqlite_migration::{M, Migrations};

    fn setup_test_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("Failed to open in-memory database");
        let migrations = Migrations::new(vec![
            M::up(include_str!("../migrations/20260128205548_init_schema.sql")),
            M::up(include_str!(
                "../migrations/20260216134152_add_event_type.sql"
            )),
        ]);
        migrations
            .to_latest(&mut conn)
            .expect("Failed to apply migrations");
        conn
    }

    fn create_test_action(conn: &Connection, title: &str) -> Action {
        let action = Action::new_task(title);
        insert_action(conn, &action).expect("Failed to insert action");
        action
    }

    #[test]
    fn test_insert_and_fetch_routine() {
        let conn = setup_test_db();

        let routine = Routine::new("Morning Routine")
            .description("Start the day right")
            .is_sequential(true)
            .allow_randomization(false);

        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let routines = fetch_routines(&conn).expect("Failed to fetch routines");
        assert_eq!(routines.len(), 1);
        assert_eq!(routines[0].name, "Morning Routine");
        assert_eq!(
            routines[0].description,
            Some("Start the day right".to_string())
        );
        assert!(routines[0].is_sequential);
        assert!(!routines[0].allow_randomization);
    }

    #[test]
    fn test_fetch_routine_by_id() {
        let conn = setup_test_db();

        let routine = Routine::new("Test Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let fetched = fetch_routine_by_id(&conn, &routine.id)
            .expect("Failed to fetch routine")
            .expect("Routine should exist");

        assert_eq!(fetched.id, routine.id);
        assert_eq!(fetched.name, "Test Routine");
    }

    #[test]
    fn test_delete_routine() {
        let conn = setup_test_db();

        let routine = Routine::new("To Delete");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        delete_routine(&conn, &routine.id).expect("Failed to delete routine");

        let fetched = fetch_routine_by_id(&conn, &routine.id).expect("Failed to fetch routine");
        assert!(fetched.is_none());
    }

    #[test]
    fn test_routine_steps_crud() {
        let conn = setup_test_db();

        let routine = Routine::new("Test Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let action1 = create_test_action(&conn, "Brush teeth");
        let action2 = create_test_action(&conn, "Shower");
        let action3 = create_test_action(&conn, "Get dressed");

        // Add steps
        let step1 = RoutineStep::new(&routine.id, &action1.id, 1);
        let step2 = RoutineStep::new(&routine.id, &action2.id, 2);
        let step3 = RoutineStep::new(&routine.id, &action3.id, 3);

        insert_routine_step(&conn, &step1).expect("Failed to insert step 1");
        insert_routine_step(&conn, &step2).expect("Failed to insert step 2");
        insert_routine_step(&conn, &step3).expect("Failed to insert step 3");

        // Fetch steps
        let steps = fetch_routine_steps(&conn, &routine.id).expect("Failed to fetch steps");
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].step_order, 1);
        assert_eq!(steps[0].action_title, Some("Brush teeth".to_string()));
        assert_eq!(steps[1].step_order, 2);
        assert_eq!(steps[2].step_order, 3);

        // Delete middle step
        delete_routine_step(&conn, &step2.id).expect("Failed to delete step");

        let steps = fetch_routine_steps(&conn, &routine.id).expect("Failed to fetch steps");
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].step_order, 1);
        assert_eq!(steps[1].step_order, 2); // Was 3, now shifted down
    }

    #[test]
    fn test_next_routine_step_order() {
        let conn = setup_test_db();

        let routine = Routine::new("Test Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        // Empty routine should return 1
        let next = next_routine_step_order(&conn, &routine.id).expect("Failed to get next order");
        assert_eq!(next, 1);

        // Add a step
        let action = create_test_action(&conn, "Test Action");
        let step = RoutineStep::new(&routine.id, &action.id, 1);
        insert_routine_step(&conn, &step).expect("Failed to insert step");

        let next = next_routine_step_order(&conn, &routine.id).expect("Failed to get next order");
        assert_eq!(next, 2);
    }

    #[test]
    fn test_shift_routine_steps() {
        let conn = setup_test_db();

        let routine = Routine::new("Test Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let action1 = create_test_action(&conn, "Step 1");
        let action2 = create_test_action(&conn, "Step 2");
        let action3 = create_test_action(&conn, "Step 3");

        let step1 = RoutineStep::new(&routine.id, &action1.id, 1);
        let step2 = RoutineStep::new(&routine.id, &action2.id, 2);
        let step3 = RoutineStep::new(&routine.id, &action3.id, 3);

        insert_routine_step(&conn, &step1).expect("Failed to insert step 1");
        insert_routine_step(&conn, &step2).expect("Failed to insert step 2");
        insert_routine_step(&conn, &step3).expect("Failed to insert step 3");

        // Shift steps 2 and 3 up by 1 (to make room for insertion at position 2)
        shift_routine_steps(&conn, &routine.id, 2, 1).expect("Failed to shift steps");

        let steps = fetch_routine_steps(&conn, &routine.id).expect("Failed to fetch steps");
        assert_eq!(steps[0].step_order, 1);
        assert_eq!(steps[1].step_order, 3); // Was 2
        assert_eq!(steps[2].step_order, 4); // Was 3
    }

    #[test]
    fn test_fetch_routine_step_by_order() {
        let conn = setup_test_db();

        let routine = Routine::new("Test Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let action = create_test_action(&conn, "Test Action");
        let step = RoutineStep::new(&routine.id, &action.id, 1);
        insert_routine_step(&conn, &step).expect("Failed to insert step");

        let fetched = fetch_routine_step_by_order(&conn, &routine.id, 1)
            .expect("Failed to fetch step")
            .expect("Step should exist");

        assert_eq!(fetched.id, step.id);
        assert_eq!(fetched.step_order, 1);
    }

    #[test]
    fn test_routine_cascade_delete() {
        let conn = setup_test_db();

        let routine = Routine::new("Test Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let action = create_test_action(&conn, "Test Action");
        let step = RoutineStep::new(&routine.id, &action.id, 1);
        insert_routine_step(&conn, &step).expect("Failed to insert step");

        // Verify step exists
        let steps = fetch_routine_steps(&conn, &routine.id).expect("Failed to fetch steps");
        assert_eq!(steps.len(), 1);

        // Delete routine
        delete_routine(&conn, &routine.id).expect("Failed to delete routine");

        // Steps should be deleted via cascade
        let steps = fetch_routine_steps(&conn, &routine.id).expect("Failed to fetch steps");
        assert_eq!(steps.len(), 0);
    }

    #[test]
    fn test_step_duration_bounds() {
        let conn = setup_test_db();

        let routine = Routine::new("Test Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let action = create_test_action(&conn, "Timed Task");
        let step = RoutineStep::new(&routine.id, &action.id, 1)
            .min_duration_bucket(5)
            .max_duration_bucket(13);

        insert_routine_step(&conn, &step).expect("Failed to insert step");

        let steps = fetch_routine_steps(&conn, &routine.id).expect("Failed to fetch steps");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].min_duration_bucket, Some(5));
        assert_eq!(steps[0].max_duration_bucket, Some(13));
    }

    #[test]
    fn test_count_routine_steps() {
        let conn = setup_test_db();

        let routine = Routine::new("Test Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        // Empty routine
        let count = count_routine_steps(&conn, &routine.id).expect("Failed to count steps");
        assert_eq!(count, 0);

        // Add steps
        let action1 = create_test_action(&conn, "Step 1");
        let action2 = create_test_action(&conn, "Step 2");

        insert_routine_step(&conn, &RoutineStep::new(&routine.id, &action1.id, 1))
            .expect("Failed to insert step");
        insert_routine_step(&conn, &RoutineStep::new(&routine.id, &action2.id, 2))
            .expect("Failed to insert step");

        let count = count_routine_steps(&conn, &routine.id).expect("Failed to count steps");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_routine_parallel_mode() {
        let conn = setup_test_db();

        let routine = Routine::new("Parallel Tasks")
            .is_sequential(false)
            .allow_randomization(true);

        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let fetched = fetch_routine_by_id(&conn, &routine.id)
            .expect("Failed to fetch routine")
            .expect("Routine should exist");

        assert!(!fetched.is_sequential);
        assert!(fetched.allow_randomization);
    }

    #[test]
    fn test_instantiate_routine_basic() {
        let conn = setup_test_db();
        crate::pipeline::ensure_default_pipeline(&conn).expect("Failed to create default pipeline");

        let routine = Routine::new("Morning Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let action1 = create_test_action(&conn, "Brush teeth");
        let action2 = create_test_action(&conn, "Make coffee");
        let action3 = create_test_action(&conn, "Check emails");

        let step1 = RoutineStep::new(&routine.id, &action1.id, 1);
        let step2 = RoutineStep::new(&routine.id, &action2.id, 2);
        let step3 = RoutineStep::new(&routine.id, &action3.id, 3);

        insert_routine_step(&conn, &step1).expect("Failed to insert step 1");
        insert_routine_step(&conn, &step2).expect("Failed to insert step 2");
        insert_routine_step(&conn, &step3).expect("Failed to insert step 3");

        let options = InstantiateRoutineOptions::default();
        let result = instantiate_routine(&conn, &routine, options).expect("Failed to instantiate");

        assert_eq!(result.created_items.len(), 3);
        assert!(!result.was_randomized);

        // Check that instances were created with source = "routine"
        for (instance, _pipeline_item, _title) in &result.created_items {
            assert_eq!(instance.source.as_deref(), Some("routine"));
        }

        // Verify pipeline items are in order
        let items =
            crate::pipeline::fetch_pipeline_items(&conn, crate::pipeline::DEFAULT_PIPELINE_ID)
                .expect("Failed to fetch pipeline items");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].position, Some(1));
        assert_eq!(items[1].position, Some(2));
        assert_eq!(items[2].position, Some(3));
    }

    #[test]
    fn test_instantiate_routine_empty() {
        let conn = setup_test_db();
        crate::pipeline::ensure_default_pipeline(&conn).expect("Failed to create default pipeline");

        let routine = Routine::new("Empty Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let options = InstantiateRoutineOptions::default();
        let result = instantiate_routine(&conn, &routine, options).expect("Failed to instantiate");

        assert!(result.created_items.is_empty());
        assert!(!result.was_randomized);
    }

    #[test]
    fn test_instantiate_routine_with_position() {
        let conn = setup_test_db();
        crate::pipeline::ensure_default_pipeline(&conn).expect("Failed to create default pipeline");

        let routine = Routine::new("Test Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let action = create_test_action(&conn, "Test Action");
        let step = RoutineStep::new(&routine.id, &action.id, 1);
        insert_routine_step(&conn, &step).expect("Failed to insert step");

        let options = InstantiateRoutineOptions {
            start_position: Some(10),
            ..Default::default()
        };
        let result = instantiate_routine(&conn, &routine, options).expect("Failed to instantiate");

        assert_eq!(result.created_items.len(), 1);
        let (_instance, pipeline_item, _title) = &result.created_items[0];
        assert_eq!(pipeline_item.position, Some(10));
    }

    #[test]
    fn test_instantiate_routine_randomization_flag() {
        let conn = setup_test_db();
        crate::pipeline::ensure_default_pipeline(&conn).expect("Failed to create default pipeline");

        // Create routine with randomization disabled
        let routine = Routine::new("Sequential Routine").allow_randomization(false);
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let action = create_test_action(&conn, "Test Action");
        let step = RoutineStep::new(&routine.id, &action.id, 1);
        insert_routine_step(&conn, &step).expect("Failed to insert step");

        // Override with randomize = true
        let options = InstantiateRoutineOptions {
            randomize: Some(true),
            ..Default::default()
        };
        let result = instantiate_routine(&conn, &routine, options).expect("Failed to instantiate");

        assert!(result.was_randomized);
    }

    #[test]
    fn test_instantiate_routine_by_id() {
        let conn = setup_test_db();
        crate::pipeline::ensure_default_pipeline(&conn).expect("Failed to create default pipeline");

        let routine = Routine::new("Test Routine");
        insert_routine(&conn, &routine).expect("Failed to insert routine");

        let action = create_test_action(&conn, "Test Action");
        let step = RoutineStep::new(&routine.id, &action.id, 1);
        insert_routine_step(&conn, &step).expect("Failed to insert step");

        let options = InstantiateRoutineOptions::default();
        let result = instantiate_routine_by_id(&conn, &routine.id, options)
            .expect("Failed to instantiate by id");

        assert_eq!(result.created_items.len(), 1);
        assert_eq!(result.routine.id, routine.id);
    }
}
