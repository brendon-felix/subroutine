use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;
use tokio_postgres::{Client, NoTls};

use crate::DatabaseConnection;

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
}

impl PostgresConfig {
    pub fn connection_string(&self) -> String {
        format!(
            "host={} port={} user={} password={} dbname={}",
            self.host, self.port, self.user, self.password, self.dbname
        )
    }
}

async fn connect_postgres(config: &PostgresConfig) -> Result<Client> {
    let (client, connection) = tokio_postgres::connect(&config.connection_string(), NoTls)
        .await
        .context("Failed to connect to Postgres")?;

    // Drive the connection in a background task. It exits when the client is
    // dropped, which is fine for short-lived sync sessions.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("[sync] postgres connection driver error: {e}");
        }
    });

    Ok(client)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_rfc3339() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Ensure the watermark row exists and return the current value.
fn get_watermark(conn: &Connection, table: &str) -> Result<String> {
    conn.execute(
        "INSERT OR IGNORE INTO sync_watermarks (table_name) VALUES (?1)",
        [table],
    )?;
    let wm: String = conn.query_row(
        "SELECT last_pulled_at FROM sync_watermarks WHERE table_name = ?1",
        [table],
        |row| row.get(0),
    )?;
    Ok(wm)
}

fn set_watermark(conn: &Connection, table: &str, watermark: &str) -> Result<()> {
    conn.execute(
        "UPDATE sync_watermarks SET last_pulled_at = ?1 WHERE table_name = ?2",
        rusqlite::params![watermark, table],
    )?;
    Ok(())
}

