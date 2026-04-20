use chrono::Utc;
use rusqlite::Connection;
use simple_core::{Pipeline, QueueItem};
use uuid::Uuid;

use crate::{DatabaseError, Result, upsert_action, upsert_event};

pub fn save_pipeline(conn: &Connection, pipeline: &Pipeline) -> Result<()> {
    for action in &pipeline.backlog {
        upsert_action(conn, action).map_err(|error| {
            DatabaseError::sqlite(
                format!(
                    "Failed to upsert action '{}' during pipeline save",
                    action.id
                ),
                match error {
                    DatabaseError::Sqlite { source, .. } => source,
                    other => return other,
                },
            )
        })?;
    }

    for item in &pipeline.queue {
        match item {
            QueueItem::Action(action) => {
                upsert_action(conn, action).map_err(|error| {
                    DatabaseError::sqlite(
                        format!(
                            "Failed to upsert action '{}' during pipeline save",
                            action.id
                        ),
                        match error {
                            DatabaseError::Sqlite { source, .. } => source,
                            other => return other,
                        },
                    )
                })?;
            }
            QueueItem::Event(event) => {
                upsert_event(conn, event).map_err(|error| {
                    DatabaseError::sqlite(
                        format!("Failed to upsert event '{}' during pipeline save", event.id),
                        match error {
                            DatabaseError::Sqlite { source, .. } => source,
                            other => return other,
                        },
                    )
                })?;
            }
        }
    }

    conn.execute("DELETE FROM pipeline_entries", [])
        .map_err(|error| {
            DatabaseError::sqlite("Failed to clear pipeline_entries before save", error)
        })?;

    for (position, action) in pipeline.backlog.iter().enumerate() {
        insert_pipeline_entry(conn, "action", &action.id.to_string(), "backlog", position)?;
    }

    for (position, item) in pipeline.queue.iter().enumerate() {
        let (entry_type, entry_id) = queue_item_type_and_id(item);
        insert_pipeline_entry(conn, entry_type, &entry_id, "queue", position)?;
    }

    Ok(())
}

fn queue_item_type_and_id(item: &QueueItem) -> (&'static str, String) {
    match item {
        QueueItem::Action(action) => ("action", action.id.to_string()),
        QueueItem::Event(event) => ("event", event.id.to_string()),
    }
}

fn insert_pipeline_entry(
    conn: &Connection,
    entry_type: &str,
    entry_id: &str,
    list: &str,
    position: usize,
) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO pipeline_entries (id, entry_type, entry_id, list, position)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        rusqlite::params![
            Uuid::now_v7().to_string(),
            entry_type,
            entry_id,
            list,
            position as i64,
        ],
    )
    .map_err(|error| DatabaseError::sqlite("Failed to insert pipeline_entry row", error))?;
    Ok(())
}

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
        .map_err(|error| {
            DatabaseError::sqlite("Failed to prepare pipeline_entries fetch query", error)
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok(PipelineRow {
                entry_type: row.get(0)?,
                entry_id: row.get(1)?,
                list: row.get(2)?,
                position: row.get(3)?,
            })
        })
        .map_err(|error| DatabaseError::sqlite("Failed to query pipeline_entries", error))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| DatabaseError::sqlite("Failed to map pipeline_entry rows", error))?;

    let mut backlog_rows: Vec<(i64, PipelineRow)> = Vec::new();
    let mut queue_rows: Vec<(i64, PipelineRow)> = Vec::new();

    for row in rows {
        match row.list.as_str() {
            "backlog" => backlog_rows.push((row.position, row)),
            "queue" => queue_rows.push((row.position, row)),
            other => {
                return Err(DatabaseError::unknown_variant(
                    "pipeline_entries.list",
                    other,
                ));
            }
        }
    }

    backlog_rows.sort_by_key(|(position, _)| *position);
    queue_rows.sort_by_key(|(position, _)| *position);

    let mut pipeline = Pipeline {
        backlog: Vec::new(),
        queue: Vec::new(),
    };

    for (_, row) in backlog_rows {
        let id = Uuid::parse_str(&row.entry_id)
            .map_err(|error| DatabaseError::invalid_uuid(0, &row.entry_id, error))?;

        match row.entry_type.as_str() {
            "action" => {
                let action = crate::fetch_action_by_id(conn, id)?.ok_or_else(|| {
                    DatabaseError::missing_reference("pipeline_entries", "actions", id.to_string())
                })?;
                pipeline.backlog.push(action);
            }
            other => {
                return Err(DatabaseError::unknown_variant(
                    "pipeline_entries.entry_type",
                    other,
                ));
            }
        }
    }

    for (_, row) in queue_rows {
        let id = Uuid::parse_str(&row.entry_id)
            .map_err(|error| DatabaseError::invalid_uuid(0, &row.entry_id, error))?;

        let item = fetch_queue_item(conn, &row.entry_type, id)?.ok_or_else(|| {
            DatabaseError::missing_reference("pipeline_entries", &row.entry_type, id.to_string())
        })?;

        pipeline.queue.push(item);
    }

    Ok(pipeline)
}

pub fn refresh_pipeline(conn: &Connection) -> Result<Pipeline> {
    let mut pipeline = load_pipeline(conn)?;
    let result = pipeline.refresh(Utc::now());

    for event in &result.new_events {
        upsert_event(conn, event).map_err(|error| {
            DatabaseError::sqlite(
                format!(
                    "Failed to insert next recurrence event '{}' during pipeline refresh",
                    event.id
                ),
                match error {
                    DatabaseError::Sqlite { source, .. } => source,
                    other => return other,
                },
            )
        })?;
    }

    for action in &result.new_actions {
        upsert_action(conn, action).map_err(|error| {
            DatabaseError::sqlite(
                format!(
                    "Failed to insert next recurrence action '{}' during pipeline refresh",
                    action.id
                ),
                match error {
                    DatabaseError::Sqlite { source, .. } => source,
                    other => return other,
                },
            )
        })?;
    }

    for action in &result.demoted_actions {
        upsert_action(conn, action).map_err(|error| {
            DatabaseError::sqlite(
                format!(
                    "Failed to upsert demoted action '{}' during pipeline refresh",
                    action.id
                ),
                match error {
                    DatabaseError::Sqlite { source, .. } => source,
                    other => return other,
                },
            )
        })?;
    }

    for action in &result.promoted_actions {
        upsert_action(conn, action).map_err(|error| {
            DatabaseError::sqlite(
                format!(
                    "Failed to upsert promoted action '{}' during pipeline refresh",
                    action.id
                ),
                match error {
                    DatabaseError::Sqlite { source, .. } => source,
                    other => return other,
                },
            )
        })?;
    }

    save_pipeline(conn, &pipeline)?;

    Ok(pipeline)
}

fn fetch_queue_item(conn: &Connection, entry_type: &str, id: Uuid) -> Result<Option<QueueItem>> {
    match entry_type {
        "action" => {
            let action = crate::fetch_action_by_id(conn, id)?;
            Ok(action.map(QueueItem::Action))
        }
        "event" => {
            let event = crate::fetch_event_by_id(conn, id)?;
            Ok(event.map(QueueItem::Event))
        }
        other => Err(DatabaseError::unknown_variant(
            "pipeline_entries.entry_type",
            other,
        )),
    }
}
