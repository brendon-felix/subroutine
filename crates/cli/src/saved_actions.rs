use anyhow::{Result, bail};
use app_core::{ActionContext, SavedAction, SavedConstraints};
use clap::Subcommand;
use rusqlite::Connection;
use uuid::Uuid;

use crate::actions::parse_time_of_day;

#[derive(Debug, Subcommand)]
pub enum SavedActionsCommand {
    /// List all saved action templates
    List,

    /// Show details for a saved action template (by ID prefix or title prefix)
    Show {
        /// UUID prefix or title prefix
        identifier: String,
    },

    /// Create a new saved action template
    Create {
        /// Title of the saved action
        title: String,

        /// Optional description or notes
        #[arg(short, long)]
        content: Option<String>,

        /// Importance level (1–5, where 5 is critical)
        #[arg(short, long)]
        importance: Option<u8>,

        /// Energy rate: how draining (-2) or energizing (+2) this action is
        #[arg(short, long, allow_hyphen_values = true)]
        energy: Option<i8>,

        /// Attention level required (1–5, where 5 is deep focus)
        #[arg(short, long)]
        attention: Option<u8>,

        /// Transition difficulty (1–5, where 5 is very hard to start/stop)
        #[arg(short, long)]
        transition: Option<u8>,

        /// Preferred time of day (HH:MM, 24-hour format, e.g. 08:00)
        #[arg(long)]
        target_time: Option<String>,

        /// Daily deadline as a time of day (HH:MM, 24-hour format, e.g. 17:00)
        #[arg(short, long)]
        deadline: Option<String>,

        /// Spoons required to do this action
        #[arg(short, long)]
        spoons: Option<u32>,

        /// Minimum interval between recurrences (e.g. 24h, 7d)
        #[arg(long)]
        recur_min: Option<String>,

        /// Maximum interval between recurrences before considered overdue
        #[arg(long)]
        recur_max: Option<String>,

        /// Automatically reschedule when the previous instance is completed
        #[arg(long)]
        auto_reschedule: bool,
    },

    /// Delete a saved action template (by ID prefix or title prefix)
    Delete {
        /// UUID prefix or title prefix
        identifier: String,
    },
}

pub fn handle_saved_actions_command(
    command: &SavedActionsCommand,
    conn: &Connection,
) -> Result<()> {
    match command {
        SavedActionsCommand::List => list_saved_actions(conn),
        SavedActionsCommand::Show { identifier } => show_saved_action(conn, identifier),
        SavedActionsCommand::Create {
            title,
            content,
            importance,
            energy,
            attention,
            transition,
            target_time,
            deadline,
            spoons,
            recur_min,
            recur_max,
            auto_reschedule,
        } => create_saved_action(
            conn,
            title,
            content.as_deref(),
            *importance,
            *energy,
            *attention,
            *transition,
            target_time.as_deref(),
            deadline.as_deref(),
            *spoons,
            recur_min.as_deref(),
            recur_max.as_deref(),
            *auto_reschedule,
        ),
        SavedActionsCommand::Delete { identifier } => delete_saved_action_cmd(conn, identifier),
    }
}

fn list_saved_actions(conn: &Connection) -> Result<()> {
    let saved_actions = database::fetch_saved_actions(conn)?;

    if saved_actions.is_empty() {
        println!("No saved action templates found.");
        return Ok(());
    }

    println!("Saved action templates ({}):", saved_actions.len());
    for saved in &saved_actions {
        let importance_marker = saved
            .context
            .importance
            .map(|i| format!(" [!{}]", i))
            .unwrap_or_default();
        let recurrence_marker = saved
            .recurrence
            .as_ref()
            .map(|r| {
                if r.auto_reschedule {
                    " [↻ auto]".to_string()
                } else {
                    " [↻]".to_string()
                }
            })
            .unwrap_or_default();
        println!(
            "  {} {}{}{}",
            &saved.id.to_string()[..8],
            saved.title,
            importance_marker,
            recurrence_marker
        );
    }

    Ok(())
}