fn mark_synced(conn: &Connection, table: &str, id: &str) -> Result<()> {
    let now = now_rfc3339();
    conn.execute(
        &format!("UPDATE {table} SET synced_at = ?1 WHERE id = ?2"),
        rusqlite::params![now, id],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Row types collected from SQLite before any await
// ---------------------------------------------------------------------------

struct ActionSyncRow {
    id: String,
    lineage_id: String,
    origin_routine_id: Option<String>,
    title: String,
    content: Option<String>,
    target: Option<String>,
    target_static: i64,
    naive_date: Option<String>,
    duration_secs: Option<i64>,
    recurrence_secs: Option<i64>,
    ephemeral: i64,
    completed_at: Option<String>,
    updated_at: String,
    deleted: i64,
}

struct EventSyncRow {
    id: String,
    lineage_id: String,
    title: String,
    content: Option<String>,
    time: String,
    duration_secs: Option<i64>,
    recurrence_secs: Option<i64>,
    ephemeral: i64,
    updated_at: String,
    deleted: i64,
}

struct RoutineSyncRow {
    id: String,
    title: String,
    content: Option<String>,
    target: Option<String>,
    recurrence_secs: Option<i64>,
    updated_at: String,
    deleted: i64,
}

struct CompletionSyncRow {
    id: String,
    action_id: String,
    lineage_id: String,
    completed_at: String,
    notes: Option<String>,
    updated_at: String,
    deleted: i64,
}

// ---------------------------------------------------------------------------
// Collect dirty rows from SQLite (synchronous, no await)
// ---------------------------------------------------------------------------

fn collect_dirty_actions(conn: &Connection) -> Result<Vec<ActionSyncRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, lineage_id, origin_routine_id, title, content, target, target_static,
                naive_date, duration_secs, recurrence_secs, ephemeral, completed_at,
                updated_at, deleted
         FROM actions WHERE synced_at IS NULL",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ActionSyncRow {
                id: row.get(0)?,
                lineage_id: row.get(1)?,
                origin_routine_id: row.get(2)?,
                title: row.get(3)?,
                content: row.get(4)?,
                target: row.get(5)?,
                target_static: row.get(6)?,
                naive_date: row.get(7)?,
                duration_secs: row.get(8)?,
                recurrence_secs: row.get(9)?,
                ephemeral: row.get(10)?,
                completed_at: row.get(11)?,
                updated_at: row.get(12)?,
                deleted: row.get(13)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn collect_dirty_events(conn: &Connection) -> Result<Vec<EventSyncRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, lineage_id, title, content, time, duration_secs, recurrence_secs,
                ephemeral, updated_at, deleted
         FROM events WHERE synced_at IS NULL",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(EventSyncRow {
                id: row.get(0)?,
                lineage_id: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                time: row.get(4)?,
                duration_secs: row.get(5)?,
                recurrence_secs: row.get(6)?,
                ephemeral: row.get(7)?,
                updated_at: row.get(8)?,
                deleted: row.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn collect_dirty_routines(conn: &Connection) -> Result<Vec<RoutineSyncRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, content, target, recurrence_secs, updated_at, deleted
         FROM routines WHERE synced_at IS NULL",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(RoutineSyncRow {
                id: row.get(0)?,
                title: row.get(1)?,
                content: row.get(2)?,
                target: row.get(3)?,
                recurrence_secs: row.get(4)?,
                updated_at: row.get(5)?,
                deleted: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn collect_dirty_completions(conn: &Connection) -> Result<Vec<CompletionSyncRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, action_id, lineage_id, completed_at, notes, updated_at, deleted
         FROM action_completions WHERE synced_at IS NULL",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(CompletionSyncRow {
                id: row.get(0)?,
                action_id: row.get(1)?,
                lineage_id: row.get(2)?,
                completed_at: row.get(3)?,
                notes: row.get(4)?,
                updated_at: row.get(5)?,
                deleted: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Push: send collected rows to Postgres, then stamp SQLite (two short locks)
// ---------------------------------------------------------------------------

async fn push_actions(db: &DatabaseConnection, pg: &Client) -> Result<()> {
    // Lock 1: collect dirty rows.
    let rows = {
        let conn = db.lock().unwrap();
        collect_dirty_actions(&conn)?
    };

    // Async work: upsert each row into Postgres.
    for row in &rows {
        pg.execute(
            "INSERT INTO actions (
                id, lineage_id, origin_routine_id, title, content, target,
                target_static, naive_date, duration_secs, recurrence_secs, ephemeral,
                completed_at, updated_at, synced_at, deleted)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,NULL,$14)
             ON CONFLICT(id) DO UPDATE SET
                lineage_id        = EXCLUDED.lineage_id,
                origin_routine_id = EXCLUDED.origin_routine_id,
                title             = EXCLUDED.title,
                content           = EXCLUDED.content,
                target            = EXCLUDED.target,
                target_static     = EXCLUDED.target_static,
                naive_date        = EXCLUDED.naive_date,
                duration_secs     = EXCLUDED.duration_secs,
                recurrence_secs   = EXCLUDED.recurrence_secs,
                ephemeral         = EXCLUDED.ephemeral,
                completed_at      = EXCLUDED.completed_at,
                updated_at        = EXCLUDED.updated_at,
                synced_at         = NULL,
                deleted           = EXCLUDED.deleted
             WHERE EXCLUDED.updated_at > actions.updated_at",
            &[
                &row.id,
                &row.lineage_id,
                &row.origin_routine_id,
                &row.title,
                &row.content,
                &row.target,
                &(row.target_static as i32),
                &row.naive_date,
                &row.duration_secs,
                &row.recurrence_secs,
                &(row.ephemeral as i32),
                &row.completed_at,
                &row.updated_at,
                &(row.deleted as i32),
            ],
        )
        .await
        .context("Failed to push action to Postgres")?;
    }

    // Lock 2: mark synced.
    {
        let conn = db.lock().unwrap();
        for row in &rows {
            mark_synced(&conn, "actions", &row.id)?;
        }
    }
    Ok(())
}

async fn push_events(db: &DatabaseConnection, pg: &Client) -> Result<()> {
    let rows = {
        let conn = db.lock().unwrap();
        collect_dirty_events(&conn)?
    };

    for row in &rows {
        pg.execute(
            "INSERT INTO events (
                id, lineage_id, title, content, time, duration_secs, recurrence_secs,
                ephemeral, updated_at, synced_at, deleted)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,NULL,$10)
             ON CONFLICT(id) DO UPDATE SET
                lineage_id      = EXCLUDED.lineage_id,
                title           = EXCLUDED.title,
                content         = EXCLUDED.content,
                time            = EXCLUDED.time,
                duration_secs   = EXCLUDED.duration_secs,
                recurrence_secs = EXCLUDED.recurrence_secs,
                ephemeral       = EXCLUDED.ephemeral,
                updated_at      = EXCLUDED.updated_at,
                synced_at       = NULL,
                deleted         = EXCLUDED.deleted
             WHERE EXCLUDED.updated_at > events.updated_at",
            &[
                &row.id,
                &row.lineage_id,
                &row.title,
                &row.content,
                &row.time,
                &row.duration_secs,
                &row.recurrence_secs,
                &(row.ephemeral as i32),
                &row.updated_at,
                &(row.deleted as i32),
            ],
        )
        .await
        .context("Failed to push event to Postgres")?;
    }

    {
        let conn = db.lock().unwrap();
        for row in &rows {
            mark_synced(&conn, "events", &row.id)?;
        }
    }
    Ok(())
}

