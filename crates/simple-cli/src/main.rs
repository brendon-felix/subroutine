mod actions;
mod events;
mod export_import;
mod pipeline;
mod routines;

use anyhow::Result;
use clap::{Parser, Subcommand};

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

fn main() -> Result<()> {
    let cli = Cli::parse();

    let conn = simple_db::connect_and_migrate()?;
    let conn = conn.lock().unwrap();

    match &cli.command {
        Commands::Actions { command } => handle_actions(command, &conn),
        Commands::Events { command } => handle_events(command, &conn),
        Commands::Routines { command } => handle_routines(command, &conn),
        Commands::Pipeline { command } => handle_pipeline(command, &conn),
        Commands::Data { command } => handle_export_import(command, &conn),
    }
}
