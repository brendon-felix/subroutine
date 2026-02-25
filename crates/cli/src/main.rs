mod actions;
mod mental_states;
mod pipeline;
mod routines;
mod saved_actions;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::actions::{ActionsCommand, handle_actions_command};
use crate::mental_states::{MentalStatesCommand, handle_mental_states_command};
use crate::pipeline::{PipelineCommand, handle_pipeline_command};
use crate::routines::{RoutinesCommand, handle_routines_command};
use crate::saved_actions::{SavedActionsCommand, handle_saved_actions_command};

#[derive(Debug, Subcommand)]
enum Commands {
    /// Manage concrete actions in the pipeline
    Actions {
        #[command(subcommand)]
        command: ActionsCommand,
    },
    /// Manage saved action templates
    SavedActions {
        #[command(subcommand)]
        command: SavedActionsCommand,
    },
    /// Manage saved mental states
    MentalStates {
        #[command(subcommand)]
        command: MentalStatesCommand,
    },
    /// Manage the pipeline (backlog and queue)
    Pipeline {
        #[command(subcommand)]
        command: PipelineCommand,
    },
    /// Manage routines
    Routines {
        #[command(subcommand)]
        command: RoutinesCommand,
    },
}

#[derive(Parser, Debug)]
#[command(version, about = "Subroutine — an executive function prosthetic", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let conn = database::connect_and_migrate()?;
    let conn = conn.lock().unwrap();

    match &cli.command {
        Some(Commands::Actions { command }) => {
            handle_actions_command(command, &conn)?;
        }
        Some(Commands::SavedActions { command }) => {
            handle_saved_actions_command(command, &conn)?;
        }
        Some(Commands::MentalStates { command }) => {
            handle_mental_states_command(command, &conn)?;
        }
        Some(Commands::Pipeline { command }) => {
            handle_pipeline_command(command, &conn)?;
        }
        Some(Commands::Routines { command }) => {
            handle_routines_command(command, &conn)?;
        }
        None => {
            println!("Use --help to see available commands.");
        }
    }

    Ok(())
}