async fn push_routines(db: &DatabaseConnection, pg: &Client) -> Result<()> {
    let rows = {
        let conn = db.lock().unwrap();
        collect_dirty_routines(&conn)?
    };

    for row in &rows {
        pg.execute(
            "INSERT INTO routines (
                id, title, content, target, recurrence_secs, updated_at, synced_at, deleted)
             VALUES ($1,$2,$3,$4,$5,$6,NULL,$7)
             ON CONFLICT(id) DO UPDATE SET
                title           = EXCLUDED.title,
                content         = EXCLUDED.content,
                target          = EXCLUDED.target,
                recurrence_secs = EXCLUDED.recurrence_secs,
                updated_at      = EXCLUDED.updated_at,
                synced_at       = NULL,
                deleted         = EXCLUDED.deleted
             WHERE EXCLUDED.updated_at > routines.updated_at",
            &[
                &row.id,
                &row.title,
                &row.content,
                &row.target,
                &row.recurrence_secs,
                &row.updated_at,
                &(row.deleted as i32),
            ],
        )
        .await
        .context("Failed to push routine to Postgres")?;
    }

    {
        let conn = db.lock().unwrap();
        for row in &rows {
            mark_synced(&conn, "routines", &row.id)?;
        }
    }
    Ok(())
}

async fn push_action_completions(db: &DatabaseConnection, pg: &Client) -> Result<()> {
    let rows = {
        let conn = db.lock().unwrap();
        collect_dirty_completions(&conn)?
    };

    for row in &rows {
        pg.execute(
            "INSERT INTO action_completions (
                id, action_id, lineage_id, completed_at, notes, updated_at, synced_at, deleted)
             VALUES ($1,$2,$3,$4,$5,$6,NULL,$7)
             ON CONFLICT(id) DO UPDATE SET
                action_id    = EXCLUDED.action_id,
                lineage_id   = EXCLUDED.lineage_id,
                completed_at = EXCLUDED.completed_at,
                notes        = EXCLUDED.notes,
                updated_at   = EXCLUDED.updated_at,
                synced_at    = NULL,
                deleted      = EXCLUDED.deleted
             WHERE EXCLUDED.updated_at > action_completions.updated_at",
            &[
                &row.id,
                &row.action_id,
                &row.lineage_id,
                &row.completed_at,
                &row.notes,
                &row.updated_at,
                &(row.deleted as i32),
            ],
        )
        .await
        .context("Failed to push action_completion to Postgres")?;
    }

    {
        let conn = db.lock().unwrap();
        for row in &rows {
            mark_synced(&conn, "action_completions", &row.id)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Pull: fetch from Postgres, collect into Vecs, then write to SQLite
// ---------------------------------------------------------------------------

async fn pull_actions(db: &DatabaseConnection, pg: &Client) -> Result<()> {
    let (watermark, new_watermark) = {
        let conn = db.lock().unwrap();
        (get_watermark(&conn, "actions")?, now_rfc3339())
    };

    let pg_rows = pg
        .query(
            "SELECT id, lineage_id, origin_routine_id, title, content, target, target_static,
                    naive_date, duration_secs, recurrence_secs, ephemeral, completed_at,
                    updated_at, deleted
             FROM actions WHERE updated_at > $1",
            &[&watermark],
        )
        .await
        .context("Failed to pull actions from Postgres")?;

    // Collect into owned Vecs before touching SQLite.
    let rows: Vec<ActionSyncRow> = pg_rows
        .iter()
        .map(|row| ActionSyncRow {
            id: row.get(0),
            lineage_id: row.get(1),
            origin_routine_id: row.get(2),
            title: row.get(3),
            content: row.get(4),
            target: row.get(5),
            target_static: row.get::<_, i32>(6) as i64,
            naive_date: row.get(7),
            duration_secs: row.get(8),
            recurrence_secs: row.get(9),
            ephemeral: row.get::<_, i32>(10) as i64,
            completed_at: row.get(11),
            updated_at: row.get(12),
            deleted: row.get::<_, i32>(13) as i64,
        })
        .collect();

    {
        let conn = db.lock().unwrap();
        for row in &rows {
            // On pull, synced_at = updated_at (it came from the server, it's already synced).
            // Last-write-wins: only update if the incoming row is newer.
            conn.execute(
                "INSERT INTO actions (
                    id, lineage_id, origin_routine_id, title, content, target,
                    target_static, naive_date, duration_secs, recurrence_secs, ephemeral,
                    completed_at, updated_at, synced_at, deleted)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?13,?14)
                 ON CONFLICT(id) DO UPDATE SET
                    lineage_id        = excluded.lineage_id,
                    origin_routine_id = excluded.origin_routine_id,
                    title             = excluded.title,
                    content           = excluded.content,
                    target            = excluded.target,
                    target_static     = excluded.target_static,
                    naive_date        = excluded.naive_date,
                    duration_secs     = excluded.duration_secs,
                    recurrence_secs   = excluded.recurrence_secs,
                    ephemeral         = excluded.ephemeral,
                    completed_at      = excluded.completed_at,
                    updated_at        = excluded.updated_at,
                    synced_at         = excluded.updated_at,
                    deleted           = excluded.deleted
                 WHERE excluded.updated_at > actions.updated_at",
                rusqlite::params![
                    row.id,
                    row.lineage_id,
                    row.origin_routine_id,
                    row.title,
                    row.content,
                    row.target,
                    row.target_static,
                    row.naive_date,
                    row.duration_secs,
                    row.recurrence_secs,
                    row.ephemeral,
                    row.completed_at,
                    row.updated_at,
                    row.deleted,
                ],
            )?;
        }
        set_watermark(&conn, "actions", &new_watermark)?;
    }
    Ok(())
}

