use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "parser-cli")]
#[command(about = "Natural language parser test CLI for Subroutine")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Parse input as an action description
    Action {
        input: String,

        #[arg(long)]
        json: bool,

        /// Also build the parsed draft into a real Action entity
        #[arg(long)]
        build: bool,
    },

    /// Parse input as an event description
    Event {
        input: String,

        #[arg(long)]
        json: bool,

        /// Also build the parsed draft into a real Event entity
        #[arg(long)]
        build: bool,
    },
}
