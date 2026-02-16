use anyhow::Result;
use clap::Subcommand;
use database::{EventType, fetch_event_patterns, fetch_tracked_events, insert_tracked_event};

use crate::resolve::{resolve_action, resolve_instance};

#[derive(Debug, Clone, Subcommand)]
pub enum EventsCommand {
    /// List events with optional filters
    List {
        /// Filter by instance (ID, prefix, or title)
        #[arg(long)]
        instance: Option<String>,

        /// Filter by action (ID, prefix, or title)
        #[arg(long)]
        action: Option<String>,

        /// Filter by event type (suggested, accepted, completed, skipped, snoozed, abandoned)
        #[arg(long, value_name = "TYPE")]
        r#type: Option<String>,

        /// Limit the number of results (default: 20)
        #[arg(long, short, default_value = "20")]
        limit: usize,
    },

    /// Record a new event for an instance
    Record {
        /// Instance identifier (ID, prefix, or title)
        identifier: String,

        /// Event type (suggested, accepted, completed, skipped, snoozed, abandoned)
        r#type: String,

        /// Optional note about the event
        #[arg(long)]
        note: Option<String>,
    },

    /// Show completion patterns and statistics
    Patterns {
        /// Filter by action (ID, prefix, or title)
        #[arg(long)]
        action: Option<String>,

        /// Limit the number of results (default: 10)
        #[arg(long, short, default_value = "10")]
        limit: usize,
    },
}

pub fn handle_events_command(cmd: &EventsCommand, conn: &rusqlite::Connection) -> Result<()> {
    match cmd {
        EventsCommand::List {
            instance,
            action,
            r#type,
            limit,
        } => {
            let instance_id = if let Some(id) = instance {
                Some(resolve_instance(conn, id)?)
            } else {
                None
            };

            let action_id = if let Some(id) = action {
                Some(resolve_action(conn, id)?)
            } else {
                None
            };

            let event_type = if let Some(t) = r#type {
                Some(EventType::from_str(t)?)
            } else {
                None
            };

            let events = fetch_tracked_events(
                conn,
                instance_id.as_ref().map(|i| i.id.as_str()),
                action_id.as_ref().map(|a| a.id.as_str()),
                event_type,
                Some(*limit),
            )?;

            if events.is_empty() {
                println!("No events found.");
                return Ok(());
            }

            println!("\n{} Events:", events.len());
            println!("{}", "─".repeat(80));

            for event in events {
                println!("\n🔸 {} | {}", event.event_type, event.id);
                println!("   Occurred: {}", format_timestamp(&event.occurred_at));

                if let Some(iid) = &event.instance_id {
                    let instances = database::fetch_instances(conn)?;
                    if let Some(instance) = instances.iter().find(|i| i.id == *iid) {
                        let actions = database::fetch_actions(conn)?;
                        if let Some(action) = actions.iter().find(|a| a.id == instance.action_id) {
                            println!("   Task: {}", action.title);
                        }
                    }
                }

                if let Some(note) = &event.note {
                    println!("   Note: {}", note);
                }
            }

            println!();
            Ok(())
        }

        EventsCommand::Record {
            identifier,
            r#type,
            note,
        } => {
            let instance = resolve_instance(conn, &identifier)?;
            let event_type = EventType::from_str(&r#type)?;

            let event = insert_tracked_event(
                conn,
                event_type,
                Some(&instance.id),
                Some(&instance.action_id),
                note.as_deref(),
            )?;

            let actions = database::fetch_actions(conn)?;
            let action = actions
                .iter()
                .find(|a| a.id == instance.action_id)
                .ok_or_else(|| anyhow::anyhow!("Action '{}' not found", instance.action_id))?;

            println!("\n✅ Event recorded:");
            println!("   Type: {}", event.event_type);
            println!("   Task: {}", action.title);
            println!("   Time: {}", format_timestamp(&event.occurred_at));

            if let Some(n) = &event.note {
                println!("   Note: {}", n);
            }

            println!();
            Ok(())
        }

        EventsCommand::Patterns { action, limit } => {
            let action_id = if let Some(id) = action {
                Some(resolve_action(conn, id)?)
            } else {
                None
            };

            let patterns = fetch_event_patterns(
                conn,
                action_id.as_ref().map(|a| a.id.as_str()),
                Some(*limit),
            )?;

            if patterns.is_empty() {
                println!("\nNo event patterns found.");
                println!(
                    "Events are only tracked for completion outcomes (completed, skipped, snoozed, abandoned)."
                );
                return Ok(());
            }

            println!("\n📊 Task Completion Patterns:");
            println!("{}", "─".repeat(80));

            for pattern in patterns {
                let completion_pct = (pattern.completion_rate * 100.0).round() as i32;
                let bar_length = (pattern.completion_rate * 20.0).round() as usize;
                let bar = "█".repeat(bar_length);
                let empty_bar = "░".repeat(20 - bar_length);

                println!("\n📝 {}", pattern.action_title);
                println!("   ID: {}", pattern.action_id);
                println!("   Total Events: {}", pattern.total_events);
                println!();
                println!("   ✅ Completed: {}", pattern.completed_count);
                println!("   ⏭️  Skipped:   {}", pattern.skipped_count);
                println!("   ⏰ Snoozed:   {}", pattern.snoozed_count);
                println!("   ❌ Abandoned: {}", pattern.abandoned_count);
                println!();
                println!(
                    "   Completion Rate: {}{}  {}%",
                    bar, empty_bar, completion_pct
                );
            }

            println!();
            Ok(())
        }
    }
}

fn format_timestamp(ts: &str) -> String {
    // SQLite timestamp format: "YYYY-MM-DD HH:MM:SS"
    // Try to parse and format nicely, or just return as-is
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") {
        dt.format("%b %d, %Y at %I:%M %p").to_string()
    } else {
        ts.to_string()
    }
}
