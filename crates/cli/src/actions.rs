use anyhow::Result;
use clap::Subcommand;
use rusqlite::Connection;

use crate::resolve::resolve_action;

#[derive(Debug, Subcommand)]
pub enum ActionsCommand {
    /// List all actions
    List,

    /// Create a new action
    Create {
        /// Title of the action
        title: String,

        /// Type of action (e.g. "task", "habit")
        #[arg(short = 't', long, default_value = "task")]
        action_type: String,

        /// Description of the action
        #[arg(short, long)]
        description: Option<String>,

        /// Duration bucket in minutes (fibonacci: 1,2,3,5,8,13,21,34,55,89,144)
        #[arg(long)]
        duration: Option<i64>,

        /// Energy rate required (-5 to +5)
        #[arg(long, allow_hyphen_values = true)]
        energy: Option<i64>,

        /// Attention level required (0 to 5)
        #[arg(long)]
        attention: Option<i64>,

        /// Difficulty of transitioning into the action (0 to 5)
        #[arg(long)]
        transition_difficulty: Option<i64>,

        /// Enjoyment after starting the action (-5 to +5)
        #[arg(long, allow_hyphen_values = true)]
        enjoyment: Option<i64>,

        /// General importance of the action (0 to 5)
        #[arg(short, long)]
        importance: Option<i64>,

        /// Whether urgency grows over time
        #[arg(long)]
        urgency_growth: bool,
    },

    /// Show details for a specific action (by ID or unique title prefix)
    Show {
        /// Action ID or title prefix to look up
        identifier: String,
    },

    /// Delete an action by ID or unique title prefix
    Delete {
        /// Action ID or title prefix to delete
        identifier: String,
    },
}

pub fn handle_actions_command(command: &ActionsCommand, conn: &Connection) -> Result<()> {
    match command {
        ActionsCommand::List => list_actions(conn),
        ActionsCommand::Create {
            title,
            action_type,
            description,
            duration,
            energy,
            attention,
            transition_difficulty,
            enjoyment,
            importance,
            urgency_growth,
        } => {
            let mut action = database::Action::new(action_type.as_str(), title.as_str());

            if let Some(description) = description {
                action = action.description(description.as_str());
            }
            if let Some(duration) = duration {
                action = action.duration_bucket(*duration);
            }
            if let Some(energy) = energy {
                action = action.energy_rate(*energy);
            }
            if let Some(attention) = attention {
                action = action.attention_level(*attention);
            }
            if let Some(transition_difficulty) = transition_difficulty {
                action = action.transition_difficulty(*transition_difficulty);
            }
            if let Some(enjoyment) = enjoyment {
                action = action.enjoyment_after_start(*enjoyment);
            }
            if let Some(importance) = importance {
                action = action.importance(*importance);
            }
            if *urgency_growth {
                action = action.urgency_growth(true);
            }

            create_action(conn, action)
        }
        ActionsCommand::Show { identifier } => show_action(conn, identifier),
        ActionsCommand::Delete { identifier } => delete_action(conn, identifier),
    }
}

fn list_actions(conn: &Connection) -> Result<()> {
    let actions = database::fetch_actions(conn)?;

    if actions.is_empty() {
        println!("No actions found.");
        return Ok(());
    }

    println!("Actions ({}):", actions.len());
    for action in &actions {
        println!("  [{}] {}", &action.id[..8], action);
    }

    Ok(())
}

fn create_action(conn: &Connection, action: database::Action) -> Result<()> {
    let id = action.id.clone();
    let title = action.title.clone();
    database::insert_action(conn, &action)?;
    println!("Created action '{}' ({})", title, &id[..8]);
    Ok(())
}

fn show_action(conn: &Connection, identifier: &str) -> Result<()> {
    let action = resolve_action(conn, identifier)?;

    println!("Action Details:");
    println!("  ID:          {}", action.id);
    println!("  Type:        {}", action.action_type);
    println!("  Title:       {}", action.title);

    if let Some(ref description) = action.description {
        println!("  Description: {}", description);
    }
    if let Some(duration) = action.duration_bucket {
        println!("  Duration:    {} min", duration);
    }
    if let Some(energy) = action.energy_rate {
        println!("  Energy:      {}", energy);
    }
    if let Some(attention) = action.attention_level {
        println!("  Attention:   {}", attention);
    }
    if let Some(transition) = action.transition_difficulty {
        println!("  Transition:  {}", transition);
    }
    if let Some(enjoyment) = action.enjoyment_after_start {
        println!("  Enjoyment:   {}", enjoyment);
    }
    if let Some(importance) = action.importance {
        println!("  Importance:  {}", importance);
    }
    if let Some(urgency) = action.urgency_growth {
        println!(
            "  Urgency:     {}",
            if urgency { "grows" } else { "static" }
        );
    }
    if let Some(ref preferred_time) = action.preferred_time_of_day {
        println!("  Preferred:   {}", preferred_time);
    }
    if let Some(ref created_at) = action.created_at {
        println!("  Created:     {}", created_at);
    }
    if let Some(ref metadata) = action.metadata {
        println!("  Metadata:    {}", metadata);
    }

    Ok(())
}

fn delete_action(conn: &Connection, identifier: &str) -> Result<()> {
    let action = resolve_action(conn, identifier)?;
    database::delete_action(conn, &action.id)?;
    println!("Deleted action '{}' ({})", action.title, &action.id[..8]);
    Ok(())
}
