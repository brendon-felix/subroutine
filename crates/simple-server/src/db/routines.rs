use anyhow::Context as _;
use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

use simple_core::{RecurrenceRule, RecurrenceUnit, Routine, RoutineStep};

fn recurrence_from_row(row: &PgRow) -> Result<Option<RecurrenceRule>, sqlx::Error> {
    let count: Option<i32> = row.try_get("recurrence_count")?;
    let unit_str: Option<String> = row.try_get("recurrence_unit")?;
    match (count, unit_str) {
        (Some(count), Some(unit_str)) => {
            let unit = RecurrenceUnit::from_str(&unit_str).ok_or_else(|| {
                sqlx::Error::Decode(format!("unknown recurrence_unit: '{unit_str}'").into())
            })?;
            Ok(Some(RecurrenceRule {
                count: count as u32,
                unit,
            }))
        }
        _ => Ok(None),
    }
}

struct RoutineRow {
    id: Uuid,
    title: String,
    content: Option<String>,
    target: Option<DateTime<Utc>>,
    recurrence: Option<RecurrenceRule>,
}

impl<'r> sqlx::FromRow<'r, PgRow> for RoutineRow {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(RoutineRow {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            content: row.try_get("content")?,
            target: row.try_get("target")?,
            recurrence: recurrence_from_row(row)?,
        })
    }
}

struct StepRow {
    title: String,
    duration_secs: Option<i64>,
}

impl<'r> sqlx::FromRow<'r, PgRow> for StepRow {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(StepRow {
            title: row.try_get("title")?,
            duration_secs: row.try_get("duration_secs")?,
        })
    }
}

fn steps_from_rows(rows: Vec<StepRow>) -> Vec<RoutineStep> {
    rows.into_iter()
        .map(|r| {
            let mut step = RoutineStep::new(r.title);
            if let Some(secs) = r.duration_secs {
                step = step.with_duration(Duration::seconds(secs));
            }
            step
        })
        .collect()
}

async fn fetch_steps(pool: &PgPool, routine_id: Uuid) -> anyhow::Result<Vec<RoutineStep>> {
    let rows = sqlx::query_as::<sqlx::Postgres, StepRow>(
        "SELECT title, duration_secs FROM routine_steps WHERE routine_id = $1 ORDER BY position",
    )
    .bind(routine_id)
    .fetch_all(pool)
    .await
    .context("fetch routine steps")?;
    Ok(steps_from_rows(rows))
}

const ROUTINE_COLS: &str = "id, title, content, target, recurrence_count, recurrence_unit";

pub(crate) async fn upsert(pool: &PgPool, routine: &Routine) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO routines (id, title, content, target, recurrence_count, recurrence_unit)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (id) DO UPDATE SET
          title            = EXCLUDED.title,
          content          = EXCLUDED.content,
          target           = EXCLUDED.target,
          recurrence_count = EXCLUDED.recurrence_count,
          recurrence_unit  = EXCLUDED.recurrence_unit,
          updated_at       = NOW()
    "#,
    )
    .bind(routine.id)
    .bind(&routine.title)
    .bind(&routine.content)
    .bind(routine.target)
    .bind(routine.recurrence.map(|r| r.count as i32))
    .bind(routine.recurrence.map(|r| r.unit.as_str()))
    .execute(pool)
    .await
    .context("upsert routine")?;

    // Replace all steps for this routine.
    sqlx::query("DELETE FROM routine_steps WHERE routine_id = $1")
        .bind(routine.id)
        .execute(pool)
        .await
        .context("delete old routine steps")?;

    for (position, step) in routine.steps.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO routine_steps (id, routine_id, title, duration_secs, position)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(routine.id)
        .bind(&step.title)
        .bind(step.duration.map(|d: chrono::Duration| d.num_seconds()))
        .bind(position as i32)
        .execute(pool)
        .await
        .context("insert routine step")?;
    }

    Ok(())
}

pub(crate) async fn fetch_all(pool: &PgPool) -> anyhow::Result<Vec<Routine>> {
    let rows = sqlx::query_as::<sqlx::Postgres, RoutineRow>(&format!(
        "SELECT {ROUTINE_COLS} FROM routines WHERE deleted = FALSE ORDER BY created_at"
    ))
    .fetch_all(pool)
    .await
    .context("fetch all routines")?;

    let mut routines = Vec::with_capacity(rows.len());
    for row in rows {
        let steps = fetch_steps(pool, row.id).await?;
        routines.push(Routine {
            id: row.id,
            title: row.title,
            content: row.content,
            target: row.target,
            steps,
            recurrence: row.recurrence,
        });
    }
    Ok(routines)
}

pub(crate) async fn fetch_by_id(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<Routine>> {
    let row = sqlx::query_as::<sqlx::Postgres, RoutineRow>(&format!(
        "SELECT {ROUTINE_COLS} FROM routines WHERE id = $1 AND deleted = FALSE"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("fetch routine by id")?;

    match row {
        None => Ok(None),
        Some(row) => {
            let steps = fetch_steps(pool, row.id).await?;
            Ok(Some(Routine {
                id: row.id,
                title: row.title,
                content: row.content,
                target: row.target,
                steps,
                recurrence: row.recurrence,
            }))
        }
    }
}

pub(crate) async fn soft_delete(pool: &PgPool, id: Uuid) -> anyhow::Result<bool> {
    let rows = sqlx::query(
        "UPDATE routines SET deleted = TRUE, updated_at = NOW() WHERE id = $1 AND deleted = FALSE",
    )
    .bind(id)
    .execute(pool)
    .await
    .context("soft delete routine")?
    .rows_affected();
    Ok(rows > 0)
}