async fn pull_events(db: &DatabaseConnection, pg: &Client) -> Result<()> {
    let (watermark, new_watermark) = {
        let conn = db.lock().unwrap();
        (get_watermark(&conn, "events")?, now_rfc3339())
    };

    let pg_rows = pg
        .query(
            "SELECT id, lineage_id, title, content, time, duration_secs, recurrence_secs,
                    ephemeral, updated_at, deleted
             FROM events WHERE updated_at > $1",
            &[&watermark],
        )
        .await
        .context("Failed to pull events from Postgres")?;

    let rows: Vec<EventSyncRow> = pg_rows
        .iter()
        .map(|row| EventSyncRow {
            id: row.get(0),
            lineage_id: row.get(1),
            title: row.get(2),
            content: row.get(3),
            time: row.get(4),
            duration_secs: row.get(5),
            recurrence_secs: row.get(6),
            ephemeral: row.get::<_, i32>(7) as i64,
            updated_at: row.get(8),
            deleted: row.get::<_, i32>(9) as i64,
        })
        .collect();

    {
        let conn = db.lock().unwrap();
        for row in &rows {
            conn.execute(
                "INSERT INTO events (
                    id, lineage_id, title, content, time, duration_secs, recurrence_secs,
                    ephemeral, updated_at, synced_at, deleted)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9,?10)
                 ON CONFLICT(id) DO UPDATE SET
                    lineage_id      = excluded.lineage_id,
                    title           = excluded.title,
                    content         = excluded.content,
                    time            = excluded.time,
                    duration_secs   = excluded.duration_secs,
                    recurrence_secs = excluded.recurrence_secs,
                    ephemeral       = excluded.ephemeral,
                    updated_at      = excluded.updated_at,
                    synced_at       = excluded.updated_at,
                    deleted         = excluded.deleted
                 WHERE excluded.updated_at > events.updated_at",
                rusqlite::params![
                    row.id,
                    row.lineage_id,
                    row.title,
                    row.content,
                    row.time,
                    row.duration_secs,
                    row.recurrence_secs,
                    row.ephemeral,
                    row.updated_at,
                    row.deleted,
                ],
            )?;
        }
        set_watermark(&conn, "events", &new_watermark)?;
    }
    Ok(())
}

