use anyhow::{Context, Result};
use clap::Subcommand;
use database::{
    MentalState, MentalStateEvent, fetch_current_mental_state, fetch_mental_state_events,
    fetch_mental_states, insert_mental_state, insert_mental_state_event,
};
use rusqlite::Connection;

use crate::resolve::resolve_mental_state;

#[derive(Debug, Subcommand)]
pub enum MentalStatesCommand {
    /// List all defined mental states
    List,

    /// Create a new mental state definition
    Create {
        /// Name of the mental state (e.g. 'overwhelmed', 'focused', 'anxious')
        name: String,

        /// Description of what this mental state means
        #[arg(long)]
        description: Option<String>,
    },

    /// Record an occurrence of a mental state
    Record {
        /// Mental state name or ID
        identifier: String,

        /// Intensity level (1-5, where 1 is mild and 5 is intense)
        #[arg(long)]
        intensity: Option<i64>,
    },

    /// Show the current (most recent) mental state
    Current,

    /// Show mental state history
    History {
        /// Number of events to show
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

pub fn handle_mental_states_command(
    command: &MentalStatesCommand,
    conn: &Connection,
) -> Result<()> {
    match command {
        MentalStatesCommand::List => {
            let states = fetch_mental_states(conn).context("Failed to fetch mental states")?;

            if states.is_empty() {
                println!("No mental states defined.");
                println!("Create one with: subroutine-cli mental-states create <name>");
            } else {
                println!("Mental states ({}):", states.len());
                for state in states {
                    println!("  {} (id: {})", state, state.id);
                }
            }
        }

        MentalStatesCommand::Create { name, description } => {
            let mut state = MentalState::new(name);
            state.description = description.clone();

            let id = insert_mental_state(conn, &state).context("Failed to create mental state")?;

            println!("✓ Mental state created (id: {})", id);
            println!("  {}", state);
        }

        MentalStatesCommand::Record {
            identifier,
            intensity,
        } => {
            let state = resolve_mental_state(conn, identifier)?;

            // Validate intensity if provided
            if let Some(intensity_value) = intensity {
                if *intensity_value < 1 || *intensity_value > 5 {
                    anyhow::bail!("Intensity must be between 1 and 5");
                }
            }

            let mut event = MentalStateEvent::new(&state.id);
            event.intensity = *intensity;
            event.recorded_at = Some(chrono::Utc::now().to_rfc3339());

            let id = insert_mental_state_event(conn, &event)
                .context("Failed to record mental state event")?;

            println!("✓ Mental state recorded (event id: {})", id);
            print!("  State: {}", state.name);
            if let Some(intensity_value) = intensity {
                println!(" (intensity: {}/5)", intensity_value);
            } else {
                println!();
            }
        }

        MentalStatesCommand::Current => {
            match fetch_current_mental_state(conn)
                .context("Failed to fetch current mental state")?
            {
                Some(state) => {
                    println!("Current mental state:");
                    println!("  {}", state);
                }
                None => {
                    println!("No mental state events recorded.");
                    println!("Record one with: subroutine-cli mental-states record <name>");
                }
            }
        }

        MentalStatesCommand::History { limit } => {
            let events = fetch_mental_state_events(conn, *limit)
                .context("Failed to fetch mental state history")?;

            if events.is_empty() {
                println!("No mental state events found.");
                println!("Record one with: subroutine-cli mental-states record <name>");
            } else {
                println!("Mental state history ({} event(s)):", events.len());

                // Fetch all mental states to map IDs to names
                let states = fetch_mental_states(conn)?;
                let state_map: std::collections::HashMap<String, String> = states
                    .iter()
                    .map(|s| (s.id.clone(), s.name.clone()))
                    .collect();

                for event in events {
                    let state_name = state_map
                        .get(&event.mental_state_id)
                        .map(|n| n.as_str())
                        .unwrap_or("Unknown");

                    print!("  {}", state_name);

                    if let Some(intensity) = event.intensity {
                        print!(" (intensity: {}/5)", intensity);
                    }

                    if let Some(ref recorded_at) = event.recorded_at {
                        print!(" - {}", recorded_at);
                    }

                    println!();
                }
            }
        }
    }

    Ok(())
}
