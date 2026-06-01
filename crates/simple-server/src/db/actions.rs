use anyhow::Context as _;
use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

use simple_core::{Action, ActionState, ActionTarget, RecurrenceRule, RecurrenceUnit};

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

fn state_from_row(row: &PgRow) -> Result<ActionState, sqlx::Error> {
    let state_str: String = row.try_get("state")?;
    match state_str.as_str() {
        "queued" => {
            let time: DateTime<Utc> = row.try_get("target")?;
            let is_static: bool = row.try_get("target_static")?;
            Ok(ActionState::Queued(ActionTarget { time, is_static }))
        }
        "backlogged" => Ok(ActionState::Backlogged(row.try_get("naive_date")?)),
        "completed" => Ok(ActionState::Completed(row.try_get("completed_at")?)),
        "skipped" => Ok(ActionState::Skipped),
        other => Err(sqlx::Error::Decode(
            format!("unknown action state: '{other}'").into(),
        )),
    }
}

struct ActionRow(Action);

impl<'r> sqlx::FromRow<'r, PgRow> for ActionRow {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(ActionRow(Action {
            id: row.try_get("id")?,
            lineage_id: row.try_get("lineage_id")?,
            origin_routine_id: row.try_get("origin_routine_id")?,
            title: row.try_get("title")?,
            content: row.try_get("content")?,
            duration: row
                .try_get::<Option<i64>, _>("duration_secs")?
                .map(Duration::seconds),
            recurrence: recurrence_from_row(row)?,
            saved: row.try_get("saved")?,
            state: state_from_row(row)?,
        }))
    }
}

const ACTION_COLS: &str = r#"
  id, lineage_id, origin_routine_id, title, content,
  state, target, target_static, naive_date, completed_at,
  duration_secs, recurrence_count, recurrence_unit, saved
"#;

pub(crate) async fn upsert(pool: &PgPool, action: &Action) -> anyhow::Result<()> {
    let (state_str, target, target_static, naive_date, completed_at) = match action.state {
        ActionState::Queued(t) => ("queued", Some(t.time), t.is_static, None, None),
        ActionState::Backlogged(d) => ("backlogged", None, false, d, None),
        ActionState::Completed(at) => ("completed", None, false, None, Some(at)),
        ActionState::Skipped => ("skipped", None, false, None, None),
    };

    sqlx::query(
        r#"
        INSERT INTO actions (
          id, lineage_id, origin_routine_id, title, content,
          state, target, target_static, naive_date, completed_at,
          duration_secs, recurrence_count, recurrence_unit, saved
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        ON CONFLICT (id) DO UPDATE SET
          lineage_id        = EXCLUDED.lineage_id,
          origin_routine_id = EXCLUDED.origin_routine_id,
          title             = EXCLUDED.title,
          content           = EXCLUDED.content,
          state             = EXCLUDED.state,
          target            = EXCLUDED.target,
          target_static     = EXCLUDED.target_static,
          naive_date        = EXCLUDED.naive_date,
          completed_at      = EXCLUDED.completed_at,
          duration_secs     = EXCLUDED.duration_secs,
          recurrence_count  = EXCLUDED.recurrence_count,
          recurrence_unit   = EXCLUDED.recurrence_unit,
          saved             = EXCLUDED.saved,
          updated_at        = NOW()
    "#,
    )
    .bind(action.id)
    .bind(action.lineage_id)
    .bind(action.origin_routine_id)
    .bind(&action.title)
    .bind(&action.content)
    .bind(state_str)
    .bind(target)
    .bind(target_static)
    .bind(naive_date)
    .bind(completed_at)
    .bind(action.duration.map(|d: chrono::Duration| d.num_seconds()))
    .bind(action.recurrence.map(|r| r.count as i32))
    .bind(action.recurrence.map(|r| r.unit.as_str()))
    .bind(action.saved)
    .execute(pool)
    .await
    .context("upsert action")?;

    Ok(())
}

pub(crate) async fn fetch_all(pool: &PgPool) -> anyhow::Result<Vec<Action>> {
    sqlx::query_as::<sqlx::Postgres, ActionRow>(&format!(
        "SELECT {ACTION_COLS} FROM actions WHERE deleted = FALSE ORDER BY created_at"
    ))
    .fetch_all(pool)
    .await
    .context("fetch all actions")
    .map(|rows| rows.into_iter().map(|r| r.0).collect())
}

pub(crate) async fn fetch_by_id(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<Action>> {
    sqlx::query_as::<sqlx::Postgres, ActionRow>(&format!(
        "SELECT {ACTION_COLS} FROM actions WHERE id = $1 AND deleted = FALSE"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("fetch action by id")
    .map(|opt| opt.map(|r| r.0))
}

pub(crate) async fn soft_delete(pool: &PgPool, id: Uuid) -> anyhow::Result<bool> {
    let rows = sqlx::query(
        "UPDATE actions SET deleted = TRUE, updated_at = NOW() WHERE id = $1 AND deleted = FALSE",
    )
    .bind(id)
    .execute(pool)
    .await
    .context("soft delete action")?
    .rows_affected();
    Ok(rows > 0)
}
