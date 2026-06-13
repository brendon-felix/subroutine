use anyhow::{Result, bail};
use chrono::Local;
use clap::Subcommand;
use reqwest::blocking::Client;
use serde::Deserialize;
use simple_core::{Action, ActionState, ActionTarget, Event};
use uuid::Uuid;

use crate::actions::{fetch_api_data, id_matches, short_id};

#[derive(Debug, Subcommand)]
pub enum PipelineCommand {
    /// Show the current pipeline
    Show,
    /// Add an action to the backlog or an event to the queue
    Add { identifier: String },
    /// Promote an action from the backlog to the queue (assigns target time)
    Promote {
        identifier: String,
        /// Target time in HH:MM (local time, today)
        time: String,
    },
    /// Demote an action from the queue back to the backlog
    Demote { identifier: String },
    /// Mark a queued action as complete and remove it from the pipeline
    Complete {
        identifier: String,
        #[arg(short, long)]
        notes: Option<String>,
    },
    /// Remove an item from the pipeline without completing it
    Remove { identifier: String },
}

pub fn handle_pipeline(command: &PipelineCommand, client: &Client, base: &str) -> Result<()> {
    match command {
        PipelineCommand::Show => show(client, base),
        PipelineCommand::Add { identifier } => add(client, base, identifier),
        PipelineCommand::Promote { identifier, time } => promote(client, base, identifier, time),
        PipelineCommand::Demote { identifier } => demote(client, base, identifier),
        PipelineCommand::Complete { identifier, notes } => {
            complete(client, base, identifier, notes.as_deref())
        }
        PipelineCommand::Remove { identifier } => remove(client, base, identifier),
    }
}

// ── Local AnyItem ─────────────────────────────────────────────────────────────

enum AnyItem {
    Action(Action),
    Event(Event),
}

impl AnyItem {
    fn id(&self) -> Uuid {
        match self {
            AnyItem::Action(a) => a.id,
            AnyItem::Event(e) => e.id,
        }
    }

    fn title(&self) -> &str {
        match self {
            AnyItem::Action(a) => &a.title,
            AnyItem::Event(e) => &e.title,
        }
    }

