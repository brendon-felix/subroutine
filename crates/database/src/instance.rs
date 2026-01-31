use std::time::Instant;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    action::Action,
    pipeline::{
        DEFAULT_PIPELINE_ID, PipelineItem, insert_pipeline_item, next_pipeline_position,
        normalize_pipeline_positions,
    },
};

/// An Instance is an Action that is scheduled or in progress.
/// "Scheduled" can mean queued in the pipeline, scheduled for a specific time, or recurring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    /// Unique identifier for the instance
    pub id: String,
    /// The ID of the action this instance is based on
    pub action_id: String,
    /// Current status of the instance (e.g., "pending", "active", "completed", "scheduled")
    pub status: String,
    /// Optional scheduled start timestamp (ISO-8601 stored as TEXT)
    pub scheduled_start: Option<String>,
    /// Optional scheduled end timestamp (ISO-8601 stored as TEXT)
    pub scheduled_end: Option<String>,
    /// ISO-8601 timestamp the row was created (stored as TEXT in SQLite)
    pub created_at: Option<String>,
    /// Source of the instance: 'manual' | 'routine' | 'suggested' | 'generated'
    pub source: Option<String>,
    /// Earliest allowed start time for flexible scheduling (stored as TEXT)
    pub earliest_start: Option<String>,
    /// Latest allowed end time for flexible scheduling (stored as TEXT)
    pub latest_end: Option<String>,
    /// Free-form JSON metadata for future extensions
    pub metadata: Option<String>,
}

impl Instance {
    /// Create a new scheduled instance from an action id.
    pub fn new(action_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            action_id: action_id.into(),
            status: "scheduled".to_string(),
            scheduled_start: None,
            scheduled_end: None,
            created_at: None,
            source: None,
            earliest_start: None,
            latest_end: None,
            metadata: None,
        }
    }
}

pub fn insert_instance(conn: &Connection, instance: &Instance) -> Result<()> {
    conn.execute(
        r#"
            INSERT INTO instances (
                id,
                action_id,
                status,
                scheduled_start,
                scheduled_end,
                created_at,
                source,
                earliest_start,
                latest_end,
                metadata
            )
            VALUES (
                ?1, ?2, ?3, ?4, ?5, COALESCE(?6, datetime('now')),
                ?7, ?8, ?9, ?10
            )
            ON CONFLICT(id) DO UPDATE SET
                action_id = excluded.action_id,
                status = excluded.status,
                scheduled_start = excluded.scheduled_start,
                scheduled_end = excluded.scheduled_end,
                created_at = excluded.created_at,
                source = excluded.source,
                earliest_start = excluded.earliest_start,
                latest_end = excluded.latest_end,
                metadata = excluded.metadata
        "#,
        (
            &instance.id,
            &instance.action_id,
            &instance.status,
            &instance.scheduled_start,
            &instance.scheduled_end,
            &instance.created_at,
            &instance.source,
            &instance.earliest_start,
            &instance.latest_end,
            &instance.metadata,
        ),
    )
    .context("Failed to insert or update instance")?;
    Ok(())
}

pub fn fetch_instances(conn: &Connection) -> Result<Vec<Instance>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                action_id,
                status,
                scheduled_start,
                scheduled_end,
                created_at,
                source,
                earliest_start,
                latest_end,
                metadata
            FROM instances
            ORDER BY created_at DESC
        "#,
        )
        .context("Failed to prepare instance fetch query")?;

    let instances = stmt
        .query_map([], |row| {
            Ok(Instance {
                id: row.get(0)?,
                action_id: row.get(1)?,
                status: row.get(2)?,
                scheduled_start: row.get(3)?,
                scheduled_end: row.get(4)?,
                created_at: row.get(5)?,
                source: row.get(6)?,
                earliest_start: row.get(7)?,
                latest_end: row.get(8)?,
                metadata: row.get(9)?,
            })
        })
        .context("Failed to query instances")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to map instance rows")?;

    Ok(instances)
}

pub fn set_instance_status(conn: &Connection, instance_id: &str, status: &str) -> Result<()> {
    conn.execute(
        r#"
            UPDATE instances
            SET status = ?1
            WHERE id = ?2
        "#,
        (status, instance_id),
    )
    .with_context(|| format!("Failed to update status for instance '{}'", instance_id))?;
    Ok(())
}

pub fn enqueue_instance(
    conn: &Connection,
    instance_id: &str,
    action_title: Option<&str>,
) -> Result<PipelineItem> {
    let position = next_pipeline_position(conn, DEFAULT_PIPELINE_ID)?;
    let pipeline_item = PipelineItem {
        id: Uuid::new_v4().to_string(),
        pipeline_id: Some(DEFAULT_PIPELINE_ID.to_string()),
        item_type: None,
        data: None,
        instance_id: Some(instance_id.to_owned()),
        action_title: action_title.map(|title| title.to_owned()),
        position: Some(position),
        created_at: None,
        metadata: None,
    };
    insert_pipeline_item(conn, &pipeline_item)?;
    Ok(pipeline_item)
}

