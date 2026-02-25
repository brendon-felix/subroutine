use anyhow::{Result, bail};
use app_core::{Action, ActionContext, Constraints, PipelineEntry, SavedAction, SavedConstraints};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use clap::Subcommand;
use rusqlite::Connection;
use uuid::Uuid;

#[derive(Debug, Subcommand)]
pub enum ActionsCommand {
    /// List all concrete actions currently in the pipeline
    List,

    /// Create a new action and add it to the pipeline backlog.
    /// By default a reusable saved template is also created. Use --ephemeral for one-off actions.
    Create {
        /// Title of the action
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

        /// Deadline as an RFC-3339 datetime (e.g. 2026-03-01T18:00:00Z)
        #[arg(short, long)]
        deadline: Option<String>,

        /// Spoons required to do this action
        #[arg(short, long)]
        spoons: Option<u32>,

        /// Create a one-off action with no reusable saved template
        #[arg(long)]
        ephemeral: bool,
    },

    /// Show details for a specific action (by ID prefix or title prefix)
    Show {
        /// Action UUID prefix or title prefix
        identifier: String,
    },

    /// Delete an action by ID prefix or title prefix
    Delete {
        /// Action UUID prefix or title prefix
        identifier: String,
    },
}

pub fn handle_actions_command(command: &ActionsCommand, conn: &Connection) -> Result<()> {
    match command {
        ActionsCommand::List => list_actions(conn),
        ActionsCommand::Create {
            title,
            content,
            importance,
            energy,
            attention,
            transition,
            deadline,
            spoons,
            ephemeral,
        } => create_action(
            conn,
            title,
            content.as_deref(),
            *importance,
            *energy,
            *attention,
            *transition,
            deadline.as_deref(),
            *spoons,
            *ephemeral,
        ),
        ActionsCommand::Show { identifier } => show_action(conn, identifier),
        ActionsCommand::Delete { identifier } => delete_action(conn, identifier),
    }
}

