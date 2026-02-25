use anyhow::{Result, bail};
use app_core::{Context, MentalState, Pipeline, PipelineEntry};
use clap::Subcommand;
use rusqlite::Connection;
use std::collections::HashSet;
use uuid::Uuid;

use crate::actions::resolve_action;
use crate::saved_actions::resolve_saved_action;

#[derive(Debug, Subcommand)]
pub enum PipelineCommand {
    /// Show the current pipeline (backlog and queue)
    Show {
        /// Also show scores for each entry (requires a mental state)
        #[arg(long)]
        scores: bool,

        /// Mental state UUID prefix or name to use for scoring
        #[arg(long)]
        mental_state: Option<String>,
    },

    /// Refresh the pipeline: score all entries and auto-promote/demote based on threshold
    Refresh {
        /// Mental state UUID prefix or name to use for scoring
        #[arg(long)]
        mental_state: Option<String>,

        /// Remaining spoons to use for scoring (default: 10)
        #[arg(long, default_value = "10")]
        spoons: u32,
    },

    /// Instantiate a saved action and add it to the pipeline backlog
    Add {
        /// Saved action UUID prefix or title prefix
        identifier: String,
    },

    /// Remove an entry from the pipeline entirely (from backlog or queue)
    Remove {
        /// Action UUID prefix or title prefix
        identifier: String,
    },

    /// Manually promote an entry from the backlog to the queue
    Promote {
        /// Action UUID prefix or title prefix
        identifier: String,
    },

    /// Manually demote an entry from the queue back to the backlog
    Demote {
        /// Action UUID prefix or title prefix
        identifier: String,
    },
}

pub fn handle_pipeline_command(command: &PipelineCommand, conn: &Connection) -> Result<()> {
    match command {
        PipelineCommand::Show {
            scores,
            mental_state,
        } => show_pipeline(conn, *scores, mental_state.as_deref()),
        PipelineCommand::Refresh {
            mental_state,
            spoons,
        } => refresh_pipeline(conn, mental_state.as_deref(), *spoons),
        PipelineCommand::Add { identifier } => add_to_pipeline(conn, identifier),
        PipelineCommand::Remove { identifier } => remove_from_pipeline(conn, identifier),
        PipelineCommand::Promote { identifier } => promote_entry(conn, identifier),
        PipelineCommand::Demote { identifier } => demote_entry(conn, identifier),
    }
}

fn show_pipeline(
    conn: &Connection,
    show_scores: bool,
    mental_state_identifier: Option<&str>,
) -> Result<()> {
    let pipeline = database::load_pipeline(conn)?;

    let context = if show_scores || mental_state_identifier.is_some() {
        Some(build_context(
            conn,
            mental_state_identifier,
            app_core::MAX_SPOONS,
        )?)
    } else {
        None
    };

    let completed_ids: HashSet<Uuid> = HashSet::new();

    let queue = pipeline.queue();
    let backlog = pipeline.backlog();

    if queue.is_empty() && backlog.is_empty() {
        println!("The pipeline is empty.");
        println!("Use 'pipeline add <action>' to add an action.");
        return Ok(());
    }

    if !queue.is_empty() {
        println!("Queue ({}):", queue.len());
        for (position, entry) in queue.iter().enumerate() {
            if entry.is_transition() {
                continue;
            }
            let score_str =
                if let (Some(ctx), Some(actionable)) = (context.as_ref(), entry.as_actionable()) {
                    let _ = actionable;
                    let breakdown = app_core::score(entry, ctx, &completed_ids);
                    format!("  [score: {:.2}]", breakdown.total)
                } else {
                    String::new()
                };
            println!(
                "  {}. {} ({}){}",
                position + 1,
                entry.title(),
                &entry.id().to_string()[..8],
                score_str
            );
        }
    } else {
        println!("Queue: (empty)");
    }

    println!();

    if !backlog.is_empty() {
        println!("Backlog ({}):", backlog.len());
        for entry in backlog {
            let score_str =
                if let (Some(ctx), Some(_actionable)) = (context.as_ref(), entry.as_actionable()) {
                    let breakdown = app_core::score(entry, ctx, &completed_ids);
                    format!("  [score: {:.2}]", breakdown.total)
                } else {
                    String::new()
                };
            println!(
                "  - {} ({}){}",
                entry.title(),
                &entry.id().to_string()[..8],
                score_str
            );
        }
    } else {
        println!("Backlog: (empty)");
    }

    Ok(())
}

