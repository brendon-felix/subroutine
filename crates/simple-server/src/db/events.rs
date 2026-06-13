use anyhow::Context as _;
use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

use simple_core::{Event, EventTemplate, RecurrenceRule, RecurrenceUnit};

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

/// wrapper around [`Event`] for sqlx
struct EventRow(Event);

impl<'r> sqlx::FromRow<'r, PgRow> for EventRow {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(EventRow(Event {
            id: row.try_get("id")?,
            lineage_id: row.try_get("lineage_id")?,
            template_id: row.try_get("template_id")?,
            title: row.try_get("title")?,
            content: row.try_get("content")?,
            time: row.try_get::<DateTime<Utc>, _>("starts_at")?,
            duration: Duration::seconds(row.try_get::<i64, _>("duration_secs")?),
            recurrence: recurrence_from_row(row)?,
        }))
    }
}

const EVENT_COLS: &str = r#"
  id,
  lineage_id,
  template_id,
  title,
  content,
  starts_at,
  duration_secs,
  recurrence_count,
  recurrence_unit
"#;

pub(crate) async fn upsert(pool: &PgPool, event: &Event) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO events (
          id,
          lineage_id,
          template_id,
          title,
          content,
          starts_at,
          duration_secs,
          recurrence_count,
          recurrence_unit,
          updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
        ON CONFLICT (id) DO UPDATE SET
          lineage_id       = EXCLUDED.lineage_id,
          template_id      = EXCLUDED.template_id,
          title            = EXCLUDED.title,
          content          = EXCLUDED.content,
          starts_at        = EXCLUDED.starts_at,
          duration_secs    = EXCLUDED.duration_secs,
          recurrence_count = EXCLUDED.recurrence_count,
          recurrence_unit  = EXCLUDED.recurrence_unit,
          updated_at       = NOW()
    "#,
    )
    .bind(event.id)
    .bind(event.lineage_id)
    .bind(event.template_id)
    .bind(&event.title)
    .bind(&event.content)
    .bind(event.time)
    .bind(event.duration.num_seconds())
    .bind(event.recurrence.map(|r| r.count as i32))
    .bind(event.recurrence.map(|r| r.unit.as_str()))
    .execute(pool)
    .await
    .context("upsert event")?;
    Ok(())
}

pub(crate) async fn fetch_all(pool: &PgPool) -> anyhow::Result<Vec<Event>> {
    sqlx::query_as::<sqlx::Postgres, EventRow>(&format!(
        "SELECT {EVENT_COLS} FROM events WHERE deleted = FALSE ORDER BY starts_at"
    ))
    .fetch_all(pool)
    .await
    .context("fetch all events")
    .map(|rows| rows.into_iter().map(|r| r.0).collect())
}

pub(crate) async fn fetch_by_id(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<Event>> {
    sqlx::query_as::<sqlx::Postgres, EventRow>(&format!(
        "SELECT {EVENT_COLS} FROM events WHERE id = $1 AND deleted = FALSE"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("fetch event by id")
    .map(|opt| opt.map(|r| r.0))
}

pub(crate) async fn soft_delete(pool: &PgPool, id: Uuid) -> anyhow::Result<bool> {
    let rows = sqlx::query(
        "UPDATE events SET deleted = TRUE, updated_at = NOW() WHERE id = $1 AND deleted = FALSE",
    )
    .bind(id)
    .execute(pool)
    .await
    .context("soft delete event")?
    .rows_affected();
    Ok(rows > 0)
}

// ── Event templates ──────────────────────────────────────────────────────────

struct EventTemplateRow(pub EventTemplate);

impl<'r> sqlx::FromRow<'r, PgRow> for EventTemplateRow {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(EventTemplateRow(EventTemplate {
            id: row.try_get("id")?,
            lineage_id: row.try_get("lineage_id")?,
            title: row.try_get("title")?,
            content: row.try_get("content")?,
            duration: Duration::seconds(row.try_get::<i64, _>("duration_secs")?),
            recurrence: recurrence_from_row(row)?,
        }))
    }
}

const EVENT_TEMPLATE_COLS: &str = r#"
  id,
  lineage_id,
  title,
  content,
  duration_secs,
  recurrence_count,
  recurrence_unit
"#;

pub(crate) async fn upsert_event_template(
    pool: &PgPool,
    template: &EventTemplate,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO event_templates (
          id,
          lineage_id,
          title,
          content,
          duration_secs,
          recurrence_count,
          recurrence_unit
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (id) DO UPDATE SET
          lineage_id        = EXCLUDED.lineage_id,
          title             = EXCLUDED.title,
          content           = EXCLUDED.content,
          duration_secs     = EXCLUDED.duration_secs,
          recurrence_count  = EXCLUDED.recurrence_count,
          recurrence_unit   = EXCLUDED.recurrence_unit,
          updated_at        = NOW()
    "#,
    )
    .bind(template.id)
    .bind(template.lineage_id)
    .bind(&template.title)
    .bind(&template.content)
    .bind(template.duration.num_seconds())
    .bind(template.recurrence.map(|r| r.count as i32))
    .bind(template.recurrence.map(|r| r.unit.as_str()))
    .execute(pool)
    .await
    .context("upsert event template")?;

    Ok(())
}

pub(crate) async fn fetch_all_event_templates(pool: &PgPool) -> anyhow::Result<Vec<EventTemplate>> {
    sqlx::query_as::<sqlx::Postgres, EventTemplateRow>(&format!(
        "SELECT {EVENT_TEMPLATE_COLS} FROM event_templates WHERE deleted = FALSE ORDER BY created_at"
    ))
    .fetch_all(pool)
    .await
    .context("fetch all event templates")
    .map(|rows| rows.into_iter().map(|r| r.0).collect())
}

pub(crate) async fn fetch_event_template_by_id(
    pool: &PgPool,
    id: Uuid,
) -> anyhow::Result<Option<EventTemplate>> {
    sqlx::query_as::<sqlx::Postgres, EventTemplateRow>(&format!(
        "SELECT {EVENT_TEMPLATE_COLS} FROM event_templates WHERE id = $1 AND deleted = FALSE"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("fetch event template by id")
    .map(|opt| opt.map(|r| r.0))
}

pub(crate) async fn soft_delete_event_template(pool: &PgPool, id: Uuid) -> anyhow::Result<bool> {
    let rows = sqlx::query(
        "UPDATE event_templates SET deleted = TRUE, updated_at = NOW() WHERE id = $1 AND deleted = FALSE",
    )
    .bind(id)
    .execute(pool)
    .await
    .context("soft delete event template")?
    .rows_affected();
    Ok(rows > 0)
}