    fn time(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        match self {
            AnyItem::Action(a) => {
                if let ActionState::Scheduled(target) = &a.state {
                    Some(target.time)
                } else {
                    None
                }
            }
            AnyItem::Event(e) => Some(e.time),
        }
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

fn show(client: &Client, base: &str) -> Result<()> {
    let data = fetch_api_data(client, base)?;

    // Build the queue: queued actions + all events, sorted by time.
    let mut queue: Vec<AnyItem> = Vec::new();

    for action in data.actions {
        if matches!(action.state, ActionState::Scheduled(_)) {
            queue.push(AnyItem::Action(action));
        }
    }
    for event in &data.events {
        queue.push(AnyItem::Event(event.clone()));
    }

    queue.sort_by_key(|item| item.time());

    let backlog: Vec<Action> = {
        // Re-fetch only backlogged actions from the already-fetched data.
        // We already consumed data above, so we need to rebuild from queue + a separate pass.
        // Actually we consumed data.actions above — fetch again for backlog.
        fetch_api_data(client, base)?
            .actions
            .into_iter()
            .filter(|a| a.is_backlogged())
            .collect()
    };

    if queue.is_empty() && backlog.is_empty() {
        println!("The pipeline is empty.");
        return Ok(());
    }

    if !queue.is_empty() {
        println!("Queue ({}):", queue.len());
        for (i, item) in queue.iter().enumerate() {
            let time_str = item
                .time()
                .map(|t| t.with_timezone(&Local).format("%H:%M").to_string())
                .unwrap_or_else(|| "??:??".to_string());
            let tag = match item {
                AnyItem::Action(_) => "action",
                AnyItem::Event(_) => "event",
            };
            println!(
                "  {}. [{}] {} {} ({})",
                i + 1,
                time_str,
                tag,
                item.title(),
                short_id(item.id()),
            );
        }
    } else {
        println!("Queue: (empty)");
    }

    println!();

    if !backlog.is_empty() {
        println!("Backlog ({}):", backlog.len());
        for action in &backlog {
            println!("  - {} ({})", action.title, short_id(action.id));
        }
    } else {
        println!("Backlog: (empty)");
    }

    Ok(())
}

fn add(client: &Client, base: &str, identifier: &str) -> Result<()> {
    let item = resolve_any_item(client, base, identifier)?;

    match item {
        AnyItem::Action(mut action) => {
            let title = action.title.clone();
            let id = action.id;
            action.backlog(None);
            client
                .put(format!("{}/api/actions/{}", base, id))
                .json(&action)
                .send()?
                .error_for_status()?;
            println!("Added '{}' ({}) to the backlog.", title, short_id(id));
        }
        AnyItem::Event(event) => {
            let title = event.title.clone();
            let id = event.id;
            client
                .put(format!("{}/api/events/{}", base, id))
                .json(&event)
                .send()?
                .error_for_status()?;
            println!("Added '{}' ({}) to the queue.", title, short_id(id));
        }
    }

    Ok(())
}

fn promote(client: &Client, base: &str, identifier: &str, time: &str) -> Result<()> {
    use chrono::{NaiveTime, TimeZone, Utc};

    let naive_time = NaiveTime::parse_from_str(time, "%H:%M")
        .map_err(|_| anyhow::anyhow!("Invalid time '{}'. Use HH:MM format.", time))?;
    let today = Local::now().date_naive();
    let naive_dt = today.and_time(naive_time);
    let local_dt = Local
        .from_local_datetime(&naive_dt)
        .single()
        .ok_or_else(|| anyhow::anyhow!("Could not convert local time to UTC."))?;
    let target = local_dt.with_timezone(&Utc);

    let data = fetch_api_data(client, base)?;
    let action = data
        .actions
        .into_iter()
        .filter(|a| a.is_backlogged())
        .find(|a| id_matches(a.id, &a.title, identifier))
        .ok_or_else(|| anyhow::anyhow!("No matching action found in the backlog."))?;

    let title = action.title.clone();
    let id = action.id;
    let updated = action.with_state(ActionState::Scheduled(ActionTarget {
        time: target,
        is_static: true,
    }));
    client
        .put(format!("{}/api/actions/{}", base, id))
        .json(&updated)
        .send()?
        .error_for_status()?;

    println!(
        "Promoted '{}' ({}) to the queue at {}.",
        title,
        short_id(id),
        time
    );
    Ok(())
}

fn demote(client: &Client, base: &str, identifier: &str) -> Result<()> {
    let data = fetch_api_data(client, base)?;

    let action = data
        .actions
        .into_iter()
        .filter(|a| matches!(a.state, ActionState::Scheduled(_)))
        .find(|a| id_matches(a.id, &a.title, identifier))
        .ok_or_else(|| anyhow::anyhow!("No matching action found in the queue."))?;

    let title = action.title.clone();
    let id = action.id;
    client
        .post(format!("{}/api/actions/{}/backlog", base, id))
        .json(&serde_json::json!({}))
        .send()?
        .error_for_status()?;

    println!("Demoted '{}' ({}) to the backlog.", title, short_id(id));
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CompleteResponse {
    #[allow(dead_code)]
    completed: Action,
    next: Option<Action>,
}

fn complete(client: &Client, base: &str, identifier: &str, notes: Option<&str>) -> Result<()> {
    let data = fetch_api_data(client, base)?;

    let action = data
        .actions
        .into_iter()
        .filter(|a| matches!(a.state, ActionState::Scheduled(_)))
        .find(|a| id_matches(a.id, &a.title, identifier))
        .ok_or_else(|| anyhow::anyhow!("No matching action found in the queue."))?;

    let title = action.title.clone();
    let id = action.id;

    if notes.is_some() {
        eprintln!("Note: completion notes are not yet persisted.");
    }

    let resp: CompleteResponse = client
        .post(format!("{}/api/actions/{}/complete", base, id))
        .json(&serde_json::json!({}))
        .send()?
        .error_for_status()?
        .json()?;

    if let Some(next) = resp.next {
        println!(
            "Completed '{}' ({}). Next recurrence scheduled: {} ({}).",
            title,
            short_id(id),
            next.title,
            short_id(next.id)
        );
    } else {
        println!("Completed '{}' ({}).", title, short_id(id));
    }

    Ok(())
}

fn remove(client: &Client, base: &str, identifier: &str) -> Result<()> {
    let data = fetch_api_data(client, base)?;

    // Check backlog first.
    if let Some(action) = data
        .actions
        .iter()
        .filter(|a| a.is_backlogged())
        .find(|a| id_matches(a.id, &a.title, identifier))
    {
        let title = action.title.clone();
        let id = action.id;
        client
            .delete(format!("{}/api/actions/{}", base, id))
            .send()?
            .error_for_status()?;
        println!("Removed '{}' ({}) from the pipeline.", title, short_id(id));
        return Ok(());
    }

    // Then check queued actions.
    if let Some(action) = data
        .actions
        .iter()
        .filter(|a| matches!(a.state, ActionState::Scheduled(_)))
        .find(|a| id_matches(a.id, &a.title, identifier))
    {
        let title = action.title.clone();
        let id = action.id;
        client
            .delete(format!("{}/api/actions/{}", base, id))
            .send()?
            .error_for_status()?;
        println!("Removed '{}' ({}) from the pipeline.", title, short_id(id));
        return Ok(());
    }

    // Then check events.
    if let Some(event) = data
        .events
        .iter()
        .find(|e| id_matches(e.id, &e.title, identifier))
    {
        let title = event.title.clone();
        let id = event.id;
        client
            .delete(format!("{}/api/events/{}", base, id))
            .send()?
            .error_for_status()?;
        println!("Removed '{}' ({}) from the pipeline.", title, short_id(id));
        return Ok(());
    }

    bail!("No matching item found in the pipeline.");
}

fn resolve_any_item(client: &Client, base: &str, identifier: &str) -> Result<AnyItem> {
    let data = fetch_api_data(client, base)?;

    if let Ok(uuid) = Uuid::parse_str(identifier) {
        if let Some(action) = data.actions.into_iter().find(|a| a.id == uuid) {
            return Ok(AnyItem::Action(action));
        }
        if let Some(event) = data.events.into_iter().find(|e| e.id == uuid) {
            return Ok(AnyItem::Event(event));
        }
        bail!("No action or event found with id '{}'.", identifier);
    }

    let action_matches: Vec<Action> = data
        .actions
        .into_iter()
        .filter(|a| id_matches(a.id, &a.title, identifier))
        .collect();

    let event_matches: Vec<Event> = data
        .events
        .into_iter()
        .filter(|e| id_matches(e.id, &e.title, identifier))
        .collect();

    let total = action_matches.len() + event_matches.len();

    match total {
        0 => bail!("No action or event found matching '{}'.", identifier),
        1 => {
            if let Some(action) = action_matches.into_iter().next() {
                return Ok(AnyItem::Action(action));
            }
            if let Some(event) = event_matches.into_iter().next() {
                return Ok(AnyItem::Event(event));
            }
            unreachable!()
        }
        _ => bail!(
            "Multiple items match '{}'. Use a more specific identifier.",
            identifier
        ),
    }
}
