use std::{
    fmt,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

mod action;
mod event;
mod pipeline;
mod routine;
pub mod sync;

pub use action::*;
pub use event::*;
pub use pipeline::*;
pub use routine::*;
pub use sync::{PostgresConfig, spawn_sync_loop, sync_once};

pub type DatabaseConnection = Arc<Mutex<Connection>>;

#[derive(Debug)]
pub enum DatabaseError {
    NoDataDirectory,

    Io {
        message: String,
        source: std::io::Error,
    },

    Sqlite {
        message: String,
        source: rusqlite::Error,
    },

    Migration {
        source: rusqlite_migration::Error,
    },

    InvalidUuid {
        column: usize,
        value: String,
        source: uuid::Error,
    },

    InvalidDateTime {
        column: usize,
        value: String,
        source: chrono::ParseError,
    },

    InvalidNaiveDate {
        column: usize,
        value: String,
        source: chrono::format::ParseError,
    },

    UnknownVariant {
        column: &'static str,
        value: String,
    },

    MissingReference {
        referencing_table: String,
        referenced_table: String,
        id: String,
    },
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDataDirectory => {
                write!(
                    formatter,
                    "Could not determine the operating system data directory"
                )
            }
            Self::Io { message, .. } => write!(formatter, "{}", message),
            Self::Sqlite { message, .. } => write!(formatter, "{}", message),
            Self::Migration { source } => {
                write!(formatter, "Applying database migrations failed: {}", source)
            }
            Self::InvalidUuid { column, value, .. } => {
                write!(formatter, "Invalid UUID in column {}: '{}'", column, value)
            }
            Self::InvalidDateTime { column, value, .. } => write!(
                formatter,
                "Invalid RFC3339 datetime in column {}: '{}'",
                column, value
            ),
            Self::InvalidNaiveDate { column, value, .. } => write!(
                formatter,
                "Invalid ISO date (YYYY-MM-DD) in column {}: '{}'",
                column, value
            ),
            Self::UnknownVariant { column, value } => write!(
                formatter,
                "Unknown variant '{}' for column '{}'",
                value, column
            ),
            Self::MissingReference {
                referencing_table,
                referenced_table,
                id,
            } => write!(
                formatter,
                "'{}' references a missing '{}' row with id '{}'",
                referencing_table, referenced_table, id
            ),
        }
    }
}

impl std::error::Error for DatabaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Sqlite { source, .. } => Some(source),
            Self::Migration { source } => Some(source),
            Self::InvalidUuid { source, .. } => Some(source),
            Self::InvalidDateTime { source, .. } => Some(source),
            Self::InvalidNaiveDate { source, .. } => Some(source),
            Self::NoDataDirectory | Self::UnknownVariant { .. } | Self::MissingReference { .. } => {
                None
            }
        }
    }
}

impl DatabaseError {
    pub fn io(message: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            message: message.into(),
            source,
        }
    }

    pub fn sqlite(message: impl Into<String>, source: rusqlite::Error) -> Self {
        Self::Sqlite {
            message: message.into(),
            source,
        }
    }

    pub fn invalid_uuid(column: usize, value: impl Into<String>, source: uuid::Error) -> Self {
        Self::InvalidUuid {
            column,
            value: value.into(),
            source,
        }
    }

    pub fn invalid_datetime(
        column: usize,
        value: impl Into<String>,
        source: chrono::ParseError,
    ) -> Self {
        Self::InvalidDateTime {
            column,
            value: value.into(),
            source,
        }
    }

    pub fn unknown_variant(column: &'static str, value: impl Into<String>) -> Self {
        Self::UnknownVariant {
            column,
            value: value.into(),
        }
    }

    pub fn missing_reference(
        referencing_table: impl Into<String>,
        referenced_table: impl Into<String>,
        id: impl Into<String>,
    ) -> Self {
        Self::MissingReference {
            referencing_table: referencing_table.into(),
            referenced_table: referenced_table.into(),
            id: id.into(),
        }
    }
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite {
            message: error.to_string(),
            source: error,
        }
    }
}

impl From<rusqlite_migration::Error> for DatabaseError {
    fn from(error: rusqlite_migration::Error) -> Self {
        Self::Migration { source: error }
    }
}

pub type Result<T> = std::result::Result<T, DatabaseError>;

fn database_path() -> Result<PathBuf> {
    let mut path = dirs::data_dir().ok_or(DatabaseError::NoDataDirectory)?;
    path.push("Subroutine");
    fs::create_dir_all(&path).map_err(|error| {
        DatabaseError::io(
            format!("Unable to create database directory '{}'", path.display()),
            error,
        )
    })?;
    path.push("simple.db");
    Ok(path)
}

pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(include_str!("../migrations/sqlite_schema.sql"))])
}

pub fn create_connection() -> Result<Connection> {
    let path = database_path()?;
    create_connection_at(path)
}

pub fn create_connection_at(path: impl AsRef<Path>) -> Result<Connection> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            DatabaseError::io(
                format!(
                    "Unable to create parent directory '{}' for database file",
                    parent.display()
                ),
                error,
            )
        })?;
    }

    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|error| {
            DatabaseError::io(
                format!("Unable to create or open database '{}'", path.display()),
                error,
            )
        })?;

    let connection = Connection::open(path).map_err(|error| {
        DatabaseError::sqlite(
            format!("Failed to connect to SQLite at '{}'", path.display()),
            error,
        )
    })?;

    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|error| DatabaseError::sqlite("Failed to enable WAL journal mode", error))?;

    connection
        .pragma_update(None, "busy_timeout", 5000)
        .map_err(|error| DatabaseError::sqlite("Failed to set busy timeout", error))?;

    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| DatabaseError::sqlite("Failed to enable foreign keys", error))?;

    Ok(connection)
}

pub fn connect() -> Result<DatabaseConnection> {
    let connection = create_connection()?;
    Ok(Arc::new(Mutex::new(connection)))
}

pub fn connect_at(path: impl AsRef<Path>) -> Result<DatabaseConnection> {
    let connection = create_connection_at(path)?;
    Ok(Arc::new(Mutex::new(connection)))
}

pub fn connect_and_migrate() -> Result<DatabaseConnection> {
    let conn = connect()?;
    apply_migrations(&conn)?;
    Ok(conn)
}

pub fn connect_and_migrate_at(path: impl AsRef<Path>) -> Result<DatabaseConnection> {
    let conn = connect_at(path)?;
    apply_migrations(&conn)?;
    Ok(conn)
}

fn apply_migrations(conn: &DatabaseConnection) -> Result<()> {
    let mut connection = conn.lock().unwrap();
    migrations()
        .to_latest(&mut connection)
        .map_err(DatabaseError::from)?;
    Ok(())
}
