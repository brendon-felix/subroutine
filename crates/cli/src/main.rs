mod actions;
mod context;
mod events;
mod instances;
mod interactive;
mod mental_states;
mod pipeline;
mod resolve;
mod routines;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::actions::{ActionsCommand, handle_actions_command};
use crate::context::{ContextCommand, handle_context_command};
use crate::events::{EventsCommand, handle_events_command};
use crate::instances::{InstancesCommand, handle_instances_command};
use crate::interactive::interactive_mode;
use crate::mental_states::{MentalStatesCommand, handle_mental_states_command};
use crate::pipeline::{PipelineCommand, handle_pipeline_command};
use crate::routines::{RoutinesCommand, handle_routines_command};

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start interactive mode - guided workflows for common tasks
    Interactive,
    /// Manage actions (tasks, habits, etc.)
    Actions {
        #[command(subcommand)]
        command: ActionsCommand,
    },
    /// Manage instances of actions
    Instances {
        #[command(subcommand)]
        command: InstancesCommand,
    },
    /// Manage context snapshots and current context
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },
    /// Manage mental states and record mental state events
    MentalStates {
        #[command(subcommand)]
        command: MentalStatesCommand,
    },
    /// Manage the pipeline (smart task queue)
    Pipeline {
        #[command(subcommand)]
        command: PipelineCommand,
    },
    /// Track and analyze events for learning and pattern recognition
    Events {
        #[command(subcommand)]
        command: EventsCommand,
    },
    /// Manage routines (modular task sequences and templates)
    Routines {
        #[command(subcommand)]
        command: RoutinesCommand,
    },
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut conn = database::create_connection()?;
    database::migrations()
        .to_latest(&mut conn)
        .context("Applying database migrations failed")?;

    match &cli.command {
        Some(Commands::Interactive) => {
            interactive_mode(&conn)?;
        }
        Some(Commands::Actions { command }) => {
            handle_actions_command(command, &conn)?;
        }
        Some(Commands::Instances { command }) => {
            handle_instances_command(command, &conn)?;
        }
        Some(Commands::Context { command }) => {
            handle_context_command(command, &conn)?;
        }
        Some(Commands::MentalStates { command }) => {
            handle_mental_states_command(command, &conn)?;
        }
        Some(Commands::Pipeline { command }) => {
            handle_pipeline_command(command, &conn)?;
        }
        Some(Commands::Events { command }) => {
            handle_events_command(command, &conn)?;
        }
        Some(Commands::Routines { command }) => {
            handle_routines_command(command, &conn)?;
        }
        None => {
            // No command provided - start interactive mode by default
            interactive_mode(&conn)?;
        }
    }

    Ok(())
}
