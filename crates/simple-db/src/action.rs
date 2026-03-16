use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension};
use simple_core::{Action, ActionCompletion};
use uuid::Uuid;

use crate::{DatabaseError, Result};

struct ActionRow {
    id: String,
    lineage_id: String,
    origin_routine_id: Option<String>,
    title: String,
    content: Option<String>,
    target: Option<String>,
    target_static: i64,
    duration_secs: Option<i64>,
    recurrence_secs: Option<i64>,
    ephemeral: i64,
}

impl TryFrom<&rusqlite::Row<'_>> for ActionRow {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            lineage_id: row.get(1)?,
            origin_routine_id: row.get(2)?,
            title: row.get(3)?,
            content: row.get(4)?,
            target: row.get(5)?,
            target_static: row.get(6)?,
            duration_secs: row.get(7)?,
            recurrence_secs: row.get(8)?,
            ephemeral: row.get(9)?,
        })
    }
}

impl From<&Action> for ActionRow {
    fn from(action: &Action) -> Self {
        Self {
            id: action.id.to_string(),
            lineage_id: action.lineage_id.to_string(),
            origin_routine_id: action.origin_routine_id.map(|id| id.to_string()),
            title: action.title.clone(),
            content: action.content.clone(),
            target: action.target.map(|t| t.to_rfc3339()),
            target_static: action.target_static as i64,
            duration_secs: action.duration.map(|d| d.num_seconds()),
            recurrence_secs: action.recurrence.map(|d| d.num_seconds()),
            ephemeral: action.ephemeral as i64,
        }
    }
}

impl TryFrom<ActionRow> for Action {
    type Error = DatabaseError;

    fn try_from(row: ActionRow) -> Result<Self> {
        let id =
            Uuid::parse_str(&row.id).map_err(|e| DatabaseError::invalid_uuid(0, &row.id, e))?;

        let lineage_id = Uuid::parse_str(&row.lineage_id)
            .map_err(|e| DatabaseError::invalid_uuid(1, &row.lineage_id, e))?;

        let origin_routine_id = row
            .origin_routine_id
            .map(|s| Uuid::parse_str(&s).map_err(|e| DatabaseError::invalid_uuid(2, &s, e)))
            .transpose()?;

        let target = row
            .target
            .map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| DatabaseError::invalid_datetime(5, &s, e))
            })
            .transpose()?;

        Ok(Action {
            id,
            lineage_id,
            origin_routine_id,
            title: row.title,
            content: row.content,
            target,
            target_static: row.target_static != 0,
            duration: row.duration_secs.map(Duration::seconds),
            recurrence: row.recurrence_secs.map(Duration::seconds),
            ephemeral: row.ephemeral != 0,
        })
    }
}

fn row_to_action(row: &rusqlite::Row) -> rusqlite::Result<Action> {
    let action_row = ActionRow::try_from(row)?;
    Action::try_from(action_row).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

pub fn upsert_action(conn: &Connection, action: &Action) -> Result<()> {
    let row = ActionRow::from(action);
    conn.execute(
        r#"
        INSERT INTO actions (
            id, lineage_id, origin_routine_id,
            title, content, target, target_static, duration_secs, recurrence_secs, ephemeral
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(id) DO UPDATE SET
            lineage_id        = excluded.lineage_id,
            origin_routine_id = excluded.origin_routine_id,
            title             = excluded.title,
            content           = excluded.content,
            target            = excluded.target,
            target_static     = excluded.target_static,
            duration_secs     = excluded.duration_secs,
            recurrence_secs   = excluded.recurrence_secs,
            ephemeral         = excluded.ephemeral
        "#,
        rusqlite::params![
            row.id,
            row.lineage_id,
            row.origin_routine_id,
            row.title,
            row.content,
            row.target,
            row.target_static,
            row.duration_secs,
            row.recurrence_secs,
            row.ephemeral,
        ],
    )
    .map_err(|e| DatabaseError::sqlite("Failed to insert or update action", e))?;
    Ok(())
}

pub fn fetch_actions(conn: &Connection) -> Result<Vec<Action>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, lineage_id, origin_routine_id,
                   title, content, target, target_static, duration_secs, recurrence_secs, ephemeral
            FROM actions
            ORDER BY title ASC
            "#,
        )
        .map_err(|e| DatabaseError::sqlite("Failed to prepare action fetch query", e))?;

    let actions = stmt
        .query_map([], row_to_action)
        .map_err(|e| DatabaseError::sqlite("Failed to query actions", e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| DatabaseError::sqlite("Failed to map action rows", e))?;

    Ok(actions)
}

pub fn fetch_action_by_id(conn: &Connection, id: Uuid) -> Result<Option<Action>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, lineage_id, origin_routine_id,
                   title, content, target, target_static, duration_secs, recurrence_secs, ephemeral
            FROM actions
            WHERE id = ?1
            "#,
        )
        .map_err(|e| DatabaseError::sqlite("Failed to prepare action fetch by id query", e))?;

    stmt.query_row([id.to_string()], row_to_action)
        .optional()
        .map_err(|e| DatabaseError::sqlite("Failed to fetch action by id", e))
}

