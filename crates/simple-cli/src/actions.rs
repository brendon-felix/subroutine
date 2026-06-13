use anyhow::{Result, bail};
use clap::Subcommand;
use reqwest::blocking::Client;
use serde::Deserialize;
use simple_core::{Action, Event, Routine};
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

#[derive(Debug, Deserialize)]
pub struct ApiData {
    pub actions: Vec<Action>,
    pub events: Vec<Event>,
    pub routines: Vec<Routine>,
}

pub fn fetch_api_data(client: &Client, base: &str) -> Result<ApiData> {
    let data = client
        .get(format!("{}/api/data", base))
        .send()?
        .error_for_status()?
        .json::<ApiData>()?;
    Ok(data)
}

pub fn fetch_all_actions(client: &Client, base: &str) -> Result<Vec<Action>> {
    Ok(fetch_api_data(client, base)?.actions)
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

pub fn handle_actions(command: &ActionsCommand, client: &Client, base: &str) -> Result<()> {
    match command {
        ActionsCommand::List => {
            let actions = fetch_all_actions(client, base)?;
            if actions.is_empty() {
                println!("No saved actions.");
                return Ok(());
            }
            for action in &actions {
                println!("  {} {}", short_id(action.id), action.title);
            }
        }
        ActionsCommand::Add { title, content } => {
            let mut action = Action::new(title);
            if let Some(c) = content {
                action = action.with_content(c);
            }
            let id = action.id;
            client
                .put(format!("{}/api/actions/{}", base, id))
                .json(&action)
                .send()?
                .error_for_status()?;
            println!("Added action '{}' ({})", title, short_id(id));
        }
        ActionsCommand::Delete { identifier } => {
            let action = resolve_action(client, base, identifier)?;
            client
                .delete(format!("{}/api/actions/{}", base, action.id))
                .send()?
                .error_for_status()?;
            println!(
                "Deleted action '{}' ({})",
                action.title,
                short_id(action.id)
            );
        }
    }
    Ok(())
}

pub fn resolve_action(client: &Client, base: &str, identifier: &str) -> Result<Action> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        let actions = fetch_all_actions(client, base)?;
        if let Some(action) = actions.into_iter().find(|a| a.id == uuid) {
            return Ok(action);
        }
        bail!("No action found with id '{}'", identifier);
    }

    let actions = fetch_all_actions(client, base)?;
    let matches: Vec<Action> = actions
        .into_iter()
        .filter(|a| id_matches(a.id, &a.title, identifier))
        .collect();

    match matches.len() {
        0 => bail!("No action found matching '{}'", identifier),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => bail!(
            "Multiple actions match '{}'. Use a more specific identifier.",
            identifier
        ),
    }
}
