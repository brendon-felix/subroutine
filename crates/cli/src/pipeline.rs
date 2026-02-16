use anyhow::Result;
use clap::Subcommand;
use rusqlite::Connection;

use crate::resolve::{resolve_action, resolve_pipeline_item_in};

#[derive(Debug, Subcommand)]
pub enum PipelineCommand {
    /// List all items in the pipeline
    List {
        /// Show current scores for each item
        #[arg(short, long)]
        scored: bool,

        /// Pipeline ID (defaults to "default")
        #[arg(short, long, default_value = "default")]
        pipeline: String,
    },

    /// Get smart task suggestions based on current context (doesn't add to pipeline)
    Suggest {
        /// Number of suggestions to show
        #[arg(short, long, default_value = "3")]
        count: usize,
    },

    /// Re-score and re-order pipeline items based on current context
    Refresh {
        /// Pipeline ID (defaults to "default")
        #[arg(short, long, default_value = "default")]
        pipeline: String,
    },

    /// Show detailed scoring breakdown for a pipeline item
    Explain {
        /// Pipeline item ID or action title prefix
        identifier: String,

        /// Pipeline ID (defaults to "default")
        #[arg(short, long, default_value = "default")]
        pipeline: String,
    },

    /// Add an action instance to the pipeline
    Add {
        /// Action ID or title prefix to add
        action: String,

        /// Position to insert at (defaults to end)
        #[arg(short, long)]
        position: Option<i64>,

        /// Pipeline ID (defaults to "default")
        #[arg(long, default_value = "default")]
        pipeline: String,
    },

    /// Move a pipeline item to a new position
    Move {
        /// Pipeline item ID or action title prefix
        identifier: String,

        /// New position (1-indexed)
        #[arg(short, long)]
        position: i64,

        /// Pipeline ID (defaults to "default")
        #[arg(long, default_value = "default")]
        pipeline: String,
    },

    /// Remove an item from the pipeline (doesn't delete the instance)
    Remove {
        /// Pipeline item ID or action title prefix
        identifier: String,

        /// Pipeline ID (defaults to "default")
        #[arg(short, long, default_value = "default")]
        pipeline: String,
    },

    /// Normalize pipeline positions (fix gaps, make sequential starting from 1)
    Normalize {
        /// Pipeline ID (defaults to "default")
        #[arg(short, long, default_value = "default")]
        pipeline: String,
    },
}

pub fn handle_pipeline_command(command: &PipelineCommand, conn: &Connection) -> Result<()> {
    match command {
        PipelineCommand::List { scored, pipeline } => list_pipeline(conn, pipeline, *scored),
        PipelineCommand::Suggest { count } => suggest_tasks(conn, *count),
        PipelineCommand::Refresh { pipeline } => refresh_pipeline(conn, pipeline),
        PipelineCommand::Explain {
            identifier,
            pipeline,
        } => explain_pipeline_item(conn, pipeline, identifier),
        PipelineCommand::Add {
            action,
            position,
            pipeline,
        } => add_to_pipeline(conn, pipeline, action, *position),
        PipelineCommand::Move {
            identifier,
            position,
            pipeline,
        } => move_pipeline_item(conn, pipeline, identifier, *position),
        PipelineCommand::Remove {
            identifier,
            pipeline,
        } => remove_from_pipeline(conn, pipeline, identifier),
        PipelineCommand::Normalize { pipeline } => normalize_pipeline(conn, pipeline),
    }
}