pub fn delete_action(conn: &Connection, id: Uuid) -> Result<()> {
    conn.execute("DELETE FROM actions WHERE id = ?1", [id.to_string()])
        .map_err(|e| DatabaseError::sqlite(format!("Failed to delete action '{}'", id), e))?;
    Ok(())
}

struct ActionCompletionRow {
    id: String,
    action_id: String,
    lineage_id: String,
    completed_at: String,
    notes: Option<String>,
}

impl TryFrom<&rusqlite::Row<'_>> for ActionCompletionRow {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            action_id: row.get(1)?,
            lineage_id: row.get(2)?,
            completed_at: row.get(3)?,
            notes: row.get(4)?,
        })
    }
}

impl From<&ActionCompletion> for ActionCompletionRow {
    fn from(completion: &ActionCompletion) -> Self {
        Self {
            id: completion.id.to_string(),
            action_id: completion.action_id.to_string(),
            lineage_id: completion.lineage_id.to_string(),
            completed_at: completion.completed_at.to_rfc3339(),
            notes: completion.notes.clone(),
        }
    }
}

impl TryFrom<ActionCompletionRow> for ActionCompletion {
    type Error = DatabaseError;

    fn try_from(row: ActionCompletionRow) -> Result<Self> {
        let id =
            Uuid::parse_str(&row.id).map_err(|e| DatabaseError::invalid_uuid(0, &row.id, e))?;

        let action_id = Uuid::parse_str(&row.action_id)
            .map_err(|e| DatabaseError::invalid_uuid(1, &row.action_id, e))?;

        let lineage_id = Uuid::parse_str(&row.lineage_id)
            .map_err(|e| DatabaseError::invalid_uuid(2, &row.lineage_id, e))?;

        let completed_at = DateTime::parse_from_rfc3339(&row.completed_at)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| DatabaseError::invalid_datetime(3, &row.completed_at, e))?;

        Ok(ActionCompletion {
            id,
            action_id,
            lineage_id,
            completed_at,
            notes: row.notes,
        })
    }
}

fn row_to_completion(row: &rusqlite::Row) -> rusqlite::Result<ActionCompletion> {
    let completion_row = ActionCompletionRow::try_from(row)?;
    ActionCompletion::try_from(completion_row).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

pub fn insert_action_completion(conn: &Connection, completion: &ActionCompletion) -> Result<()> {
    let row = ActionCompletionRow::from(completion);
    conn.execute(
        r#"
        INSERT INTO action_completions (id, action_id, lineage_id, completed_at, notes)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(id) DO UPDATE SET
            action_id    = excluded.action_id,
            lineage_id   = excluded.lineage_id,
            completed_at = excluded.completed_at,
            notes        = excluded.notes
        "#,
        rusqlite::params![
            row.id,
            row.action_id,
            row.lineage_id,
            row.completed_at,
            row.notes,
        ],
    )
    .map_err(|e| DatabaseError::sqlite("Failed to insert action completion", e))?;
    Ok(())
}

pub fn delete_action_completion(conn: &Connection, id: Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM action_completions WHERE id = ?1",
        [id.to_string()],
    )
    .map_err(|e| {
        DatabaseError::sqlite(format!("Failed to delete action completion '{}'", id), e)
    })?;
    Ok(())
}

pub fn fetch_completions_by_lineage(
    conn: &Connection,
    lineage_id: Uuid,
) -> Result<Vec<ActionCompletion>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, action_id, lineage_id, completed_at, notes
            FROM action_completions
            WHERE lineage_id = ?1
            ORDER BY completed_at ASC
            "#,
        )
        .map_err(|e| DatabaseError::sqlite("Failed to prepare completions by lineage query", e))?;

    let completions = stmt
        .query_map([lineage_id.to_string()], row_to_completion)
        .map_err(|e| DatabaseError::sqlite("Failed to query completions by lineage", e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| DatabaseError::sqlite("Failed to map completion rows", e))?;

    Ok(completions)
}

pub fn fetch_all_completions(conn: &Connection) -> Result<Vec<ActionCompletion>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, action_id, lineage_id, completed_at, notes
            FROM action_completions
            ORDER BY completed_at DESC
            "#,
        )
        .map_err(|e| DatabaseError::sqlite("Failed to prepare fetch all completions query", e))?;

    let completions = stmt
        .query_map([], row_to_completion)
        .map_err(|e| DatabaseError::sqlite("Failed to query all completions", e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| DatabaseError::sqlite("Failed to map completion rows", e))?;

    Ok(completions)
}

pub fn fetch_completions_by_origin_routine(
    conn: &Connection,
    routine_id: Uuid,
) -> Result<Vec<ActionCompletion>> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT c.id, c.action_id, c.lineage_id, c.completed_at, c.notes
            FROM action_completions c
            JOIN actions a ON a.id = c.action_id
            WHERE a.origin_routine_id = ?1
            ORDER BY c.completed_at ASC
            "#,
        )
        .map_err(|e| {
            DatabaseError::sqlite("Failed to prepare completions by origin routine query", e)
        })?;

    let completions = stmt
        .query_map([routine_id.to_string()], row_to_completion)
        .map_err(|e| DatabaseError::sqlite("Failed to query completions by origin routine", e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| DatabaseError::sqlite("Failed to map completion rows", e))?;

    Ok(completions)
}