fn show_saved_action(conn: &Connection, identifier: &str) -> Result<()> {
    let saved = resolve_saved_action(conn, identifier)?;

    println!("Saved Action Details:");
    println!("  ID:    {}", saved.id);
    println!("  Title: {}", saved.title);

    if let Some(ref content) = saved.content {
        println!("  Content: {}", content);
    }

    if let Some(target_time) = saved.target_time {
        println!("  Preferred time: {}", target_time.format("%H:%M"));
    }

    println!();
    println!("  Context:");
    println!(
        "    Importance:            {}",
        saved
            .context
            .importance
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
    );
    println!(
        "    Energy rate:           {}",
        saved
            .context
            .energy_rate
            .map(|v| format!("{:+}", v))
            .unwrap_or_else(|| "—".into())
    );
    println!(
        "    Attention level:       {}",
        saved
            .context
            .attention_level
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
    );
    println!(
        "    Transition difficulty: {}",
        saved
            .context
            .transition_difficulty
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
    );

    println!();
    println!("  Constraints:");
    println!(
        "    Deadline (time-of-day): {}",
        saved
            .constraints
            .deadline
            .map(|t| t.format("%H:%M").to_string())
            .unwrap_or_else(|| "—".into())
    );
    println!(
        "    Spoons required:        {}",
        saved
            .constraints
            .spoons_required
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
    );

    if let Some(ref recurrence) = saved.recurrence {
        println!();
        println!("  Recurrence:");
        println!(
            "    Min interval:    {}",
            recurrence
                .min_interval
                .map(|d| format_duration(d))
                .unwrap_or_else(|| "—".into())
        );
        println!(
            "    Max interval:    {}",
            recurrence
                .max_interval
                .map(|d| format_duration(d))
                .unwrap_or_else(|| "—".into())
        );
        println!(
            "    Auto-reschedule: {}",
            if recurrence.auto_reschedule {
                "yes"
            } else {
                "no"
            }
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_saved_action(
    conn: &Connection,
    title: &str,
    content: Option<&str>,
    importance: Option<u8>,
    energy: Option<i8>,
    attention: Option<u8>,
    transition: Option<u8>,
    target_time: Option<&str>,
    deadline: Option<&str>,
    spoons: Option<u32>,
    recur_min: Option<&str>,
    recur_max: Option<&str>,
    auto_reschedule: bool,
) -> Result<()> {
    if let Some(i) = importance {
        if !(1..=5).contains(&i) {
            bail!("Importance must be between 1 and 5, got {}", i);
        }
    }
    if let Some(e) = energy {
        if !(-2..=2).contains(&e) {
            bail!("Energy rate must be between -2 and +2, got {}", e);
        }
    }
    if let Some(a) = attention {
        if !(1..=5).contains(&a) {
            bail!("Attention level must be between 1 and 5, got {}", a);
        }
    }
    if let Some(t) = transition {
        if !(1..=5).contains(&t) {
            bail!("Transition difficulty must be between 1 and 5, got {}", t);
        }
    }

    let target_time = target_time.map(|s| parse_time_of_day(s)).transpose()?;

    let deadline = deadline.map(|s| parse_time_of_day(s)).transpose()?;

    let recur_min = recur_min
        .map(|s| {
            parse_duration(s).map_err(|e| anyhow::anyhow!("Invalid --recur-min '{}': {}", s, e))
        })
        .transpose()?;

    let recur_max = recur_max
        .map(|s| {
            parse_duration(s).map_err(|e| anyhow::anyhow!("Invalid --recur-max '{}': {}", s, e))
        })
        .transpose()?;

    let recurrence = if recur_min.is_some() || recur_max.is_some() || auto_reschedule {
        Some(app_core::RecurrenceRule {
            min_interval: recur_min,
            max_interval: recur_max,
            auto_reschedule,
        })
    } else {
        None
    };

    let mut saved = SavedAction::new(title);

    if let Some(c) = content {
        saved = saved.with_content(c);
    }

    saved.target_time = target_time;
    saved.context = ActionContext {
        importance,
        energy_rate: energy,
        attention_level: attention,
        transition_difficulty: transition,
    };
    saved.constraints = SavedConstraints {
        deadline,
        spoons_required: spoons,
        ..SavedConstraints::default()
    };
    saved.recurrence = recurrence;

    let id = saved.id;
    database::insert_saved_action(conn, &saved)?;
    println!(
        "Created saved action '{}' ({})",
        title,
        &id.to_string()[..8]
    );
    Ok(())
}

fn delete_saved_action_cmd(conn: &Connection, identifier: &str) -> Result<()> {
    let saved = resolve_saved_action(conn, identifier)?;
    database::delete_saved_action(conn, saved.id)?;
    println!(
        "Deleted saved action '{}' ({})",
        saved.title,
        &saved.id.to_string()[..8]
    );
    Ok(())
}

/// Resolves a saved action by full UUID, UUID prefix, or title prefix.
pub fn resolve_saved_action(conn: &Connection, identifier: &str) -> Result<SavedAction> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        if let Some(saved) = database::fetch_saved_action_by_id(conn, uuid)? {
            return Ok(saved);
        }
        bail!("No saved action found with id '{}'", identifier);
    }

    let saved_actions = database::fetch_saved_actions(conn)?;

    // Try UUID prefix match first
    let uuid_prefix_matches: Vec<&SavedAction> = saved_actions
        .iter()
        .filter(|s| s.id.to_string().starts_with(identifier))
        .collect();

    if uuid_prefix_matches.len() == 1 {
        return Ok(uuid_prefix_matches[0].clone());
    }
    if uuid_prefix_matches.len() > 1 {
        bail!(
            "Multiple saved actions match UUID prefix '{}'. Use a longer prefix.",
            identifier
        );
    }

    // Fall back to title prefix match (case-insensitive)
    let title_matches: Vec<&SavedAction> = saved_actions
        .iter()
        .filter(|s| {
            s.title
                .to_lowercase()
                .starts_with(&identifier.to_lowercase())
        })
        .collect();

    match title_matches.len() {
        0 => bail!("No saved action found matching '{}'", identifier),
        1 => Ok(title_matches[0].clone()),
        _ => bail!(
            "Multiple saved actions match title prefix '{}'. Use a more specific identifier.",
            identifier
        ),
    }
}

/// Parses a human-readable duration string like "24h", "7d", "90m", "3600s".
fn parse_duration(s: &str) -> Result<chrono::Duration> {
    let s = s.trim();
    if let Some(value) = s.strip_suffix('d') {
        let days: i64 = value
            .parse()
            .map_err(|_| anyhow::anyhow!("Expected a number before 'd'"))?;
        return Ok(chrono::Duration::days(days));
    }
    if let Some(value) = s.strip_suffix('h') {
        let hours: i64 = value
            .parse()
            .map_err(|_| anyhow::anyhow!("Expected a number before 'h'"))?;
        return Ok(chrono::Duration::hours(hours));
    }
    if let Some(value) = s.strip_suffix('m') {
        let minutes: i64 = value
            .parse()
            .map_err(|_| anyhow::anyhow!("Expected a number before 'm'"))?;
        return Ok(chrono::Duration::minutes(minutes));
    }
    if let Some(value) = s.strip_suffix('s') {
        let seconds: i64 = value
            .parse()
            .map_err(|_| anyhow::anyhow!("Expected a number before 's'"))?;
        return Ok(chrono::Duration::seconds(seconds));
    }
    bail!(
        "Unrecognized duration format '{}'. Use a number followed by d, h, m, or s (e.g. 24h, 7d)",
        s
    );
}

/// Formats a chrono Duration into a human-readable string.
fn format_duration(d: chrono::Duration) -> String {
    let total_seconds = d.num_seconds();
    if total_seconds % 86400 == 0 {
        format!("{}d", total_seconds / 86400)
    } else if total_seconds % 3600 == 0 {
        format!("{}h", total_seconds / 3600)
    } else if total_seconds % 60 == 0 {
        format!("{}m", total_seconds / 60)
    } else {
        format!("{}s", total_seconds)
    }
}
