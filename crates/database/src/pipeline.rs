use std::{fmt, time::Instant};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    action::Action,
    instance::{Instance, insert_instance},
};

pub const DEFAULT_PIPELINE_ID: &str = "default";

/// An item in the pipeline, representing an instance of an action which is queued for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineItem {
    /// Unique identifier for the pipeline item
    pub id: String,
    /// Optional pipeline identifier this item belongs to
    pub pipeline_id: Option<String>,
    /// Arbitrary type/category of the item
    pub item_type: Option<String>,
    /// Free-form payload or serialized data
    pub data: Option<String>,
    /// The instance id this pipeline item references (if any)
    pub instance_id: Option<String>,
    /// Title of the action associated with this pipeline item (denormalized for convenience)
    pub action_title: Option<String>,
    /// Position of the item in the pipeline (ordering)
    pub position: Option<i64>,
    /// ISO-8601 timestamp the row was created (stored as TEXT in SQLite)
    pub created_at: Option<String>,
    /// Free-form JSON metadata for transient UI hints or reason traces
    pub metadata: Option<String>,
}

impl fmt::Display for PipelineItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref title) = self.action_title {
            write!(f, "{}", title)?;
        } else {
            write!(f, "Pipeline item")?;
        }

        let mut details = Vec::new();

        if let Some(ref item_type) = self.item_type {
            details.push(format!("type: {}", item_type));
        }

        if let Some(position) = self.position {
            details.push(format!("pos: {}", position));
        }

        if let Some(ref pipeline_id) = self.pipeline_id {
            details.push(format!("pipeline: {}", pipeline_id));
        }

        if !details.is_empty() {
            write!(f, " ({})", details.join(", "))?;
        }

        Ok(())
    }
}

impl PipelineItem {
    /// Create a basic pipeline item referencing an instance.
    pub fn new_for_instance(
        pipeline_id: impl Into<String>,
        instance_id: impl Into<String>,
        action_title: impl Into<String>,
        position: i64,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            pipeline_id: Some(pipeline_id.into()),
            item_type: None,
            data: None,
            instance_id: Some(instance_id.into()),
            action_title: Some(action_title.into()),
            position: Some(position),
            created_at: None,
            metadata: None,
        }
    }

    /// Create a new generic pipeline item.
    pub fn new(
        pipeline_id: Option<impl Into<String>>,
        item_type: Option<impl Into<String>>,
        data: Option<impl Into<String>>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            pipeline_id: pipeline_id.map(|p| p.into()),
            item_type: item_type.map(|t| t.into()),
            data: data.map(|d| d.into()),
            instance_id: None,
            action_title: None,
            position: None,
            created_at: None,
            metadata: None,
        }
    }
}

/// A pipeline represents an ordered collection of pipeline items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: Option<String>,
}

impl fmt::Display for Pipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;

        if let Some(ref description) = self.description {
            write!(f, " - {}", description)?;
        }

        Ok(())
    }
}

pub fn fetch_pipeline_items(conn: &Connection, pipeline_id: &str) -> Result<Vec<PipelineItem>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
                pi.id,
                pi.pipeline_id,
                pi.item_type,
                pi.data,
                pi.instance_id,
                a.title AS action_title,
                pi.position,
                pi.created_at,
                pi.metadata
            FROM pipeline_items pi
            LEFT JOIN instances i ON i.id = pi.instance_id
            LEFT JOIN actions a ON a.id = i.action_id
            WHERE pi.pipeline_id = ?
            ORDER BY pi.position ASC, pi.created_at ASC
        "#,
        )
        .with_context(|| format!("Failed to prepare pipeline fetch for '{}'", pipeline_id))?;

    let items = stmt
        .query_map([pipeline_id], |row| {
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
        .with_context(|| format!("Failed to query pipeline '{}'", pipeline_id))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("Failed to map pipeline items for '{}'", pipeline_id))?;

    Ok(items)
}

pub fn next_pipeline_position(conn: &Connection, pipeline_id: &str) -> Result<i64> {
    let position: i64 = conn
        .query_row(
            r#"
            SELECT COALESCE(MAX(position), 0) + 1
            FROM pipeline_items
            WHERE pipeline_id = ?1
        "#,
            [pipeline_id],
            |row| row.get(0),
        )
        .with_context(|| {
            format!(
                "Failed to compute next position for pipeline '{}'",
                pipeline_id
            )
        })?;

    Ok(position)
}

