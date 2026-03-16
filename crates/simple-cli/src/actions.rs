use anyhow::{Result, bail};
use clap::Subcommand;
use rusqlite::Connection;
use simple_core::Action;
use uuid::Uuid;

/// Returns the last segment of a UUIDv7 string (the 12 random hex chars after
/// the final hyphen). These bits are random regardless of when the ID was
/// created, so this stays unique even when many IDs are minted in the same
/// millisecond — unlike the first 8 chars which encode the timestamp.
pub fn short_id(id: Uuid) -> String {
    let s = id.to_string();
    s[24..].to_string()
}

/// Returns true if `identifier` matches `id` or `title`. Matching rules:
/// - Full UUID string: exact match on id
/// - Otherwise: prefix match on the last UUID segment (short_id) or
///   case-insensitive prefix match on title
pub fn id_matches(id: Uuid, title: &str, identifier: &str) -> bool {
    short_id(id).starts_with(identifier)
        || id.to_string().starts_with(identifier)
        || title.to_lowercase().starts_with(&identifier.to_lowercase())
}

#[derive(Debug, Subcommand)]
pub enum ActionsCommand {
    /// List all saved actions
    List,
    /// Add a new saved action
    Add {
        title: String,
        #[arg(short, long)]
        content: Option<String>,
    },
    /// Delete a saved action (by UUID prefix or title prefix)
    Delete { identifier: String },
}

pub fn handle_actions(command: &ActionsCommand, conn: &Connection) -> Result<()> {
    match command {
        ActionsCommand::List => {
            let actions = simple_db::fetch_actions(conn)?;
            if actions.is_empty() {
                println!("No saved actions.");
                return Ok(());
            }
            for action in &actions {
                println!("  {} {}", short_id(action.id), action.title);
            }
        }
        ActionsCommand::Add { title, content } => {
            let mut action = Action::new_saved(title);
            if let Some(c) = content {
                action = action.with_content(c);
            }
            let id = action.id;
            simple_db::upsert_action(conn, &action)?;
            println!("Added action '{}' ({})", title, short_id(id));
        }
        ActionsCommand::Delete { identifier } => {
            let action = resolve_action(conn, identifier)?;
            simple_db::delete_action(conn, action.id)?;
            println!(
                "Deleted action '{}' ({})",
                action.title,
                short_id(action.id)
            );
        }
    }
    Ok(())
}

pub fn resolve_action(conn: &Connection, identifier: &str) -> Result<Action> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        if let Some(action) = simple_db::fetch_action_by_id(conn, uuid)? {
            return Ok(action);
        }
        bail!("No action found with id '{}'", identifier);
    }

    let actions = simple_db::fetch_actions(conn)?;
    let matches: Vec<&Action> = actions
        .iter()
        .filter(|a| id_matches(a.id, &a.title, identifier))
        .collect();

    match matches.len() {
        0 => bail!("No action found matching '{}'", identifier),
        1 => Ok(matches[0].clone()),
        _ => bail!(
            "Multiple actions match '{}'. Use a more specific identifier.",
            identifier
        ),
    }
}
