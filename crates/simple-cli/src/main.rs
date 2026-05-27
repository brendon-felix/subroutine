mod actions;
mod events;
mod export_import;
mod pipeline;
mod routines;

use anyhow::Result;
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;

use crate::actions::{ActionsCommand, handle_actions};
use crate::events::{EventsCommand, handle_events};
use crate::export_import::{ExportImportCommand, handle_export_import};
use crate::pipeline::{PipelineCommand, handle_pipeline};
use crate::routines::{RoutinesCommand, handle_routines};

#[derive(Parser, Debug)]
#[command(name = "simple", about = "Simple subroutine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Manage saved actions
    Actions {
        #[command(subcommand)]
        command: ActionsCommand,
    },
    /// Manage saved events
    Events {
        #[command(subcommand)]
        command: EventsCommand,
    },
    /// Manage routines
    Routines {
        #[command(subcommand)]
        command: RoutinesCommand,
    },
    /// Manage the pipeline (backlog and queue)
    Pipeline {
        #[command(subcommand)]
        command: PipelineCommand,
    },
    /// Export all data to JSON or import from JSON
    Data {
        #[command(subcommand)]
        command: ExportImportCommand,
    },
}

fn server_base_url() -> String {
    if let Ok(url) = std::env::var("SUBROUTINE_SERVER_URL") {
        return url;
    }
    if let Ok(host) = std::env::var("SUBROUTINE_HOST") {
        let port: u16 = std::env::var("SUBROUTINE_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);
        return format!("http://{}:{}", host, port);
    }
    "http://localhost:3000".to_string()
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let base_url = server_base_url();
    let client = Client::new();

    match &cli.command {
        Commands::Actions { command } => handle_actions(command, &client, &base_url),
        Commands::Events { command } => handle_events(command, &client, &base_url),
        Commands::Routines { command } => handle_routines(command, &client, &base_url),
        Commands::Pipeline { command } => handle_pipeline(command, &client, &base_url),
        Commands::Data { command } => handle_export_import(command, &client, &base_url),
    }
}
