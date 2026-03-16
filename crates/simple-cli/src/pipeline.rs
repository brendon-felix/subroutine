use anyhow::{Result, bail};
use chrono::Local;
use clap::Subcommand;
use rusqlite::Connection;
use simple_core::{Action, ActionCompletion, Event, QueueItem};
use uuid::Uuid;

use crate::actions::{id_matches, short_id};

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

pub fn handle_pipeline(command: &PipelineCommand, conn: &Connection) -> Result<()> {
    match command {
        PipelineCommand::Show => show(conn),
        PipelineCommand::Add { identifier } => add(conn, identifier),
        PipelineCommand::Promote { identifier, time } => promote(conn, identifier, time),
        PipelineCommand::Demote { identifier } => demote(conn, identifier),
        PipelineCommand::Complete { identifier, notes } => {
            complete(conn, identifier, notes.as_deref())
        }
        PipelineCommand::Remove { identifier } => remove(conn, identifier),
    }
}

fn show(conn: &Connection) -> Result<()> {
    let pipeline = simple_db::load_pipeline(conn)?;

    if pipeline.queue.is_empty() && pipeline.backlog.is_empty() {
        println!("The pipeline is empty.");
        return Ok(());
    }

    if !pipeline.queue.is_empty() {
        println!("Queue ({}):", pipeline.queue.len());
        for (i, item) in pipeline.queue.iter().enumerate() {
            let time_str = item
                .time()
                .map(|t| t.with_timezone(&Local).format("%H:%M").to_string())
                .unwrap_or_else(|| "??:??".to_string());
            let tag = match item {
                QueueItem::Action(_) => "action",
                QueueItem::Event(_) => "event",
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

    if !pipeline.backlog.is_empty() {
        println!("Backlog ({}):", pipeline.backlog.len());
        for action in &pipeline.backlog {
            println!("  - {} ({})", action.title, short_id(action.id));
        }
    } else {
        println!("Backlog: (empty)");
    }

    Ok(())
}

fn add(conn: &Connection, identifier: &str) -> Result<()> {
    let item = resolve_any_item(conn, identifier)?;
    let mut pipeline = simple_db::load_pipeline(conn)?;

    match item {
        AnyItem::Action(action) => {
            let title = action.title.clone();
            let id = action.id;
            pipeline.backlog.push(action);
            simple_db::save_pipeline(conn, &pipeline)?;
            println!("Added '{}' ({}) to the backlog.", title, short_id(id));
        }
        AnyItem::Event(event) => {
            let title = event.title.clone();
            let id = event.id;
            pipeline.queue.push(QueueItem::Event(event));
            pipeline
                .queue
                .sort_by_key(|item| item.time().map(|t| t.timestamp()).unwrap_or(i64::MAX));
            simple_db::save_pipeline(conn, &pipeline)?;
            println!("Added '{}' ({}) to the queue.", title, short_id(id));
        }
    }

    Ok(())
}

fn promote(conn: &Connection, identifier: &str, time: &str) -> Result<()> {
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

    let mut pipeline = simple_db::load_pipeline(conn)?;

    let position = pipeline
        .backlog
        .iter()
        .position(|a| id_matches(a.id, &a.title, identifier))
        .ok_or_else(|| anyhow::anyhow!("No matching action found in the backlog."))?;

    let mut action = pipeline.backlog.remove(position);
    let title = action.title.clone();
    let id = action.id;
    action.target = Some(target);
    action.target_static = true;
    pipeline.queue.push(QueueItem::Action(action));
    pipeline
        .queue
        .sort_by_key(|item| item.time().map(|t| t.timestamp()).unwrap_or(i64::MAX));
    simple_db::save_pipeline(conn, &pipeline)?;

    println!(
        "Promoted '{}' ({}) to the queue at {}.",
        title,
        short_id(id),
        time
    );
    Ok(())
}

fn demote(conn: &Connection, identifier: &str) -> Result<()> {
    let mut pipeline = simple_db::load_pipeline(conn)?;

    let position = pipeline
        .queue
        .iter()
        .position(|item| id_matches(item.id(), item.title(), identifier))
        .ok_or_else(|| anyhow::anyhow!("No matching item found in the queue."))?;

    let item = pipeline.queue.remove(position);
    match item {
        QueueItem::Action(mut action) => {
            let title = action.title.clone();
            let id = action.id;
            action.target = None;
            action.target_static = false;
            pipeline.backlog.push(action);
            simple_db::save_pipeline(conn, &pipeline)?;
            println!("Demoted '{}' ({}) to the backlog.", title, short_id(id));
        }
        QueueItem::Event(event) => {
            pipeline.queue.insert(position, QueueItem::Event(event));
            bail!("Events cannot be demoted to the backlog.");
        }
    }

    Ok(())
}

fn complete(conn: &Connection, identifier: &str, notes: Option<&str>) -> Result<()> {
    let mut pipeline = simple_db::load_pipeline(conn)?;

    let position = pipeline
        .queue
        .iter()
        .position(|item| id_matches(item.id(), item.title(), identifier))
        .ok_or_else(|| anyhow::anyhow!("No matching item found in the queue."))?;

    let item = pipeline.queue.remove(position);
    match item {
        QueueItem::Action(action) => {
            let title = action.title.clone();
            let id = action.id;

            let mut completion = ActionCompletion::new(&action);
            if let Some(notes_text) = notes {
                completion = completion.with_notes(notes_text);
            }
            simple_db::insert_action_completion(conn, &completion)?;

            if let Some(next) = action.next_recurrence() {
                simple_db::upsert_action(conn, &next)?;
                pipeline.backlog.push(next);
            }

            simple_db::save_pipeline(conn, &pipeline)?;
            println!("Completed '{}' ({}).", title, short_id(id));
        }
        QueueItem::Event(event) => {
            pipeline.queue.insert(position, QueueItem::Event(event));
            bail!(
                "Events cannot be completed. Use 'pipeline remove' to dismiss an event from the queue."
            );
        }
    }

    Ok(())
}

fn remove(conn: &Connection, identifier: &str) -> Result<()> {
    let mut pipeline = simple_db::load_pipeline(conn)?;

    if let Some(pos) = pipeline
        .backlog
        .iter()
        .position(|a| id_matches(a.id, &a.title, identifier))
    {
        let action = pipeline.backlog.remove(pos);
        simple_db::save_pipeline(conn, &pipeline)?;
        println!(
            "Removed '{}' ({}) from the pipeline.",
            action.title,
            short_id(action.id)
        );
        return Ok(());
    }

    if let Some(pos) = pipeline
        .queue
        .iter()
        .position(|item| id_matches(item.id(), item.title(), identifier))
    {
        let item = pipeline.queue.remove(pos);
        let title = item.title().to_string();
        let id = item.id();

        if let QueueItem::Event(event) = item {
            if let Some(next) = event.next_recurrence() {
                simple_db::upsert_event(conn, &next)?;
                pipeline.queue.push(QueueItem::Event(next));
                pipeline
                    .queue
                    .sort_by_key(|item| item.time().map(|t| t.timestamp()).unwrap_or(i64::MAX));
                simple_db::save_pipeline(conn, &pipeline)?;
                println!(
                    "Removed '{}' ({}) from the pipeline. Next recurrence scheduled.",
                    title,
                    short_id(id)
                );
            } else {
                simple_db::save_pipeline(conn, &pipeline)?;
                println!("Removed '{}' ({}) from the pipeline.", title, short_id(id));
            }
        } else {
            simple_db::save_pipeline(conn, &pipeline)?;
            println!("Removed '{}' ({}) from the pipeline.", title, short_id(id));
        }

        return Ok(());
    }

    bail!("No matching item found in the pipeline.");
}

enum AnyItem {
    Action(Action),
    Event(Event),
}

fn resolve_any_item(conn: &Connection, identifier: &str) -> Result<AnyItem> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        if let Some(action) = simple_db::fetch_action_by_id(conn, uuid)? {
            return Ok(AnyItem::Action(action));
        }
        if let Some(event) = simple_db::fetch_event_by_id(conn, uuid)? {
            return Ok(AnyItem::Event(event));
        }
        bail!("No action or event found with id '{}'.", identifier);
    }

    let actions = simple_db::fetch_actions(conn)?;
    let action_matches: Vec<&Action> = actions
        .iter()
        .filter(|a| id_matches(a.id, &a.title, identifier))
        .collect();

    let events = simple_db::fetch_events(conn)?;
    let event_matches: Vec<&Event> = events
        .iter()
        .filter(|e| id_matches(e.id, &e.title, identifier))
        .collect();

    let total = action_matches.len() + event_matches.len();

    match total {
        0 => bail!("No action or event found matching '{}'.", identifier),
        1 => {
            if let Some(action) = action_matches.into_iter().next() {
                return Ok(AnyItem::Action((*action).clone()));
            }
            if let Some(event) = event_matches.into_iter().next() {
                return Ok(AnyItem::Event((*event).clone()));
            }
            unreachable!()
        }
        _ => bail!(
            "Multiple items match '{}'. Use a more specific identifier.",
            identifier
        ),
    }
}
