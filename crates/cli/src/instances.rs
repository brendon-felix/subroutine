use anyhow::Result;
use clap::Subcommand;
use rusqlite::Connection;

use crate::resolve::{resolve_action, resolve_instance};

#[derive(Debug, Subcommand)]
pub enum InstancesCommand {
    /// List all instances
    List {
        /// Filter by status (e.g. "pending", "active", "completed", "scheduled")
        #[arg(short, long)]
        status: Option<String>,
    },

    /// Create a new instance from an action
    Create {
        /// Action ID or title prefix to create instance from
        action: String,

        /// Status for the new instance
        #[arg(short, long, default_value = "scheduled")]
        status: String,

        /// Also add the instance to the default pipeline
        #[arg(short, long)]
        enqueue: bool,
    },

    /// Show details of a specific instance
    Show {
        /// Instance ID or prefix to look up
        identifier: String,
    },

    /// Update the status of an instance
    Status {
        /// Instance ID or prefix
        identifier: String,

        /// New status (e.g. "pending", "active", "completed", "paused")
        status: String,
    },

    /// Delete an instance
    Delete {
        /// Instance ID or prefix to delete
        identifier: String,
    },

    /// Score an instance and show the breakdown
    Score {
        /// Instance ID or prefix to score
        identifier: String,
    },
}

pub fn handle_instances_command(command: &InstancesCommand, conn: &Connection) -> Result<()> {
    match command {
        InstancesCommand::List { status } => list_instances(conn, status.as_deref()),
        InstancesCommand::Create {
            action,
            status,
            enqueue,
        } => create_instance(conn, action, status, *enqueue),
        InstancesCommand::Show { identifier } => show_instance(conn, identifier),
        InstancesCommand::Status { identifier, status } => {
            update_instance_status(conn, identifier, status)
        }
        InstancesCommand::Delete { identifier } => delete_instance(conn, identifier),
        InstancesCommand::Score { identifier } => score_instance(conn, identifier),
    }
}

fn list_instances(conn: &Connection, status_filter: Option<&str>) -> Result<()> {
    let mut instances = database::fetch_instances(conn)?;

    // Filter by status if requested
    if let Some(status) = status_filter {
        instances.retain(|instance| {
            instance
                .status
                .to_lowercase()
                .contains(&status.to_lowercase())
        });
    }

    if instances.is_empty() {
        if let Some(status) = status_filter {
            println!("No instances found with status '{}'.", status);
        } else {
            println!("No instances found.");
        }
        return Ok(());
    }

    let actions = database::fetch_actions(conn)?;
    let action_map: std::collections::HashMap<_, _> = actions
        .iter()
        .map(|action| (action.id.as_str(), action))
        .collect();

    if let Some(status) = status_filter {
        println!("Instances with status '{}' ({}):", status, instances.len());
    } else {
        println!("Instances ({}):", instances.len());
    }

    for instance in &instances {
        let action_title = action_map
            .get(instance.action_id.as_str())
            .map(|a| a.title.as_str())
            .unwrap_or("<unknown>");

        print!("  [{}] ", &instance.id[..8]);
        print!("[{}] ", instance.status);
        print!("{}", action_title);

        if let Some(ref source) = instance.source {
            print!(" ({})", source);
        }

        if let Some(ref scheduled_start) = instance.scheduled_start {
            print!(" @ {}", scheduled_start);
        }

        println!();
    }

    Ok(())
}

fn create_instance(
    conn: &Connection,
    action_identifier: &str,
    status: &str,
    enqueue: bool,
) -> Result<()> {
    let action = resolve_action(conn, action_identifier)?;

    if enqueue {
        let (instance, _pipeline_item) =
            database::create_instance_and_enqueue(conn, &action, status)?;
        println!(
            "Created instance '{}' ({}) with status '{}' and added to pipeline",
            action.title,
            &instance.id[..8],
            status
        );
    } else {
        let mut instance = database::Instance::new(&action.id);
        instance.status = status.to_string();
        database::insert_instance(conn, &instance)?;
        println!(
            "Created instance '{}' ({}) with status '{}'",
            action.title,
            &instance.id[..8],
            status
        );
    }

    Ok(())
}

fn show_instance(conn: &Connection, identifier: &str) -> Result<()> {
    let instance = resolve_instance(conn, identifier)?;

    // Fetch the associated action to display its title
    let actions = database::fetch_actions(conn)?;
    let action = actions
        .iter()
        .find(|a| a.id == instance.action_id)
        .ok_or_else(|| anyhow::anyhow!("Action '{}' not found", instance.action_id))?;

    println!("Instance Details:");
    println!("  ID:          {}", instance.id);
    println!("  Action:      {} ({})", action.title, &action.id[..8]);
    println!("  Status:      {}", instance.status);

    if let Some(ref source) = instance.source {
        println!("  Source:      {}", source);
    }
    if let Some(ref scheduled_start) = instance.scheduled_start {
        println!("  Start:       {}", scheduled_start);
    }
    if let Some(ref scheduled_end) = instance.scheduled_end {
        println!("  End:         {}", scheduled_end);
    }
    if let Some(ref earliest_start) = instance.earliest_start {
        println!("  Earliest:    {}", earliest_start);
    }
    if let Some(ref latest_end) = instance.latest_end {
        println!("  Latest:      {}", latest_end);
    }
    if let Some(ref created_at) = instance.created_at {
        println!("  Created:     {}", created_at);
    }
    if let Some(ref metadata) = instance.metadata {
        println!("  Metadata:    {}", metadata);
    }

    Ok(())
}

fn update_instance_status(conn: &Connection, identifier: &str, status: &str) -> Result<()> {
    let instance = resolve_instance(conn, identifier)?;

    database::set_instance_status(conn, &instance.id, status)?;

    println!(
        "Updated instance ({}) status to '{}'",
        &instance.id[..8],
        status
    );

    Ok(())
}

fn delete_instance(conn: &Connection, identifier: &str) -> Result<()> {
    let instance = resolve_instance(conn, identifier)?;

    // Fetch the associated action to display its title
    let actions = database::fetch_actions(conn)?;
    let action_title = actions
        .iter()
        .find(|a| a.id == instance.action_id)
        .map(|a| a.title.as_str())
        .unwrap_or("<unknown>");

    database::delete_instance(conn, &instance.id)?;

    println!(
        "Deleted instance of '{}' ({})",
        action_title,
        &instance.id[..8]
    );

    Ok(())
}

fn score_instance(conn: &Connection, identifier: &str) -> Result<()> {
    let instance = resolve_instance(conn, identifier)?;

    // Fetch the associated action to display its title
    let actions = database::fetch_actions(conn)?;
    let action = actions
        .iter()
        .find(|a| a.id == instance.action_id)
        .ok_or_else(|| anyhow::anyhow!("Action '{}' not found", instance.action_id))?;

    // Score the instance using current context
    let scored = database::score_instance_with_context(conn, &instance.id)?;

    println!("Scoring for: {} ({})", action.title, &instance.id[..8]);
    println!("Status: {}", instance.status);
    println!();
    println!("Total Score: {:.2}", scored.total_score);
    println!();
    println!("Factor Breakdown:");

    for factor in &scored.factor_scores {
        println!(
            "  {:16} │ raw: {:5.2} │ weight: {:4.2} │ weighted: {:5.2}",
            factor.factor_name, factor.raw_score, factor.weight, factor.weighted_score
        );
        if let Some(ref explanation) = factor.explanation {
            println!("                   └─ {}", explanation);
        }
    }

    Ok(())
}