fn list_actions(conn: &Connection) -> Result<()> {
    let actions = database::fetch_actions(conn)?;

    if actions.is_empty() {
        println!("No actions found in the pipeline.");
        return Ok(());
    }

    println!("Actions ({}):", actions.len());
    for action in &actions {
        let importance_marker = action
            .context
            .importance
            .map(|i| format!(" [!{}]", i))
            .unwrap_or_default();
        let deadline_marker = action
            .constraints
            .deadline
            .map(|d| format!(" [due {}]", d.format("%Y-%m-%d")))
            .unwrap_or_default();
        let ephemeral_marker = if action.ephemeral { " [ephemeral]" } else { "" };
        println!(
            "  {} {}{}{}{}",
            &action.id.to_string()[..8],
            action.title,
            importance_marker,
            deadline_marker,
            ephemeral_marker,
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_action(
    conn: &Connection,
    title: &str,
    content: Option<&str>,
    importance: Option<u8>,
    energy: Option<i8>,
    attention: Option<u8>,
    transition: Option<u8>,
    deadline: Option<&str>,
    spoons: Option<u32>,
    ephemeral: bool,
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

    let deadline_dt = deadline.map(|s| parse_datetime(s)).transpose()?;

    let context = ActionContext {
        importance,
        energy_rate: energy,
        attention_level: attention,
        transition_difficulty: transition,
    };

    let action = if ephemeral {
        // One-off action: create the concrete Action directly, no saved template.
        let mut action = Action::new(title);
        if let Some(c) = content {
            action = action.with_content(c);
        }
        action.context = context;
        action.constraints = Constraints {
            deadline: deadline_dt,
            spoons_required: spoons,
            ..Constraints::default()
        };
        action = action.ephemeral(true);
        database::insert_action(conn, &action)?;
        action
    } else {
        // Non-ephemeral: create a SavedAction template, then instantiate a concrete Action from it.
        let mut saved = SavedAction::new(title);
        if let Some(c) = content {
            saved = saved.with_content(c);
        }
        saved.context = context;
        // SavedConstraints uses NaiveTime for deadlines; we store the absolute deadline
        // directly on the concrete action after instantiation instead.
        saved.constraints = SavedConstraints {
            spoons_required: spoons,
            ..SavedConstraints::default()
        };

        database::insert_saved_action(conn, &saved)?;

        let mut action = saved.instantiate();
        // Apply the absolute deadline the user provided, overriding any materialized value.
        action.constraints.deadline = deadline_dt;
        database::insert_action(conn, &action)?;
        action
    };

    let id = action.id;

    // Add the new concrete action to the pipeline backlog.
    let mut pipeline = database::load_pipeline(conn)?;
    pipeline.push(PipelineEntry::Action(action))?;
    database::save_pipeline(conn, &pipeline)?;

    println!(
        "Created action '{}' ({}) and added to pipeline backlog.",
        title,
        &id.to_string()[..8]
    );
    Ok(())
}

fn show_action(conn: &Connection, identifier: &str) -> Result<()> {
    let action = resolve_action(conn, identifier)?;

    println!("Action Details:");
    println!("  ID:      {}", action.id);
    println!("  Title:   {}", action.title);
    println!(
        "  Created: {}",
        action.created_at.format("%Y-%m-%d %H:%M UTC")
    );
    if action.ephemeral {
        println!("  Type:    ephemeral (no saved template)");
    } else if let Some(id) = action.saved_action_id {
        println!("  Template: {}", &id.to_string()[..8]);
    }

    if let Some(ref c) = action.content {
        println!("  Content: {}", c);
    }

    println!();
    println!("  Context:");
    println!(
        "    Importance:            {}",
        action
            .context
            .importance
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
    );
    println!(
        "    Energy rate:           {}",
        action
            .context
            .energy_rate
            .map(|v| format!("{:+}", v))
            .unwrap_or_else(|| "—".into())
    );
    println!(
        "    Attention level:       {}",
        action
            .context
            .attention_level
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
    );
    println!(
        "    Transition difficulty: {}",
        action
            .context
            .transition_difficulty
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
    );

    println!();
    println!("  Constraints:");
    println!(
        "    Deadline:        {}",
        action
            .constraints
            .deadline
            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "—".into())
    );
    println!(
        "    Earliest start:  {}",
        action
            .constraints
            .earliest_start
            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "—".into())
    );
    println!(
        "    Spoons required: {}",
        action
            .constraints
            .spoons_required
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".into())
    );
    if !action.constraints.dependencies.is_empty() {
        println!("    Dependencies:");
        for dep in &action.constraints.dependencies {
            println!("      {}", dep);
        }
    }

    Ok(())
}

fn delete_action(conn: &Connection, identifier: &str) -> Result<()> {
    let action = resolve_action(conn, identifier)?;
    database::delete_action(conn, action.id)?;
    println!(
        "Deleted action '{}' ({})",
        action.title,
        &action.id.to_string()[..8]
    );
    Ok(())
}

/// Parses a flexible datetime string into a `DateTime<Utc>`. Tries these formats in order:
///
/// - RFC-3339 with timezone: `2026-03-01T18:00:00Z`, `2026-03-01T18:00:00+05:00`
/// - Date + time (no timezone, treated as UTC): `2026-03-01 18:00`, `2026-03-01 18:00:00`
/// - Date only (midnight UTC): `2026-03-01`
/// - Time today (UTC): `18:00`, `18:00:00`, `6pm`, `6:30pm`, `6am`, `14`
pub fn parse_datetime(s: &str) -> Result<DateTime<Utc>> {
    let s = s.trim();

    // RFC-3339 / ISO-8601 with offset
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    // Date + time, no timezone (treat as UTC)
    for fmt in &[
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(Utc.from_utc_datetime(&ndt));
        }
    }

    // Date only — midnight UTC
    if let Ok(nd) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let ndt = nd.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    // Time today (UTC)
    if let Ok(time) = parse_time_of_day(s) {
        let today = Utc::now().date_naive();
        let ndt = today.and_time(time);
        return Ok(Utc.from_utc_datetime(&ndt));
    }

    bail!(
        "Unrecognized datetime '{}'. Accepted formats: \
        '2026-03-01T18:00Z', '2026-03-01 18:00', '2026-03-01', '18:00', '6pm', '6:30am'",
        s
    )
}

