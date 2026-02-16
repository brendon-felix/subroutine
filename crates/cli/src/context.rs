use anyhow::{Context, Result};
use clap::Subcommand;
use database::{
    ContextSnapshot, fetch_context_snapshots, fetch_current_context, insert_context_snapshot,
    update_context_attention, update_context_energy,
};
use rusqlite::Connection;

#[derive(Debug, Subcommand)]
pub enum ContextCommand {
    /// Capture a new context snapshot
    Snapshot {
        /// Environment (e.g. 'quiet', 'noisy', 'social')
        #[arg(long)]
        env: Option<String>,

        /// Location (e.g. 'home', 'work', 'cafe')
        #[arg(long)]
        location: Option<String>,

        /// Device being used (e.g. 'laptop', 'phone')
        #[arg(long)]
        device: Option<String>,

        /// Time of day (e.g. 'morning', 'afternoon', 'evening', 'night')
        #[arg(long)]
        time: Option<String>,

        /// Day type (e.g. 'weekday', 'weekend')
        #[arg(long)]
        day_type: Option<String>,
    },

    /// Show the current (most recent) context snapshot
    Current,

    /// Show context snapshot history
    History {
        /// Number of snapshots to show
        #[arg(long, default_value = "10")]
        limit: usize,
    },

    /// Update energy level in current context (0.0 to 1.0)
    SetEnergy {
        /// Energy level from 0.0 (depleted) to 1.0 (fully energized)
        energy: f64,
    },

    /// Update attention level in current context (0.0 to 1.0)
    SetAttention {
        /// Attention level from 0.0 (scattered) to 1.0 (hyperfocused)
        attention: f64,
    },
}

pub fn handle_context_command(command: &ContextCommand, conn: &Connection) -> Result<()> {
    match command {
        ContextCommand::Snapshot {
            env,
            location,
            device,
            time,
            day_type,
        } => {
            let mut snapshot = ContextSnapshot::new();
            snapshot.recorded_at = Some(chrono::Utc::now().to_rfc3339());
            snapshot.environment = env.clone();
            snapshot.location = location.clone();
            snapshot.device = device.clone();
            snapshot.time_of_day = time.clone();
            snapshot.day_type = day_type.clone();

            let id = insert_context_snapshot(conn, &snapshot)
                .context("Failed to insert context snapshot")?;

            println!("✓ Context snapshot captured (id: {})", id);
            println!("{}", snapshot);
        }

        ContextCommand::Current => {
            match fetch_current_context(conn).context("Failed to fetch current context")? {
                Some(snapshot) => {
                    println!("Current context:");
                    println!("{}", snapshot);

                    if let Some(ref metadata) = snapshot.metadata {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(metadata) {
                            if let Some(energy) = json.get("energy") {
                                println!("  Energy: {}", energy);
                            }
                            if let Some(attention) = json.get("attention") {
                                println!("  Attention: {}", attention);
                            }
                        }
                    }
                }
                None => {
                    println!("No context snapshots found.");
                    println!("Create one with: subroutine-cli context snapshot");
                }
            }
        }

        ContextCommand::History { limit } => {
            let snapshots =
                fetch_context_snapshots(conn, *limit).context("Failed to fetch context history")?;

            if snapshots.is_empty() {
                println!("No context snapshots found.");
                println!("Create one with: subroutine-cli context snapshot");
            } else {
                println!("Context history ({} snapshot(s)):", snapshots.len());
                for snapshot in snapshots {
                    println!("  {}", snapshot);
                }
            }
        }

        ContextCommand::SetEnergy { energy } => {
            if *energy < 0.0 || *energy > 1.0 {
                anyhow::bail!("Energy must be between 0.0 and 1.0");
            }

            update_context_energy(conn, *energy).context("Failed to update energy level")?;
            println!("✓ Energy level updated to {:.1}%", energy * 100.0);
        }

        ContextCommand::SetAttention { attention } => {
            if *attention < 0.0 || *attention > 1.0 {
                anyhow::bail!("Attention must be between 0.0 and 1.0");
            }

            update_context_attention(conn, *attention)
                .context("Failed to update attention level")?;
            println!("✓ Attention level updated to {:.1}%", attention * 100.0);
        }
    }

    Ok(())
}
