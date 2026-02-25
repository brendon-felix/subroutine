use anyhow::{Result, bail};
use app_core::SavedMentalState;
use clap::Subcommand;
use rusqlite::Connection;
use uuid::Uuid;

#[derive(Debug, Subcommand)]
pub enum MentalStatesCommand {
    /// List all saved mental states
    List,

    /// Show details for a saved mental state (by ID prefix or name prefix)
    Show {
        /// UUID prefix or name prefix
        identifier: String,
    },

    /// Create a new saved mental state
    Create {
        /// Name for this mental state
        name: String,

        /// Optional description
        #[arg(short, long)]
        description: Option<String>,

        /// Attention mode: scattered (-2) to hyperfocused (+2)
        #[arg(long, default_value = "0", allow_hyphen_values = true)]
        attention: i8,

        /// Sensory tolerance: understimulated (-2) to overstimulated (+2)
        #[arg(long, default_value = "0", allow_hyphen_values = true)]
        sensory: i8,

        /// Emotional regulation: dysregulated (-2) to regulated (+2)
        #[arg(long, default_value = "0", allow_hyphen_values = true)]
        regulation: i8,

        /// Social battery: drained (-2) to charged (+2)
        #[arg(long, default_value = "0", allow_hyphen_values = true)]
        social: i8,
    },

    /// Delete a saved mental state (by ID prefix or name prefix)
    Delete {
        /// UUID prefix or name prefix
        identifier: String,
    },
}

pub fn handle_mental_states_command(
    command: &MentalStatesCommand,
    conn: &Connection,
) -> Result<()> {
    match command {
        MentalStatesCommand::List => list_mental_states(conn),
        MentalStatesCommand::Show { identifier } => show_mental_state(conn, identifier),
        MentalStatesCommand::Create {
            name,
            description,
            attention,
            sensory,
            regulation,
            social,
        } => create_mental_state(
            conn,
            name,
            description.as_deref(),
            *attention,
            *sensory,
            *regulation,
            *social,
        ),
        MentalStatesCommand::Delete { identifier } => delete_mental_state(conn, identifier),
    }
}

fn list_mental_states(conn: &Connection) -> Result<()> {
    let states = database::fetch_saved_mental_states(conn)?;

    if states.is_empty() {
        println!("No saved mental states found.");
        return Ok(());
    }

    println!("Saved mental states ({}):", states.len());
    println!(
        "  {:<8}  {:<16}  {:>4}  {:>4}  {:>4}  {:>4}  {}",
        "ID", "Name", "Attn", "Sens", "Reg", "Soc", "Description"
    );
    println!("  {}", "-".repeat(72));
    for state in &states {
        let description = state.description.as_deref().unwrap_or("—");
        println!(
            "  {:<8}  {:<16}  {:>4}  {:>4}  {:>4}  {:>4}  {}",
            &state.id.to_string()[..8],
            state.name,
            format!("{:+}", state.attention_mode),
            format!("{:+}", state.sensory_tolerance),
            format!("{:+}", state.emotional_regulation),
            format!("{:+}", state.social_battery),
            description,
        );
    }

    Ok(())
}

fn show_mental_state(conn: &Connection, identifier: &str) -> Result<()> {
    let state = resolve_mental_state(conn, identifier)?;

    println!("Mental State Details:");
    println!("  ID:          {}", state.id);
    println!("  Name:        {}", state.name);
    if let Some(ref desc) = state.description {
        println!("  Description: {}", desc);
    }
    println!();
    println!("  Axes:");
    println!(
        "    Attention mode:       {:>3}  (scattered -2 ↔ +2 hyperfocused)",
        format!("{:+}", state.attention_mode)
    );
    println!(
        "    Sensory tolerance:    {:>3}  (understimulated -2 ↔ +2 overstimulated)",
        format!("{:+}", state.sensory_tolerance)
    );
    println!(
        "    Emotional regulation: {:>3}  (dysregulated -2 ↔ +2 regulated)",
        format!("{:+}", state.emotional_regulation)
    );
    println!(
        "    Social battery:       {:>3}  (drained -2 ↔ +2 charged)",
        format!("{:+}", state.social_battery)
    );

    Ok(())
}

fn create_mental_state(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    attention: i8,
    sensory: i8,
    regulation: i8,
    social: i8,
) -> Result<()> {
    for (label, value) in [
        ("attention", attention),
        ("sensory", sensory),
        ("regulation", regulation),
        ("social", social),
    ] {
        if !(-2..=2).contains(&value) {
            bail!("'{}' must be between -2 and +2, got {}", label, value);
        }
    }

    let mut state = SavedMentalState::new(name).with_axes(attention, sensory, regulation, social);
    if let Some(desc) = description {
        state = state.with_description(desc);
    }

    let id = state.id;
    database::insert_saved_mental_state(conn, &state)?;
    println!("Created mental state '{}' ({})", name, &id.to_string()[..8]);
    Ok(())
}

fn delete_mental_state(conn: &Connection, identifier: &str) -> Result<()> {
    let state = resolve_mental_state(conn, identifier)?;
    database::delete_saved_mental_state(conn, state.id)?;
    println!(
        "Deleted mental state '{}' ({})",
        state.name,
        &state.id.to_string()[..8]
    );
    Ok(())
}

/// Resolves a saved mental state by full UUID, UUID prefix, or name prefix.
pub fn resolve_mental_state(conn: &Connection, identifier: &str) -> Result<SavedMentalState> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        if let Some(state) = database::fetch_saved_mental_state_by_id(conn, uuid)? {
            return Ok(state);
        }
        bail!("No mental state found with id '{}'", identifier);
    }

    let states = database::fetch_saved_mental_states(conn)?;

    // Try UUID prefix match
    let uuid_prefix_matches: Vec<&SavedMentalState> = states
        .iter()
        .filter(|s| s.id.to_string().starts_with(identifier))
        .collect();

    if uuid_prefix_matches.len() == 1 {
        return Ok(uuid_prefix_matches[0].clone());
    }
    if uuid_prefix_matches.len() > 1 {
        bail!(
            "Multiple mental states match UUID prefix '{}'. Use a longer prefix.",
            identifier
        );
    }

    // Fall back to name prefix match (case-insensitive)
    let name_matches: Vec<&SavedMentalState> = states
        .iter()
        .filter(|s| {
            s.name
                .to_lowercase()
                .starts_with(&identifier.to_lowercase())
        })
        .collect();

    match name_matches.len() {
        0 => bail!("No mental state found matching '{}'", identifier),
        1 => Ok(name_matches[0].clone()),
        _ => bail!(
            "Multiple mental states match name prefix '{}'. Use a more specific identifier.",
            identifier
        ),
    }
}
