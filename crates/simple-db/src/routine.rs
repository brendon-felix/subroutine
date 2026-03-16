use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension};
use simple_core::{Routine, RoutineStep};
use uuid::Uuid;

use crate::{DatabaseError, Result};

struct RoutineRow {
    id: String,
    title: String,
    content: Option<String>,
    target: Option<String>,
    recurrence_secs: Option<i64>,
}

impl TryFrom<RoutineRow> for Routine {
    type Error = DatabaseError;

    fn try_from(row: RoutineRow) -> Result<Self> {
        let id =
            Uuid::parse_str(&row.id).map_err(|e| DatabaseError::invalid_uuid(0, &row.id, e))?;

        let target = row
            .target
            .map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| DatabaseError::invalid_datetime(3, &s, e))
            })
            .transpose()?;

        Ok(Routine {
            id,
            title: row.title,
            content: row.content,
            target,
            steps: Vec::new(),
            recurrence: row.recurrence_secs.map(Duration::seconds),
        })
    }
}

impl From<&Routine> for RoutineRow {
    fn from(routine: &Routine) -> Self {
        Self {
            id: routine.id.to_string(),
            title: routine.title.clone(),
            content: routine.content.clone(),
            target: routine.target.map(|t| t.to_rfc3339()),
            recurrence_secs: routine.recurrence.map(|d| d.num_seconds()),
        }
    }
}

fn read_routine_row(row: &rusqlite::Row) -> rusqlite::Result<RoutineRow> {
    Ok(RoutineRow {
        id: row.get(0)?,
        title: row.get(1)?,
        content: row.get(2)?,
        target: row.get(3)?,
        recurrence_secs: row.get(4)?,
    })
}

struct RoutineStepRow {
    title: String,
    duration_secs: Option<i64>,
}

impl From<RoutineStepRow> for RoutineStep {
    fn from(row: RoutineStepRow) -> Self {
        Self {
            title: row.title,
            duration: row.duration_secs.map(Duration::seconds),
        }
    }
}

impl From<&RoutineStep> for RoutineStepRow {
    fn from(step: &RoutineStep) -> Self {
        Self {
            title: step.title.clone(),
            duration_secs: step.duration.map(|d| d.num_seconds()),
        }
    }
}

fn read_step_row(row: &rusqlite::Row) -> rusqlite::Result<RoutineStepRow> {
    Ok(RoutineStepRow {
        title: row.get(0)?,
        duration_secs: row.get(1)?,
    })
}

fn fetch_steps(conn: &Connection, routine_id: &str) -> Result<Vec<RoutineStep>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT title, duration_secs
            FROM routine_steps
            WHERE routine_id = ?1
            ORDER BY position ASC
            "#,
        )
        .map_err(|e| DatabaseError::sqlite("Failed to prepare routine_steps fetch query", e))?;

    let steps = stmt
        .query_map([routine_id], read_step_row)
        .map_err(|e| DatabaseError::sqlite("Failed to query routine_steps", e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| DatabaseError::sqlite("Failed to map routine_steps rows", e))?
        .into_iter()
        .map(RoutineStep::from)
        .collect();

    Ok(steps)
}

fn insert_steps(conn: &Connection, routine_id: &str, steps: &[RoutineStep]) -> Result<()> {
    conn.execute(
        "DELETE FROM routine_steps WHERE routine_id = ?1",
        [routine_id],
    )
    .map_err(|e| DatabaseError::sqlite("Failed to clear routine_steps before insert", e))?;

    for (position, step) in steps.iter().enumerate() {
        let row = RoutineStepRow::from(step);
        conn.execute(
            r#"
            INSERT INTO routine_steps (routine_id, position, title, duration_secs)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            rusqlite::params![routine_id, position as i64, row.title, row.duration_secs],
        )
        .map_err(|e| DatabaseError::sqlite("Failed to insert routine_steps row", e))?;
    }

    Ok(())
}

pub fn upsert_routine(conn: &Connection, routine: &Routine) -> Result<()> {
    let row = RoutineRow::from(routine);
    conn.execute(
        r#"
        INSERT INTO routines (id, title, content, target, recurrence_secs)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(id) DO UPDATE SET
            title           = excluded.title,
            content         = excluded.content,
            target          = excluded.target,
            recurrence_secs = excluded.recurrence_secs
        "#,
        rusqlite::params![
            row.id,
            row.title,
            row.content,
            row.target,
            row.recurrence_secs
        ],
    )
    .map_err(|e| DatabaseError::sqlite("Failed to insert or update routine", e))?;

    insert_steps(conn, &routine.id.to_string(), &routine.steps)?;

    Ok(())
}

pub fn fetch_routines(conn: &Connection) -> Result<Vec<Routine>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, content, target, recurrence_secs
            FROM routines
            ORDER BY title ASC
            "#,
        )
        .map_err(|e| DatabaseError::sqlite("Failed to prepare routine fetch query", e))?;

    let mut routines = stmt
        .query_map([], read_routine_row)
        .map_err(|e| DatabaseError::sqlite("Failed to query routines", e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| DatabaseError::sqlite("Failed to map routine rows", e))?
        .into_iter()
        .map(Routine::try_from)
        .collect::<Result<Vec<_>>>()?;

    for routine in &mut routines {
        routine.steps = fetch_steps(conn, &routine.id.to_string())?;
    }

    Ok(routines)
}

pub fn fetch_routine_by_id(conn: &Connection, id: Uuid) -> Result<Option<Routine>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, title, content, target, recurrence_secs
            FROM routines
            WHERE id = ?1
            "#,
        )
        .map_err(|e| DatabaseError::sqlite("Failed to prepare routine fetch by id query", e))?;

    let routine_row = stmt
        .query_row([id.to_string()], read_routine_row)
        .optional()
        .map_err(|e| DatabaseError::sqlite("Failed to fetch routine by id", e))?;

    routine_row
        .map(|row| {
            let mut routine = Routine::try_from(row)?;
            routine.steps = fetch_steps(conn, &routine.id.to_string())?;
            Ok(routine)
        })
        .transpose()
}

pub fn delete_routine(conn: &Connection, id: Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM routine_steps WHERE routine_id = ?1",
        [id.to_string()],
    )
    .map_err(|e| {
        DatabaseError::sqlite(
            format!("Failed to delete routine_steps for routine '{}'", id),
            e,
        )
    })?;

    conn.execute("DELETE FROM routines WHERE id = ?1", [id.to_string()])
        .map_err(|e| DatabaseError::sqlite(format!("Failed to delete routine '{}'", id), e))?;

    Ok(())
}