async fn pull_routines(db: &DatabaseConnection, pg: &Client) -> Result<()> {
    let (watermark, new_watermark) = {
        let conn = db.lock().unwrap();
        (get_watermark(&conn, "routines")?, now_rfc3339())
    };

    let pg_rows = pg
        .query(
            "SELECT id, title, content, target, recurrence_secs, updated_at, deleted
             FROM routines WHERE updated_at > $1",
            &[&watermark],
        )
        .await
        .context("Failed to pull routines from Postgres")?;

    let rows: Vec<RoutineSyncRow> = pg_rows
        .iter()
        .map(|row| RoutineSyncRow {
            id: row.get(0),
            title: row.get(1),
            content: row.get(2),
            target: row.get(3),
            recurrence_secs: row.get(4),
            updated_at: row.get(5),
            deleted: row.get::<_, i32>(6) as i64,
        })
        .collect();

    {
        let conn = db.lock().unwrap();
        for row in &rows {
            conn.execute(
                "INSERT INTO routines (
                    id, title, content, target, recurrence_secs, updated_at, synced_at, deleted)
                 VALUES (?1,?2,?3,?4,?5,?6,?6,?7)
                 ON CONFLICT(id) DO UPDATE SET
                    title           = excluded.title,
                    content         = excluded.content,
                    target          = excluded.target,
                    recurrence_secs = excluded.recurrence_secs,
                    updated_at      = excluded.updated_at,
                    synced_at       = excluded.updated_at,
                    deleted         = excluded.deleted
                 WHERE excluded.updated_at > routines.updated_at",
                rusqlite::params![
                    row.id,
                    row.title,
                    row.content,
                    row.target,
                    row.recurrence_secs,
                    row.updated_at,
                    row.deleted,
                ],
            )?;
        }
        set_watermark(&conn, "routines", &new_watermark)?;
    }
    Ok(())
}

async fn pull_action_completions(db: &DatabaseConnection, pg: &Client) -> Result<()> {
    let (watermark, new_watermark) = {
        let conn = db.lock().unwrap();
        (get_watermark(&conn, "action_completions")?, now_rfc3339())
    };

    let pg_rows = pg
        .query(
            "SELECT id, action_id, lineage_id, completed_at, notes, updated_at, deleted
             FROM action_completions WHERE updated_at > $1",
            &[&watermark],
        )
        .await
        .context("Failed to pull action_completions from Postgres")?;

    let rows: Vec<CompletionSyncRow> = pg_rows
        .iter()
        .map(|row| CompletionSyncRow {
            id: row.get(0),
            action_id: row.get(1),
            lineage_id: row.get(2),
            completed_at: row.get(3),
            notes: row.get(4),
            updated_at: row.get(5),
            deleted: row.get::<_, i32>(6) as i64,
        })
        .collect();

    {
        let conn = db.lock().unwrap();
        for row in &rows {
            conn.execute(
                "INSERT INTO action_completions (
                    id, action_id, lineage_id, completed_at, notes,
                    updated_at, synced_at, deleted)
                 VALUES (?1,?2,?3,?4,?5,?6,?6,?7)
                 ON CONFLICT(id) DO UPDATE SET
                    action_id    = excluded.action_id,
                    lineage_id   = excluded.lineage_id,
                    completed_at = excluded.completed_at,
                    notes        = excluded.notes,
                    updated_at   = excluded.updated_at,
                    synced_at    = excluded.updated_at,
                    deleted      = excluded.deleted
                 WHERE excluded.updated_at > action_completions.updated_at",
                rusqlite::params![
                    row.id,
                    row.action_id,
                    row.lineage_id,
                    row.completed_at,
                    row.notes,
                    row.updated_at,
                    row.deleted,
                ],
            )?;
        }
        set_watermark(&conn, "action_completions", &new_watermark)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run one full push → pull cycle against Postgres.
///
/// The SQLite mutex is **never** held across an await point:
/// - Push phase: lock → collect rows → unlock → await Postgres → lock → mark synced → unlock
/// - Pull phase: lock → get watermark → unlock → await Postgres → lock → write rows + watermark → unlock
///
/// Returns `Err` if Postgres is unreachable or any operation fails.
/// The caller decides whether to surface or swallow the error.
pub async fn sync_once(db: &DatabaseConnection, config: &PostgresConfig) -> Result<()> {
    let pg = connect_postgres(config).await?;

    // Push local changes up first so the pull watermark captures them.
    push_actions(db, &pg).await?;
    push_events(db, &pg).await?;
    push_routines(db, &pg).await?;
    push_action_completions(db, &pg).await?;

    // Pull remote changes down.
    pull_actions(db, &pg).await?;
    pull_events(db, &pg).await?;
    pull_routines(db, &pg).await?;
    pull_action_completions(db, &pg).await?;

    Ok(())
}

/// Spawn a background task that syncs immediately, then every `interval_secs`
/// seconds. Errors are logged to stderr but do not abort the loop.
pub fn spawn_sync_loop(
    db: DatabaseConnection,
    config: PostgresConfig,
    interval_secs: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match sync_once(&db, &config).await {
                Ok(()) => {}
                Err(e) => eprintln!("[sync] error: {e:#}"),
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
        }
    })
}
