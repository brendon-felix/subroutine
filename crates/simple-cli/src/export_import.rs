use anyhow::{Context, Result};
use clap::Subcommand;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use simple_core::{Action, ActionCompletion, Event, Routine};
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum ExportImportCommand {
    /// Export all data to a JSON file
    Export {
        /// Path to write the JSON file (defaults to stdout if omitted)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Import data from a JSON file, upserting into all tables
    Import {
        /// Path to read the JSON file from (defaults to stdin if omitted)
        #[arg(short, long)]
        input: Option<PathBuf>,
    },
}

pub fn handle_export_import(command: &ExportImportCommand, conn: &Connection) -> Result<()> {
    match command {
        ExportImportCommand::Export { output } => export(conn, output.as_deref()),
        ExportImportCommand::Import { input } => import(conn, input.as_deref()),
    }
}

// ── Dump format ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseDump {
    actions: Vec<Action>,
    events: Vec<Event>,
    routines: Vec<Routine>,
    pipeline_entries: Vec<PipelineEntry>,
    completions: Vec<ActionCompletion>,
}

/// A single row from the `pipeline_entries` table, represented faithfully so
/// that ordering and list membership round-trip exactly.
#[derive(Debug, Serialize, Deserialize)]
struct PipelineEntry {
    id: String,
    entry_type: String,
    entry_id: String,
    list: String,
    position: i64,
}

// ── Export ────────────────────────────────────────────────────────────────────

fn export(conn: &Connection, output: Option<&std::path::Path>) -> Result<()> {
    let actions = simple_db::fetch_actions(conn)?;
    let events = simple_db::fetch_events(conn)?;
    let routines = simple_db::fetch_routines(conn)?;
    let pipeline_entries = fetch_pipeline_entries_raw(conn)?;
    let completions = fetch_all_completions(conn)?;

    let dump = DatabaseDump {
        actions,
        events,
        routines,
        pipeline_entries,
        completions,
    };

    let json = serde_json::to_string_pretty(&dump).context("Failed to serialize data to JSON")?;

    match output {
        Some(path) => {
            std::fs::write(path, &json)
                .with_context(|| format!("Failed to write to '{}'", path.display()))?;
            println!("Exported to '{}'.", path.display());
        }
        None => println!("{}", json),
    }

    Ok(())
}

fn fetch_pipeline_entries_raw(conn: &Connection) -> Result<Vec<PipelineEntry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, entry_type, entry_id, list, position \
             FROM pipeline_entries \
             ORDER BY list ASC, position ASC",
        )
        .context("Failed to prepare pipeline_entries query")?;

    let entries = stmt
        .query_map([], |row| {
            Ok(PipelineEntry {
                id: row.get(0)?,
                entry_type: row.get(1)?,
                entry_id: row.get(2)?,
                list: row.get(3)?,
                position: row.get(4)?,
            })
        })
        .context("Failed to query pipeline_entries")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to map pipeline_entry rows")?;

    Ok(entries)
}

fn fetch_all_completions(conn: &Connection) -> Result<Vec<ActionCompletion>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, action_id, lineage_id, completed_at, notes \
             FROM action_completions \
             ORDER BY completed_at ASC",
        )
        .context("Failed to prepare action_completions query")?;

    let completions = stmt
        .query_map([], |row| {
            Ok(CompletionRow {
                id: row.get(0)?,
                action_id: row.get(1)?,
                lineage_id: row.get(2)?,
                completed_at: row.get(3)?,
                notes: row.get(4)?,
            })
        })
        .context("Failed to query action_completions")?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to map action_completion rows")?;

    completions
        .into_iter()
        .map(|row| {
            use chrono::DateTime;
            use uuid::Uuid;

            let id = Uuid::parse_str(&row.id)
                .with_context(|| format!("Invalid completion id '{}'", row.id))?;
            let action_id = Uuid::parse_str(&row.action_id)
                .with_context(|| format!("Invalid action_id '{}'", row.action_id))?;
            let lineage_id = Uuid::parse_str(&row.lineage_id)
                .with_context(|| format!("Invalid lineage_id '{}'", row.lineage_id))?;
            let completed_at = DateTime::parse_from_rfc3339(&row.completed_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .with_context(|| {
                    format!("Invalid completed_at timestamp '{}'", row.completed_at)
                })?;

            Ok(ActionCompletion {
                id,
                action_id,
                lineage_id,
                completed_at,
                notes: row.notes,
            })
        })
        .collect()
}

struct CompletionRow {
    id: String,
    action_id: String,
    lineage_id: String,
    completed_at: String,
    notes: Option<String>,
}

// ── Import ────────────────────────────────────────────────────────────────────

fn import(conn: &Connection, input: Option<&std::path::Path>) -> Result<()> {
    let json = match input {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read '{}'", path.display()))?,
        None => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("Failed to read from stdin")?;
            buf
        }
    };

    let dump: DatabaseDump =
        serde_json::from_str(&json).context("Failed to parse JSON — is the format correct?")?;

    let action_count = dump.actions.len();
    let event_count = dump.events.len();
    let routine_count = dump.routines.len();
    let entry_count = dump.pipeline_entries.len();
    let completion_count = dump.completions.len();

    for action in &dump.actions {
        simple_db::upsert_action(conn, action)
            .with_context(|| format!("Failed to upsert action '{}'", action.id))?;
    }

    for event in &dump.events {
        simple_db::upsert_event(conn, event)
            .with_context(|| format!("Failed to upsert event '{}'", event.id))?;
    }

    for routine in &dump.routines {
        simple_db::upsert_routine(conn, routine)
            .with_context(|| format!("Failed to upsert routine '{}'", routine.id))?;
    }

    insert_pipeline_entries_raw(conn, &dump.pipeline_entries)?;

    for completion in &dump.completions {
        simple_db::insert_action_completion(conn, completion)
            .with_context(|| format!("Failed to upsert completion '{}'", completion.id))?;
    }

    println!(
        "Imported {} action(s), {} event(s), {} routine(s), {} pipeline entry/entries, {} completion(s).",
        action_count, event_count, routine_count, entry_count, completion_count
    );

    Ok(())
}

fn insert_pipeline_entries_raw(conn: &Connection, entries: &[PipelineEntry]) -> Result<()> {
    for entry in entries {
        conn.execute(
            r#"
            INSERT INTO pipeline_entries (id, entry_type, entry_id, list, position)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                entry_type = excluded.entry_type,
                entry_id   = excluded.entry_id,
                list       = excluded.list,
                position   = excluded.position
            "#,
            rusqlite::params![
                entry.id,
                entry.entry_type,
                entry.entry_id,
                entry.list,
                entry.position,
            ],
        )
        .with_context(|| format!("Failed to upsert pipeline_entry '{}'", entry.id))?;
    }
    Ok(())
}