pub fn insert_pipeline_item(conn: &Connection, item: &PipelineItem) -> Result<()> {
    conn.execute(
        r#"
            INSERT INTO pipeline_items (
                id,
                instance_id,
                action_title,
                position,
                pipeline_id,
                item_type,
                data,
                created_at,
                metadata
            )
            VALUES (
                ?1,
                ?2,
                ?3,
                ?4,
                ?5,
                ?6,
                ?7,
                COALESCE(?8, datetime('now')),
                ?9
            )
            ON CONFLICT(id) DO UPDATE SET
                instance_id = excluded.instance_id,
                action_title = excluded.action_title,
                position = excluded.position,
                pipeline_id = excluded.pipeline_id,
                item_type = excluded.item_type,
                data = excluded.data,
                created_at = excluded.created_at,
                metadata = excluded.metadata
        "#,
        (
            &item.id,
            &item.instance_id,
            &item.action_title,
            item.position,
            &item.pipeline_id,
            &item.item_type,
            &item.data,
            &item.created_at,
            &item.metadata,
        ),
    )
    .context("Failed to insert or update pipeline item")?;
    Ok(())
}

pub fn delete_pipeline_item(conn: &Connection, pipeline_item_id: &str) -> Result<()> {
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
            WHERE id = ?1
        "#,
        )
        .context("Failed to prepare pipeline item fetch for deletion")?;

    let item = stmt
        .query_row([pipeline_item_id], |row| {
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
        .optional()
        .context("Failed to fetch pipeline item for deletion")?;

    if let Some(item) = item {
        conn.execute(
            r#"
                DELETE FROM pipeline_items
                WHERE id = ?1
            "#,
            [pipeline_item_id],
        )
        .with_context(|| format!("Failed to delete pipeline item '{}'", pipeline_item_id))?;

        if let Some(deleted_position) = item.position {
            if let Some(pipeline_id) = item.pipeline_id {
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
                        "Failed to shift positions after deleting pipeline item at position {}",
                        deleted_position
                    )
                })?;
            }
        }
    }

    Ok(())
}

pub fn ensure_default_pipeline(conn: &Connection) -> Result<()> {
    conn.execute(
        r#"
            INSERT INTO pipelines (id, name, created_at)
            VALUES (?1, ?2, datetime('now'))
            ON CONFLICT(id) DO NOTHING
        "#,
        (DEFAULT_PIPELINE_ID, "Main"),
    )
    .context("Failed to ensure default pipeline exists")?;
    Ok(())
}

pub fn insert_instance_at_position(
    conn: &Connection,
    action: &Action,
    status: &str,
    pipeline_id: &str,
    position: i64,
) -> Result<(Instance, PipelineItem)> {
    let start = Instant::now();
    println!(
        "[DB] insert_instance_at_position starting for action {} at requested position {}",
        action.id, position
    );

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

    let insert_instance_start = Instant::now();
    insert_instance(conn, &instance)?;
    println!(
        "[DB] insert_instance_at_position inserted instance {} in {}ms",
        instance.id,
        insert_instance_start.elapsed().as_millis()
    );

    let next_position_start = Instant::now();
    let next_position = next_pipeline_position(conn, pipeline_id)?;
    println!(
        "[DB] insert_instance_at_position next position lookup completed in {}ms (next_position={})",
        next_position_start.elapsed().as_millis(),
        next_position
    );

    let mut shift_time_ms = 0u128;
    let final_position = if position > next_position {
        println!(
            "[DB] insert_instance_at_position requested position {} exceeds next_position {}, clamping",
            position, next_position
        );
        next_position
    } else {
        let shift_start = Instant::now();
        println!(
            "[DB] insert_instance_at_position shifting pipeline '{}' from position {}",
            pipeline_id, position
        );
        conn.execute(
            r#"
                UPDATE pipeline_items
                SET position = position + 1
                WHERE pipeline_id = ?1 AND position >= ?2
            "#,
            (pipeline_id, position),
        )
        .with_context(|| {
            format!(
                "Failed to shift positions when inserting at position {} in pipeline '{}'",
                position, pipeline_id
            )
        })?;
        shift_time_ms = shift_start.elapsed().as_millis();
        println!(
            "[DB] insert_instance_at_position shifted pipeline '{}' starting at position {} in {}ms",
            pipeline_id, position, shift_time_ms
        );
        position
    };

    if shift_time_ms == 0 {
        println!(
            "[DB] insert_instance_at_position no position shift required (final_position={})",
            final_position
        );
    }

    let pipeline_item_id = Uuid::new_v4().to_string();
    let pipeline_item = PipelineItem {
        id: pipeline_item_id.clone(),
        pipeline_id: Some(pipeline_id.to_string()),
        item_type: None,
        data: None,
        instance_id: Some(instance.id.clone()),
        action_title: Some(action.title.clone()),
        position: Some(final_position),
        created_at: None,
        metadata: None,
    };

    let insert_pipeline_start = Instant::now();
    insert_pipeline_item(conn, &pipeline_item)?;
    println!(
        "[DB] insert_instance_at_position inserted pipeline item {} in {}ms",
        pipeline_item_id,
        insert_pipeline_start.elapsed().as_millis()
    );

    let normalize_start = Instant::now();
    println!(
        "[DB] insert_instance_at_position normalizing pipeline '{}'",
        pipeline_id
    );
    normalize_pipeline_positions(conn, pipeline_id)?;
    let normalize_time_ms = normalize_start.elapsed().as_millis();
    println!(
        "[DB] insert_instance_at_position normalized pipeline '{}' in {}ms",
        pipeline_id, normalize_time_ms
    );

    println!(
        "[DB] insert_instance_at_position finished for instance {} in {}ms (final_position={}, shift_time={}ms, normalization_time={}ms)",
        instance.id,
        start.elapsed().as_millis(),
        final_position,
        shift_time_ms,
        normalize_time_ms
    );

    Ok((instance, pipeline_item))
}