fn refresh_pipeline(
    conn: &Connection,
    mental_state_identifier: Option<&str>,
    spoons: u32,
) -> Result<()> {
    let mut pipeline = database::load_pipeline(conn)?;
    let context = build_context(conn, mental_state_identifier, spoons)?;
    let completed_ids: HashSet<Uuid> = HashSet::new();

    let queue_before = pipeline.queue().len();
    let backlog_before = pipeline.backlog().len();

    pipeline.refresh(&context, &completed_ids);

    let queue_after = pipeline.queue().len();
    let backlog_after = pipeline.backlog().len();

    database::save_pipeline(conn, &pipeline)?;

    let promoted = queue_after.saturating_sub(queue_before);
    let demoted = backlog_after.saturating_sub(backlog_before);

    println!(
        "Pipeline refreshed: {} promoted to queue, {} demoted to backlog.",
        promoted, demoted
    );
    println!(
        "Queue: {} entries | Backlog: {} entries",
        queue_after, backlog_after
    );

    Ok(())
}

fn add_to_pipeline(conn: &Connection, identifier: &str) -> Result<()> {
    let saved = resolve_saved_action(conn, identifier)?;
    let title = saved.title.clone();

    // Instantiate a new concrete action from the saved template.
    let action = saved.instantiate();
    let id = action.id;

    database::insert_action(conn, &action)?;

    let mut pipeline = database::load_pipeline(conn)?;
    pipeline.push(PipelineEntry::Action(action))?;
    database::save_pipeline(conn, &pipeline)?;

    println!(
        "Instantiated '{}' ({}) and added to the pipeline backlog.",
        title,
        &id.to_string()[..8]
    );
    Ok(())
}

fn remove_from_pipeline(conn: &Connection, identifier: &str) -> Result<()> {
    let action = resolve_action(conn, identifier)?;
    let title = action.title.clone();
    let id = action.id;

    let mut pipeline = database::load_pipeline(conn)?;

    // Check queue first, then backlog
    let in_queue = pipeline.queue().iter().any(|e| e.id() == id);
    let in_backlog = pipeline.backlog().iter().any(|e| e.id() == id);

    if !in_queue && !in_backlog {
        bail!("'{}' is not in the pipeline.", title);
    }

    if in_queue {
        pipeline.demote(id)?;
    }

    // Now it's in the backlog — rebuild without it
    let new_backlog: Vec<PipelineEntry> = pipeline
        .backlog()
        .iter()
        .filter(|e| e.id() != id)
        .cloned()
        .collect();

    let queue_entries: Vec<PipelineEntry> = pipeline.queue().to_vec();

    let mut new_pipeline = Pipeline::new().with_promotion_threshold(pipeline.promotion_threshold());
    for entry in new_backlog {
        new_pipeline.push(entry)?;
    }
    for entry in queue_entries {
        if !entry.is_transition() {
            new_pipeline.push(entry.clone())?;
            new_pipeline.promote(entry.id())?;
        }
    }

    database::save_pipeline(conn, &new_pipeline)?;

    println!(
        "Removed '{}' ({}) from the pipeline.",
        title,
        &id.to_string()[..8]
    );
    Ok(())
}

fn promote_entry(conn: &Connection, identifier: &str) -> Result<()> {
    let action = resolve_action(conn, identifier)?;
    let title = action.title.clone();
    let id = action.id;

    let mut pipeline = database::load_pipeline(conn)?;

    let in_backlog = pipeline.backlog().iter().any(|e| e.id() == id);
    if !in_backlog {
        let in_queue = pipeline.queue().iter().any(|e| e.id() == id);
        if in_queue {
            bail!("'{}' is already in the queue.", title);
        }
        bail!("'{}' is not in the pipeline backlog.", title);
    }

    pipeline.promote(id)?;
    database::save_pipeline(conn, &pipeline)?;

    println!(
        "Promoted '{}' ({}) to the queue.",
        title,
        &id.to_string()[..8]
    );
    Ok(())
}

fn demote_entry(conn: &Connection, identifier: &str) -> Result<()> {
    let action = resolve_action(conn, identifier)?;
    let title = action.title.clone();
    let id = action.id;

    let mut pipeline = database::load_pipeline(conn)?;

    let in_queue = pipeline.queue().iter().any(|e| e.id() == id);
    if !in_queue {
        let in_backlog = pipeline.backlog().iter().any(|e| e.id() == id);
        if in_backlog {
            bail!("'{}' is already in the backlog.", title);
        }
        bail!("'{}' is not in the pipeline queue.", title);
    }

    pipeline.demote(id)?;
    database::save_pipeline(conn, &pipeline)?;

    println!(
        "Demoted '{}' ({}) to the backlog.",
        title,
        &id.to_string()[..8]
    );
    Ok(())
}

fn build_context(
    conn: &Connection,
    mental_state_identifier: Option<&str>,
    spoons: u32,
) -> Result<Context> {
    let mental_state = if let Some(identifier) = mental_state_identifier {
        let saved = crate::mental_states::resolve_mental_state(conn, identifier)?;
        MentalState::new(spoons).with_declared(saved)
    } else {
        MentalState::new(spoons)
    };

    Ok(Context::new(mental_state))
}