fn list_pipeline(conn: &Connection, pipeline_id: &str, scored: bool) -> Result<()> {
    let items = database::fetch_pipeline_items(conn, pipeline_id)?;
    let instances = database::fetch_instances(conn)?;

    if items.is_empty() {
        println!("Pipeline '{}' is empty.", pipeline_id);
        println!("\nTry:");
        println!("  subroutine-cli pipeline suggest     # Get smart recommendations");
        println!("  subroutine-cli pipeline add <action> # Add an action manually");
        return Ok(());
    }

    println!("Pipeline: {}", pipeline_id);
    println!();

    if scored {
        // Fetch scores for all items
        let scored_items = database::score_pipeline_items(conn, pipeline_id)?;
        let score_map: std::collections::HashMap<_, _> = scored_items
            .into_iter()
            .map(|(item, score)| (item.id.clone(), score))
            .collect();

        for item in &items {
            let position = item.position.unwrap_or(0);
            let id_prefix = &item.id[..8.min(item.id.len())];
            let title = item.action_title.as_deref().unwrap_or("(no title)");
            let status = if let Some(instance_id) = &item.instance_id {
                if let Some(instance) = instances.iter().find(|i| &i.id == instance_id) {
                    instance.status.as_str()
                } else {
                    "unknown"
                }
            } else {
                "no instance"
            };

            let score = score_map.get(&item.id).copied().unwrap_or(0.0);

            println!(
                "{}. [{}] {} ({}) [Score: {:.2}]",
                position, id_prefix, title, status, score
            );
        }
    } else {
        for item in &items {
            let position = item.position.unwrap_or(0);
            let id_prefix = &item.id[..8.min(item.id.len())];
            let title = item.action_title.as_deref().unwrap_or("(no title)");
            let status = if let Some(instance_id) = &item.instance_id {
                if let Some(instance) = instances.iter().find(|i| &i.id == instance_id) {
                    instance.status.as_str()
                } else {
                    "unknown"
                }
            } else {
                "no instance"
            };

            println!("{}. [{}] {} ({})", position, id_prefix, title, status);
        }
    }

    Ok(())
}

fn suggest_tasks(conn: &Connection, count: usize) -> Result<()> {
    println!("🎯 Smart Task Suggestions\n");

    // Check if there's current context
    let context_info = if let Some(snapshot) = database::fetch_current_context(conn)? {
        let mut info = Vec::new();

        if let Some(metadata_str) = &snapshot.metadata {
            if let Ok(metadata) = serde_json::from_str::<serde_json::Value>(metadata_str) {
                if let Some(energy) = metadata.get("energy").and_then(|v| v.as_f64()) {
                    info.push(format!("Energy: {:.0}%", energy * 100.0));
                }
                if let Some(attention) = metadata.get("attention").and_then(|v| v.as_f64()) {
                    info.push(format!("Attention: {:.0}%", attention * 100.0));
                }
            }
        }

        if !info.is_empty() {
            format!("Based on current context ({})", info.join(", "))
        } else {
            "Based on available tasks".to_string()
        }
    } else {
        "Based on available tasks (no context set)".to_string()
    };

    println!("{}\n", context_info);

    let suggestions = database::suggest_best_instances(conn, count)?;

    if suggestions.is_empty() {
        println!("No suggestions available.");
        println!("\nTry:");
        println!("  subroutine-cli instances create <action> # Create some task instances");
        println!("  subroutine-cli context set-energy <0.0-1.0> # Set your energy level");
        println!("  subroutine-cli context set-attention <0.0-1.0> # Set your attention capacity");
        return Ok(());
    }

    for (i, (instance, action, score)) in suggestions.iter().enumerate() {
        let id_prefix = &instance.id[..8.min(instance.id.len())];
        println!("{}. [{}] {}", i + 1, id_prefix, action.title);
        println!("   Score: {:.2} | Status: {}", score, instance.status);

        // Show a brief hint about why this was suggested
        if let Some(duration) = action.duration_bucket {
            println!("   Duration: ~{} min", fibonacci_minutes(duration as i32));
        }
        if let Some(energy) = action.energy_rate {
            let energy_label = match energy {
                1 => "very low energy",
                2 => "low energy",
                3 => "moderate energy",
                4 => "high energy",
                5 => "very high energy",
                _ => "unknown energy",
            };
            println!("   Energy: {}", energy_label);
        }

        println!();
    }

    println!("To see detailed scoring breakdown:");
    println!("  subroutine-cli instances score <id>");

    Ok(())
}

fn refresh_pipeline(conn: &Connection, pipeline_id: &str) -> Result<()> {
    println!(
        "🔄 Refreshing pipeline '{}' based on current context...\n",
        pipeline_id
    );

    // Score all pipeline items
    let scored_items = database::score_pipeline_items(conn, pipeline_id)?;

    if scored_items.is_empty() {
        println!("Pipeline is empty - nothing to refresh.");
        return Ok(());
    }

    // Sort by score (highest first)
    let mut sorted_items = scored_items;
    sorted_items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Update positions based on score ranking
    for (new_position, (item, score)) in sorted_items.iter().enumerate() {
        let new_pos = (new_position + 1) as i64;
        database::update_pipeline_item_position(conn, &item.id, new_pos)?;

        let title = item.action_title.as_deref().unwrap_or("(no title)");
        let old_pos = item.position.unwrap_or(0);

        if old_pos != new_pos {
            println!(
                "  [{}] {} (score: {:.2}) moved: {} → {}",
                &item.id[..8],
                title,
                score,
                old_pos,
                new_pos
            );
        } else {
            println!(
                "  [{}] {} (score: {:.2}) stayed at position {}",
                &item.id[..8],
                title,
                score,
                new_pos
            );
        }
    }

    println!("\n✅ Pipeline refreshed and reordered by score!");
    println!("\nView updated pipeline:");
    println!("  subroutine-cli pipeline list --scored");

    Ok(())
}

