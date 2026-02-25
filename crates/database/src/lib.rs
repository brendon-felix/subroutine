use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

mod action;
mod event;
mod pipeline;
mod routine;
mod saved_action;
mod saved_event;
mod saved_mental_state;

pub use action::*;
pub use event::*;
pub use pipeline::*;
pub use routine::*;
pub use saved_action::*;
pub use saved_event::*;
pub use saved_mental_state::*;

pub type DatabaseConnection = Arc<Mutex<Connection>>;

fn database_path() -> Result<PathBuf> {
    let mut path =
        dirs::data_dir().context("Could not determine the operating system data directory")?;
    path.push("Subroutine");
    fs::create_dir_all(&path)
        .with_context(|| format!("Unable to create database directory '{}'", path.display()))?;
    path.push("subroutine.db");
    Ok(path)
}

pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(include_str!(
        "../migrations/20260225122443_init_schema.sql"
    ))])
}

pub fn create_connection() -> Result<Connection> {
    let path = database_path()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Unable to create parent directory '{}' for database file",
                parent.display()
            )
        })?;
    }

    OpenOptions::new()
        .create(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("Unable to create or open database '{}'", path.display()))?;

    let connection = Connection::open(&path)
        .with_context(|| format!("Failed to connect to SQLite at '{}'", path.display()))?;

    connection
        .pragma_update(None, "journal_mode", "WAL")
        .context("Failed to enable WAL journal mode")?;

    connection
        .pragma_update(None, "busy_timeout", 5000)
        .context("Failed to set busy timeout")?;

    connection
        .pragma_update(None, "foreign_keys", "ON")
        .context("Failed to enable foreign keys")?;

    Ok(connection)
}

pub fn connect() -> Result<DatabaseConnection> {
    let connection = create_connection()?;
    Ok(Arc::new(Mutex::new(connection)))
}

pub fn connect_and_migrate() -> Result<DatabaseConnection> {
    let conn = connect()?;

    {
        let mut connection = conn.lock().unwrap();
        migrations()
            .to_latest(&mut connection)
            .context("Applying database migrations failed")?;
        seed_starter_mental_states(&connection).context("Failed to seed starter mental states")?;
    }

    Ok(conn)
}