pub fn update_pipeline_item_position(
    conn: &Connection,
    pipeline_item_id: &str,
    new_position: i64,
) -> Result<()> {
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
            WHERE id = ?1
        "#,
        )
        .context("Failed to prepare pipeline item fetch for position update")?;

    let current_item = stmt
        .query_row([pipeline_item_id], |row| {
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
        .optional()
        .context("Failed to fetch pipeline item for position update")?;

    let Some(item) = current_item else {
        return Err(anyhow::anyhow!(
            "Pipeline item '{}' not found",
            pipeline_item_id
        ));
    };

    let Some(current_pos) = item.position else {
        return Err(anyhow::anyhow!(
            "Pipeline item '{}' has no position",
            pipeline_item_id
        ));
    };

    let Some(ref pipeline_id) = item.pipeline_id else {
        return Err(anyhow::anyhow!(
            "Pipeline item '{}' has no pipeline_id",
            pipeline_item_id
        ));
    };

    if current_pos == new_position {
        return Ok(());
    }

    if new_position < current_pos {
        conn.execute(
            r#"
                UPDATE pipeline_items
                SET position = position + 1
                WHERE pipeline_id = ?1 AND position >= ?2 AND position < ?3
            "#,
            (&pipeline_id, new_position, current_pos),
        )
        .with_context(|| {
            format!(
                "Failed to shift positions when moving item up from {} to {}",
                current_pos, new_position
            )
        })?;
    } else {
        conn.execute(
            r#"
                UPDATE pipeline_items
                SET position = position - 1
                WHERE pipeline_id = ?1 AND position > ?2 AND position <= ?3
            "#,
            (&pipeline_id, current_pos, new_position),
        )
        .with_context(|| {
            format!(
                "Failed to shift positions when moving item down from {} to {}",
                current_pos, new_position
            )
        })?;
    }

    conn.execute(
        r#"
            UPDATE pipeline_items
            SET position = ?1
            WHERE id = ?2
        "#,
        (new_position, pipeline_item_id),
    )
    .with_context(|| {
        format!(
            "Failed to update position for pipeline item '{}'",
            pipeline_item_id
        )
    })?;

    normalize_pipeline_positions(conn, &pipeline_id)?;

    Ok(())
}

pub fn normalize_pipeline_positions(conn: &Connection, pipeline_id: &str) -> Result<()> {
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
            WHERE pipeline_id = ?1
            ORDER BY position ASC, created_at ASC
        "#,
        )
        .with_context(|| {
            format!(
                "Failed to prepare pipeline items fetch for normalization in '{}'",
                pipeline_id
            )
        })?;

    let items = stmt
        .query_map([pipeline_id], |row| {
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
        .with_context(|| {
            format!(
                "Failed to query pipeline items for normalization in '{}'",
                pipeline_id
            )
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| {
            format!(
                "Failed to map pipeline items for normalization in '{}'",
                pipeline_id
            )
        })?;

    let mut updates_needed = 0;

    for (index, item) in items.iter().enumerate() {
        let new_position = (index as i64) + 1;
        if let Some(current_pos) = item.position {
            if current_pos != new_position {
                conn.execute(
                    r#"
                        UPDATE pipeline_items
                        SET position = ?1
                        WHERE id = ?2
                    "#,
                    (new_position, &item.id),
                )
                .with_context(|| {
                    format!(
                        "Failed to normalize position for pipeline item '{}'",
                        item.id
                    )
                })?;

                updates_needed += 1;
            }
        }
    }

    if updates_needed > 0 {
        println!(
            "Normalized {} pipeline positions in '{}'",
            updates_needed, pipeline_id
        );
    }

    Ok(())
}
