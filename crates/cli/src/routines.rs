use anyhow::{Result, bail};
use app_core::{Routine, SavedStep};
use clap::Subcommand;
use rusqlite::Connection;
use uuid::Uuid;

use crate::saved_actions::resolve_saved_action;

#[derive(Debug, Subcommand)]
pub enum RoutinesCommand {
    /// List all routines
    List,

    /// Show details for a routine (by ID prefix or title prefix)
    Show {
        /// UUID prefix or title prefix
        identifier: String,
    },

    /// Create a new routine
    Create {
        /// Title of the routine
        title: String,

        /// Optional description
        #[arg(short, long)]
        content: Option<String>,
    },

    /// Delete a routine (by ID prefix or title prefix)
    Delete {
        /// UUID prefix or title prefix
        identifier: String,
    },

    /// Add a saved action as a step to a routine
    AddStep {
        /// Routine UUID prefix or title prefix
        routine: String,

        /// Saved action UUID prefix or title prefix
        action: String,
    },

    /// Remove a saved action step from a routine
    RemoveStep {
        /// Routine UUID prefix or title prefix
        routine: String,

        /// Saved action UUID prefix or title prefix
        action: String,
    },
}

pub fn handle_routines_command(command: &RoutinesCommand, conn: &Connection) -> Result<()> {
    match command {
        RoutinesCommand::List => list_routines(conn),
        RoutinesCommand::Show { identifier } => show_routine(conn, identifier),
        RoutinesCommand::Create { title, content } => {
            create_routine(conn, title, content.as_deref())
        }
        RoutinesCommand::Delete { identifier } => delete_routine(conn, identifier),
        RoutinesCommand::AddStep { routine, action } => add_step(conn, routine, action),
        RoutinesCommand::RemoveStep { routine, action } => remove_step(conn, routine, action),
    }
}

fn list_routines(conn: &Connection) -> Result<()> {
    let routines = database::fetch_routines(conn)?;

    if routines.is_empty() {
        println!("No routines found.");
        return Ok(());
    }

    println!("Routines ({}):", routines.len());
    for routine in &routines {
        let step_count = routine.steps.len();
        let steps_label = if step_count == 1 { "step" } else { "steps" };
        println!(
            "  {} {} ({} {})",
            &routine.id.to_string()[..8],
            routine.title,
            step_count,
            steps_label
        );
    }

    Ok(())
}

fn show_routine(conn: &Connection, identifier: &str) -> Result<()> {
    let routine = resolve_routine(conn, identifier)?;

    println!("Routine Details:");
    println!("  ID:      {}", routine.id);
    println!("  Title:   {}", routine.title);
    println!(
        "  Created: {}",
        routine.created_at.format("%Y-%m-%d %H:%M UTC")
    );

    if let Some(ref content) = routine.content {
        println!("  Content: {}", content);
    }

    println!();

    if routine.steps.is_empty() {
        println!("  Steps: (none)");
    } else {
        println!("  Steps ({}):", routine.steps.len());
        for (index, step) in routine.steps.iter().enumerate() {
            match step {
                SavedStep::Action(id) => match database::fetch_saved_action_by_id(conn, *id)? {
                    Some(saved) => println!(
                        "    {}. [action] {} ({})",
                        index + 1,
                        saved.title,
                        &id.to_string()[..8]
                    ),
                    None => println!(
                        "    {}. [action] <missing saved action: {}>",
                        index + 1,
                        &id.to_string()[..8]
                    ),
                },
                SavedStep::Event(id) => {
                    println!("    {}. [event] <{}>", index + 1, &id.to_string()[..8]);
                }
            }
        }
    }

    Ok(())
}

fn create_routine(conn: &Connection, title: &str, content: Option<&str>) -> Result<()> {
    let mut routine = Routine::new(title);

    if let Some(c) = content {
        routine = routine.with_content(c);
    }

    let id = routine.id;
    database::insert_routine(conn, &routine)?;
    println!("Created routine '{}' ({})", title, &id.to_string()[..8]);
    Ok(())
}

fn delete_routine(conn: &Connection, identifier: &str) -> Result<()> {
    let routine = resolve_routine(conn, identifier)?;
    database::delete_routine(conn, routine.id)?;
    println!(
        "Deleted routine '{}' ({})",
        routine.title,
        &routine.id.to_string()[..8]
    );
    Ok(())
}

fn add_step(conn: &Connection, routine_identifier: &str, action_identifier: &str) -> Result<()> {
    let mut routine = resolve_routine(conn, routine_identifier)?;
    let saved = resolve_saved_action(conn, action_identifier)?;
    let step = SavedStep::Action(saved.id);

    if routine.steps.contains(&step) {
        bail!(
            "'{}' is already a step in routine '{}'.",
            saved.title,
            routine.title
        );
    }

    routine.steps.push(step);
    database::insert_routine(conn, &routine)?;

    println!(
        "Added '{}' as step {} in routine '{}'.",
        saved.title,
        routine.steps.len(),
        routine.title
    );
    Ok(())
}

fn remove_step(conn: &Connection, routine_identifier: &str, action_identifier: &str) -> Result<()> {
    let mut routine = resolve_routine(conn, routine_identifier)?;
    let saved = resolve_saved_action(conn, action_identifier)?;
    let step = SavedStep::Action(saved.id);

    let original_len = routine.steps.len();
    routine.steps.retain(|s| s != &step);

    if routine.steps.len() == original_len {
        bail!(
            "'{}' is not a step in routine '{}'.",
            saved.title,
            routine.title
        );
    }

    database::insert_routine(conn, &routine)?;

    println!(
        "Removed '{}' from routine '{}'.",
        saved.title, routine.title
    );
    Ok(())
}

/// Resolves a routine by full UUID, UUID prefix, or title prefix.
pub fn resolve_routine(conn: &Connection, identifier: &str) -> Result<Routine> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        if let Some(routine) = database::fetch_routine_by_id(conn, uuid)? {
            return Ok(routine);
        }
        bail!("No routine found with id '{}'", identifier);
    }

    let routines = database::fetch_routines(conn)?;

    // Try UUID prefix match first
    let uuid_prefix_matches: Vec<&Routine> = routines
        .iter()
        .filter(|r| r.id.to_string().starts_with(identifier))
        .collect();

    if uuid_prefix_matches.len() == 1 {
        return Ok(uuid_prefix_matches[0].clone());
    }
    if uuid_prefix_matches.len() > 1 {
        bail!(
            "Multiple routines match UUID prefix '{}'. Use a longer prefix.",
            identifier
        );
    }

    // Fall back to title prefix match (case-insensitive)
    let title_matches: Vec<&Routine> = routines
        .iter()
        .filter(|r| {
            r.title
                .to_lowercase()
                .starts_with(&identifier.to_lowercase())
        })
        .collect();

    match title_matches.len() {
        0 => bail!("No routine found matching '{}'", identifier),
        1 => Ok(title_matches[0].clone()),
        _ => bail!(
            "Multiple routines match title prefix '{}'. Use a more specific identifier.",
            identifier
        ),
    }
}
