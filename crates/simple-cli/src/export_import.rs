use anyhow::{Context, Result};
use clap::Subcommand;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use simple_core::{Action, Event, Routine};
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

pub fn handle_export_import(
    command: &ExportImportCommand,
    client: &Client,
    base: &str,
) -> Result<()> {
    match command {
        ExportImportCommand::Export { output } => export(client, base, output.as_deref()),
        ExportImportCommand::Import { input } => import(client, base, input.as_deref()),
    }
}

// ── Dump format ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseDump {
    actions: Vec<Action>,
    events: Vec<Event>,
    routines: Vec<Routine>,
}

// ── Export ────────────────────────────────────────────────────────────────────

fn export(client: &Client, base: &str, output: Option<&std::path::Path>) -> Result<()> {
    let dump: DatabaseDump = client
        .get(format!("{}/api/data", base))
        .send()?
        .error_for_status()?
        .json()
        .context("Failed to deserialize API response")?;

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

// ── Import ────────────────────────────────────────────────────────────────────

fn import(client: &Client, base: &str, input: Option<&std::path::Path>) -> Result<()> {
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

    for action in dump.actions {
        client
            .put(format!("{}/api/actions/{}", base, action.id))
            .json(&action)
            .send()?
            .error_for_status()?;
    }

    for event in dump.events {
        client
            .put(format!("{}/api/events/{}", base, event.id))
            .json(&event)
            .send()?
            .error_for_status()?;
    }

    for routine in dump.routines {
        client
            .put(format!("{}/api/routines/{}", base, routine.id))
            .json(&routine)
            .send()?
            .error_for_status()?;
    }

    println!(
        "Imported {} action(s), {} event(s), {} routine(s).",
        action_count, event_count, routine_count
    );

    Ok(())
}