fn explain_pipeline_item(conn: &Connection, pipeline_id: &str, identifier: &str) -> Result<()> {
    // Resolve the pipeline item
    let item = resolve_pipeline_item_in(conn, pipeline_id, identifier)?;

    println!(
        "Pipeline Item: {}\n",
        item.action_title.as_deref().unwrap_or("(no title)")
    );

    // Get the instance ID
    let instance_id = item
        .instance_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Pipeline item has no associated instance"))?;

    // Use the existing scoring function to get detailed breakdown
    let scored = database::score_instance_with_context(conn, instance_id)?;

    println!("Total Score: {:.2}\n", scored.total_score);
    println!("Factor Breakdown:");
    println!(
        "{:<20} {:>10} {:>10} {:>15}",
        "Factor", "Raw", "Weight", "Weighted"
    );
    println!("{}", "=".repeat(60));

    for factor in &scored.factor_scores {
        println!(
            "{:<20} {:>10.2} {:>10.2} {:>15.2}",
            factor.factor_name, factor.raw_score, factor.weight, factor.weighted_score
        );
    }

    println!("\nExplanations:");
    for factor in &scored.factor_scores {
        println!(
            "  • {}: {}",
            factor.factor_name,
            factor.explanation.as_deref().unwrap_or("No explanation")
        );
    }

    Ok(())
}

fn add_to_pipeline(
    conn: &Connection,
    pipeline_id: &str,
    action_identifier: &str,
    position: Option<i64>,
) -> Result<()> {
    // Resolve the action
    let action = resolve_action(conn, action_identifier)?;

    // Create an instance for this action
    let instance = database::Instance::new(&action.id);
    database::insert_instance(conn, &instance)?;

    // Determine position
    let pos = match position {
        Some(p) => p,
        None => database::next_pipeline_position(conn, pipeline_id)?,
    };

    // Create pipeline item
    let pipeline_item =
        database::PipelineItem::new_for_instance(pipeline_id, &instance.id, &action.title, pos);

    database::insert_pipeline_item(conn, &pipeline_item)?;

    println!(
        "✅ Added '{}' to pipeline at position {}",
        action.title, pos
    );
    println!("   Instance ID: {}", &instance.id[..8]);
    println!("   Pipeline item ID: {}", &pipeline_item.id[..8]);

    Ok(())
}

fn move_pipeline_item(
    conn: &Connection,
    pipeline_id: &str,
    identifier: &str,
    new_position: i64,
) -> Result<()> {
    // Resolve the pipeline item
    let item = resolve_pipeline_item_in(conn, pipeline_id, identifier)?;

    let old_position = item.position.unwrap_or(0);
    let title = item.action_title.as_deref().unwrap_or("(no title)");

    database::update_pipeline_item_position(conn, &item.id, new_position)?;

    println!(
        "✅ Moved '{}' from position {} to {}",
        title, old_position, new_position
    );

    Ok(())
}

fn remove_from_pipeline(conn: &Connection, pipeline_id: &str, identifier: &str) -> Result<()> {
    // Resolve the pipeline item
    let item = resolve_pipeline_item_in(conn, pipeline_id, identifier)?;

    let title = item.action_title.as_deref().unwrap_or("(no title)");

    database::delete_pipeline_item(conn, &item.id)?;

    println!("✅ Removed '{}' from pipeline", title);
    println!("   (The underlying instance was not deleted)");

    Ok(())
}

fn normalize_pipeline(conn: &Connection, pipeline_id: &str) -> Result<()> {
    database::normalize_pipeline_positions(conn, pipeline_id)?;

    println!("✅ Normalized positions in pipeline '{}'", pipeline_id);
    println!("   All positions are now sequential starting from 1");

    Ok(())
}

// Helper function to convert Fibonacci bucket index to approximate minutes
fn fibonacci_minutes(bucket: i32) -> i32 {
    match bucket {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 5,
        5 => 8,
        6 => 13,
        7 => 21,
        8 => 34,
        9 => 55,
        10 => 89,
        11 => 144,
        _ => bucket,
    }
}