/// Parses a flexible time-of-day string into a `NaiveTime`. Tries these formats:
///
/// - 24-hour: `18:00`, `08:00`, `18:00:00`, `8:00`
/// - Bare hour (24-hour): `14`, `8`
/// - 12-hour with am/pm: `6pm`, `6am`, `6:30pm`, `06:30am`
pub fn parse_time_of_day(s: &str) -> Result<NaiveTime> {
    let s = s.trim();

    // 24-hour HH:MM:SS and HH:MM
    for fmt in &["%H:%M:%S", "%H:%M"] {
        if let Ok(t) = NaiveTime::parse_from_str(s, fmt) {
            return Ok(t);
        }
    }

    // 12-hour with am/pm: manual parsing is more reliable than chrono's %p specifier.
    // Handles: "6pm", "6am", "6:30pm", "6:30 pm", "12pm", "12am".
    let lower = s.to_lowercase();
    let lower = lower.trim();
    if lower.ends_with("am") || lower.ends_with("pm") {
        let is_pm = lower.ends_with("pm");
        let time_part = if is_pm {
            lower.trim_end_matches("pm")
        } else {
            lower.trim_end_matches("am")
        }
        .trim();
        let (hour, minute) = if let Some((h, m)) = time_part.split_once(':') {
            let hour: u32 = h
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid hour in '{}'", s))?;
            let minute: u32 = m
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid minute in '{}'", s))?;
            (hour, minute)
        } else {
            let hour: u32 = time_part
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid hour in '{}'", s))?;
            (hour, 0)
        };
        let hour_24 = match (is_pm, hour) {
            (false, 12) => 0,    // 12am → midnight
            (false, h) => h,     // 1am–11am → 1–11
            (true, 12) => 12,    // 12pm → noon
            (true, h) => h + 12, // 1pm–11pm → 13–23
        };
        if let Some(t) = NaiveTime::from_hms_opt(hour_24, minute, 0) {
            return Ok(t);
        }
    }

    // Bare integer hour (0–23)
    if let Ok(hour) = s.parse::<u32>() {
        if let Some(t) = NaiveTime::from_hms_opt(hour, 0, 0) {
            return Ok(t);
        }
    }

    bail!(
        "Unrecognized time '{}'. Accepted formats: '18:00', '6pm', '6:30am', '14'",
        s
    )
}

/// Resolves a concrete action by full UUID, UUID prefix, or title prefix.
pub fn resolve_action(conn: &Connection, identifier: &str) -> Result<Action> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        if let Some(action) = database::fetch_action_by_id(conn, uuid)? {
            return Ok(action);
        }
        bail!("No action found with id '{}'", identifier);
    }

    let actions = database::fetch_actions(conn)?;

    // Try UUID prefix match first (identifier looks like a short hex string)
    let uuid_prefix_matches: Vec<&Action> = actions
        .iter()
        .filter(|a| a.id.to_string().starts_with(identifier))
        .collect();

    if uuid_prefix_matches.len() == 1 {
        return Ok(uuid_prefix_matches[0].clone());
    }
    if uuid_prefix_matches.len() > 1 {
        bail!(
            "Multiple actions match UUID prefix '{}'. Use a longer prefix.",
            identifier
        );
    }

    // Fall back to title prefix match
    let title_matches: Vec<&Action> = actions
        .iter()
        .filter(|a| {
            a.title
                .to_lowercase()
                .starts_with(&identifier.to_lowercase())
        })
        .collect();

    match title_matches.len() {
        0 => bail!("No action found matching '{}'", identifier),
        1 => Ok(title_matches[0].clone()),
        _ => bail!(
            "Multiple actions match title prefix '{}'. Use a more specific identifier.",
            identifier
        ),
    }
}
