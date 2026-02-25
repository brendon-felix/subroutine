use anyhow::{Context, Result, bail};
use app_core::{Pipeline, PipelineEntry};
use rusqlite::Connection;
use uuid::Uuid;

use crate::{fetch_action_by_id, fetch_event_by_id, fetch_routine_by_id, fetch_subroutine_by_id};

/// Persists the current state of the pipeline to the database. Replaces all
/// existing pipeline_entries rows, then writes one row per backlog and queue
/// entry with its position. Transitions are not stored — they are ephemeral.
pub fn save_pipeline(conn: &Connection, pipeline: &Pipeline) -> Result<()> {
    conn.execute("DELETE FROM pipeline_entries", [])
        .context("Failed to clear pipeline_entries before save")?;

    for (position, entry) in pipeline.backlog().iter().enumerate() {
        insert_pipeline_entry(conn, entry, "backlog", position)?;
    }

    for (position, entry) in pipeline.queue().iter().enumerate() {
        if entry.is_transition() {
            continue;
        }
        insert_pipeline_entry(conn, entry, "queue", position)?;
    }

    Ok(())
}

fn insert_pipeline_entry(
    conn: &Connection,
    entry: &PipelineEntry,
    list: &str,
    position: usize,
) -> Result<()> {
    let entry_type = entry_type_str(entry)?;

    conn.execute(
        r#"
        INSERT INTO pipeline_entries (id, entry_type, entry_id, list, position)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        rusqlite::params![
            Uuid::new_v4().to_string(),
            entry_type,
            entry.id().to_string(),
            list,
            position as i64,
        ],
    )
    .context("Failed to insert pipeline_entry row")?;

    Ok(())
}

fn entry_type_str(entry: &PipelineEntry) -> Result<&'static str> {
    match entry {
        PipelineEntry::Action(_) => Ok("action"),
        PipelineEntry::Event(_) => Ok("event"),
        PipelineEntry::Routine(_) => Ok("routine"),
        PipelineEntry::Subroutine(_) => Ok("subroutine"),
        PipelineEntry::Transition(_) => bail!("Transitions cannot be stored in pipeline_entries"),
    }
}

/// Loads the pipeline from the database, reconstructing all backlog and queue
/// entries in their saved order. Returns a Pipeline with the stored
/// promotion_threshold (currently always the default — threshold persistence
/// is not yet implemented).
pub fn load_pipeline(conn: &Connection) -> Result<Pipeline> {
    struct PipelineRow {
        entry_type: String,
        entry_id: String,
        list: String,
        position: i64,
    }

    let mut stmt = conn
        .prepare(
            r#"
            SELECT entry_type, entry_id, list, position
            FROM pipeline_entries
            ORDER BY list ASC, position ASC
            "#,
        )
        .context("Failed to prepare pipeline_entries fetch query")?;

    let rows = stmt
        .query_map([], |row| {
            Ok(PipelineRow {
                entry_type: row.get(0)?,
                entry_id: row.get(1)?,
                list: row.get(2)?,
                position: row.get(3)?,
            })
        })
        .context("Failed to query pipeline_entries")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to map pipeline_entry rows")?;

    let mut backlog: Vec<(i64, PipelineEntry)> = Vec::new();
    let mut queue: Vec<(i64, PipelineEntry)> = Vec::new();

    for row in rows {
        let id = Uuid::parse_str(&row.entry_id)
            .with_context(|| format!("Invalid entry_id '{}'", row.entry_id))?;

        let entry = fetch_entry_by_type(conn, &row.entry_type, id)
            .with_context(|| {
                format!(
                    "Failed to fetch {} with id '{}'",
                    row.entry_type, row.entry_id
                )
            })?
            .with_context(|| {
                format!(
                    "pipeline_entries references missing {} '{}'",
                    row.entry_type, row.entry_id
                )
            })?;

        match row.list.as_str() {
            "backlog" => backlog.push((row.position, entry)),
            "queue" => queue.push((row.position, entry)),
            other => bail!("Unknown pipeline list '{}' in pipeline_entries", other),
        }
    }

    // Sort by position in case the ORDER BY wasn't sufficient after splitting by list.
    backlog.sort_by_key(|(position, _)| *position);
    queue.sort_by_key(|(position, _)| *position);

    let mut pipeline = Pipeline::new();
    for (_, entry) in backlog {
        pipeline.push(entry)?;
    }
    for (_, entry) in queue {
        let id = entry.id();
        pipeline.push(entry)?;
        pipeline.promote(id)?;
    }

    Ok(pipeline)
}

fn fetch_entry_by_type(
    conn: &Connection,
    entry_type: &str,
    id: Uuid,
) -> Result<Option<PipelineEntry>> {
    match entry_type {
        "action" => fetch_action_by_id(conn, id).map(|opt| opt.map(PipelineEntry::Action)),
        "event" => fetch_event_by_id(conn, id).map(|opt| opt.map(PipelineEntry::Event)),
        "routine" => fetch_routine_by_id(conn, id).map(|opt| opt.map(PipelineEntry::Routine)),
        "subroutine" => {
            fetch_subroutine_by_id(conn, id).map(|opt| opt.map(PipelineEntry::Subroutine))
        }
        other => bail!("Unknown entry_type '{}' in pipeline_entries", other),
    }
}