pub fn create_instance_and_enqueue(
    conn: &Connection,
    action: &Action,
    status: &str,
) -> Result<(Instance, PipelineItem)> {
    let instance = Instance {
        id: Uuid::new_v4().to_string(),
        action_id: action.id.clone(),
        status: status.to_owned(),
        scheduled_start: None,
        scheduled_end: None,
        created_at: None,
        source: None,
        earliest_start: None,
        latest_end: None,
        metadata: None,
    };
    insert_instance(conn, &instance)?;

    let position = next_pipeline_position(conn, DEFAULT_PIPELINE_ID)?;
    let pipeline_item = PipelineItem {
        id: Uuid::new_v4().to_string(),
        pipeline_id: Some(DEFAULT_PIPELINE_ID.to_string()),
        item_type: None,
        data: None,
        instance_id: Some(instance.id.clone()),
        action_title: Some(action.title.clone()),
        position: Some(position),
        created_at: None,
        metadata: None,
    };
    insert_pipeline_item(conn, &pipeline_item)?;

    Ok((instance, pipeline_item))
}

pub fn delete_instance(conn: &Connection, instance_id: &str) -> Result<()> {
    let start = Instant::now();
    println!("[DB] delete_instance starting for {}", instance_id);

    let fetch_start = Instant::now();
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                id,
                pipeline_id,
                item_type,
                data,
                instance_id,
                action_title,
                position,
                created_at,
                metadata
            FROM pipeline_items
            WHERE instance_id = ?1
        "#,
        )
        .context("Failed to prepare pipeline items fetch for instance deletion")?;

    let pipeline_items = stmt
        .query_map([instance_id], |row| {
            Ok(PipelineItem {
                id: row.get(0)?,
                pipeline_id: row.get(1)?,
                item_type: row.get(2)?,
                data: row.get(3)?,
                instance_id: row.get(4)?,
                action_title: row.get(5)?,
                position: row.get(6)?,
                created_at: row.get(7)?,
                metadata: row.get(8)?,
            })
        })
        .context("Failed to query pipeline items for instance deletion")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to map pipeline item rows")?;

    let pipeline_items_count = pipeline_items.len();
    println!(
        "[DB] delete_instance fetched {} pipeline items in {}ms",
        pipeline_items_count,
        fetch_start.elapsed().as_millis()
    );

    let delete_start = Instant::now();
    conn.execute(
        r#"
            DELETE FROM instances
            WHERE id = ?1
        "#,
        [instance_id],
    )
    .with_context(|| format!("Failed to delete instance '{}'", instance_id))?;
    println!(
        "[DB] delete_instance removed instance in {}ms",
        delete_start.elapsed().as_millis()
    );

    let mut pipelines_to_normalize = std::collections::HashSet::new();
    let mut shift_operations = 0usize;
    let mut shift_time_ms = 0u128;

    for item in pipeline_items {
        if let Some(deleted_position) = item.position {
            if let Some(pipeline_id) = item.pipeline_id.clone() {
                let shift_start = Instant::now();
                conn.execute(
                    r#"
                        UPDATE pipeline_items
                        SET position = position - 1
                        WHERE pipeline_id = ?1 AND position > ?2
                    "#,
                    (&pipeline_id, deleted_position),
                )
                .with_context(|| {
                    format!(
                        "Failed to shift positions after deleting instance at position {}",
                        deleted_position
                    )
                })?;
                let elapsed = shift_start.elapsed().as_millis();
                shift_time_ms += elapsed;
                shift_operations += 1;
                println!(
                    "[DB] delete_instance shifted pipeline '{}' past position {} in {}ms",
                    pipeline_id, deleted_position, elapsed
                );

                pipelines_to_normalize.insert(pipeline_id);
            }
        }
    }

    let mut normalize_time_ms = 0u128;
    let mut normalized_count = 0usize;

    for pipeline_id in pipelines_to_normalize {
        let normalize_start = Instant::now();
        normalize_pipeline_positions(conn, &pipeline_id)?;
        let elapsed = normalize_start.elapsed().as_millis();
        normalize_time_ms += elapsed;
        normalized_count += 1;
        println!(
            "[DB] delete_instance normalized pipeline '{}' in {}ms",
            pipeline_id, elapsed
        );
    }

    println!(
        "[DB] delete_instance finished for {} in {}ms (items={}, shifts={}, normalized={}, normalize_time={}ms, shift_time={}ms)",
        instance_id,
        start.elapsed().as_millis(),
        pipeline_items_count,
        shift_operations,
        normalized_count,
        normalize_time_ms,
        shift_time_ms
    );

    Ok(())
}
