use anyhow::{Result, bail};
use chrono::{Duration, Local, NaiveTime, TimeZone, Utc};
use clap::Subcommand;
use reqwest::blocking::Client;
use simple_core::Event;
use uuid::Uuid;

use crate::actions::{fetch_api_data, id_matches, short_id};

pub fn fetch_all_events(client: &Client, base: &str) -> Result<Vec<Event>> {
    Ok(fetch_api_data(client, base)?.events)
}

#[derive(Debug, Subcommand)]
pub enum EventsCommand {
    /// List all saved events
    List,
    /// Add a new saved event
    Add {
        title: String,
        /// Time in HH:MM (local time, today)
        time: String,
        #[arg(short, long)]
        content: Option<String>,
        /// Duration in minutes
        #[arg(short, long)]
        duration: Option<i64>,
    },
    /// Delete a saved event (by UUID prefix or title prefix)
    Delete { identifier: String },
}

pub fn handle_events(command: &EventsCommand, client: &Client, base: &str) -> Result<()> {
    match command {
        EventsCommand::List => {
            let events = fetch_all_events(client, base)?;
            if events.is_empty() {
                println!("No saved events.");
                return Ok(());
            }
            for event in &events {
                let local = event.time.with_timezone(&Local);
                println!(
                    "  {} {}  [{}]",
                    short_id(event.id),
                    event.title,
                    local.format("%Y-%m-%d %H:%M")
                );
            }
        }
        EventsCommand::Add {
            title,
            time,
            content,
            duration,
        } => {
            let naive_time = NaiveTime::parse_from_str(time, "%H:%M")
                .map_err(|_| anyhow::anyhow!("Invalid time '{}'. Use HH:MM format.", time))?;
            let today = Local::now().date_naive();
            let naive_dt = today.and_time(naive_time);
            let local_dt = Local
                .from_local_datetime(&naive_dt)
                .single()
                .ok_or_else(|| anyhow::anyhow!("Could not convert local time to UTC"))?;
            let utc_time = local_dt.with_timezone(&Utc);

            let mut event = Event::new(title, utc_time, Duration::minutes(duration.unwrap_or(60)));
            if let Some(c) = content {
                event = event.with_content(c);
            }
            if let Some(mins) = duration {
                event = event.with_duration(Duration::minutes(*mins));
            }
            let id = event.id;
            client
                .put(format!("{}/api/events/{}", base, id))
                .json(&event)
                .send()?
                .error_for_status()?;
            println!("Added event '{}' ({})", title, short_id(id));
        }
        EventsCommand::Delete { identifier } => {
            let event = resolve_event(client, base, identifier)?;
            client
                .delete(format!("{}/api/events/{}", base, event.id))
                .send()?
                .error_for_status()?;
            println!("Deleted event '{}' ({})", event.title, short_id(event.id));
        }
    }
    Ok(())
}

pub fn resolve_event(client: &Client, base: &str, identifier: &str) -> Result<Event> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        let events = fetch_all_events(client, base)?;
        if let Some(event) = events.into_iter().find(|e| e.id == uuid) {
            return Ok(event);
        }
        bail!("No event found with id '{}'", identifier);
    }

    let events = fetch_all_events(client, base)?;
    let matches: Vec<Event> = events
        .into_iter()
        .filter(|e| id_matches(e.id, &e.title, identifier))
        .collect();

    match matches.len() {
        0 => bail!("No event found matching '{}'", identifier),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => bail!(
            "Multiple events match '{}'. Use a more specific identifier.",
            identifier
        ),
    }
}
