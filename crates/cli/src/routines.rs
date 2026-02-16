use anyhow::Result;
use clap::Subcommand;
use database::{
    InstantiateRoutineOptions, Routine, RoutineStep, count_routine_steps, delete_routine,
    delete_routine_step_by_order, fetch_routine_steps, fetch_routines, insert_routine,
    insert_routine_step, instantiate_routine, next_routine_step_order, shift_routine_steps,
};

use crate::resolve::{Resolvable, resolve_action};

#[derive(Debug, Clone, Subcommand)]
pub enum RoutinesCommand {
    /// List all routines
    List,

    /// Create a new routine
    Create {
        /// Name of the routine
        name: String,

        /// Optional description
        #[arg(long, short)]
        description: Option<String>,

        /// Create a non-sequential (parallel) routine (default is sequential)
        #[arg(long)]
        parallel: bool,

        /// Allow randomization of steps
        #[arg(long)]
        randomize: bool,

        /// Default start time (HH:MM format)
        #[arg(long)]
        start_time: Option<String>,

        /// Default end time (HH:MM format)
        #[arg(long)]
        end_time: Option<String>,
    },

    /// Show routine details including its steps
    Show {
        /// Routine identifier (ID, prefix, or name)
        identifier: String,
    },

    /// Delete a routine and its steps
    Delete {
        /// Routine identifier (ID, prefix, or name)
        identifier: String,
    },

    /// Add an action as a step in a routine
    AddStep {
        /// Routine identifier (ID, prefix, or name)
        routine: String,

        /// Action identifier (ID, prefix, or title)
        action: String,

        /// Position to insert at (shifts existing steps)
        #[arg(long, short)]
        position: Option<i64>,

        /// Minimum duration in minutes (Fibonacci bucket)
        #[arg(long)]
        min_duration: Option<i64>,

        /// Maximum duration in minutes (Fibonacci bucket)
        #[arg(long)]
        max_duration: Option<i64>,
    },

    /// Remove a step from a routine by its order number
    RemoveStep {
        /// Routine identifier (ID, prefix, or name)
        routine: String,

        /// Step order number to remove
        step_order: i64,
    },

    /// Start a routine by creating instances for all steps and adding them to the pipeline
    Start {
        /// Routine identifier (ID, prefix, or name)
        identifier: String,

        /// Randomize step order (overrides routine setting)
        #[arg(long, short)]
        randomize: bool,

        /// Use sequential order even if routine allows randomization
        #[arg(long, short)]
        sequential: bool,

        /// Starting position in the pipeline (defaults to end)
        #[arg(long, short)]
        position: Option<i64>,
    },
}

// Implement Resolvable for Routine
impl Resolvable for Routine {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn fetch_all(conn: &rusqlite::Connection) -> Result<Vec<Self>> {
        fetch_routines(conn)
    }
}

/// Convenience function for resolving routines
pub fn resolve_routine(conn: &rusqlite::Connection, identifier: &str) -> Result<Routine> {
    Routine::resolve(conn, identifier)
}

pub fn handle_routines_command(cmd: &RoutinesCommand, conn: &rusqlite::Connection) -> Result<()> {
    match cmd {
        RoutinesCommand::Start {
            identifier,
            randomize,
            sequential,
            position,
        } => {
            let routine = resolve_routine(conn, identifier)?;

            let randomize_option = if *randomize {
                Some(true)
            } else if *sequential {
                Some(false)
            } else {
                None
            };

            let options = InstantiateRoutineOptions {
                randomize: randomize_option,
                start_position: *position,
                pipeline_id: None,
            };

            let result = instantiate_routine(conn, &routine, options)?;

            if result.created_items.is_empty() {
                println!(
                    "\n⚠️  Routine '{}' has no steps to instantiate.",
                    routine.name
                );
                println!(
                    "Add steps with: subroutine-cli routines add-step \"{}\" <ACTION>",
                    routine.name
                );
                return Ok(());
            }

            println!("\n🚀 Started routine: {}", routine.name);
            if result.was_randomized {
                println!("   (Step order was randomized)");
            }
            println!(
                "\n📋 Added {} items to pipeline:",
                result.created_items.len()
            );

            for (i, (instance, pipeline_item, action_title)) in
                result.created_items.iter().enumerate()
            {
                let pos = pipeline_item.position.unwrap_or((i + 1) as i64);
                let instance_prefix = &instance.id[..8.min(instance.id.len())];
                println!("   {}. [{}] {}", pos, instance_prefix, action_title);
            }

            println!("\nView pipeline with: subroutine-cli pipeline list");
            println!();
            Ok(())
        }

        RoutinesCommand::List => {
            let routines = fetch_routines(conn)?;

            if routines.is_empty() {
                println!("\nNo routines found.");
                println!("Create one with: subroutine-cli routines create <NAME>");
                return Ok(());
            }

            println!("\n📋 Routines ({}):", routines.len());
            println!("{}", "─".repeat(60));

            for routine in routines {
                let step_count = count_routine_steps(conn, &routine.id)?;
                let mode = if routine.is_sequential {
                    "sequential"
                } else {
                    "parallel"
                };
                let randomize = if routine.allow_randomization {
                    ", randomizable"
                } else {
                    ""
                };

                println!("\n📁 {}", routine.name);
                println!("   ID: {}", &routine.id[..8]);
                println!("   Steps: {} | Mode: {}{}", step_count, mode, randomize);

                if let Some(ref desc) = routine.description {
                    println!("   Description: {}", desc);
                }

                if let Some(ref start) = routine.default_start_time {
                    print!("   Schedule: {}", start);
                    if let Some(ref end) = routine.default_end_time {
                        print!(" - {}", end);
                    }
                    println!();
                }
            }

            println!();
            Ok(())
        }

        RoutinesCommand::Create {
            name,
            description,
            parallel,
            randomize,
            start_time,
            end_time,
        } => {
            let mut routine = Routine::new(name).is_sequential(!parallel);

            if *randomize {
                routine = routine.allow_randomization(true);
            }

            if let Some(desc) = description {
                routine = routine.description(desc);
            }

            if let Some(start) = start_time {
                routine = routine.default_start_time(start);
            }

            if let Some(end) = end_time {
                routine = routine.default_end_time(end);
            }

            insert_routine(conn, &routine)?;

            println!("\n✅ Routine created:");
            println!("   Name: {}", routine.name);
            println!("   ID: {}", routine.id);
            println!(
                "   Mode: {}",
                if routine.is_sequential {
                    "sequential"
                } else {
                    "parallel"
                }
            );

            if routine.allow_randomization {
                println!("   Randomization: enabled");
            }

            if let Some(ref desc) = routine.description {
                println!("   Description: {}", desc);
            }

            println!(
                "\nAdd steps with: subroutine-cli routines add-step \"{}\" <ACTION>",
                name
            );
            println!();
            Ok(())
        }

        RoutinesCommand::Show { identifier } => {
            let routine = resolve_routine(conn, identifier)?;
            let steps = fetch_routine_steps(conn, &routine.id)?;

            println!("\n📁 {}", routine.name);
            println!("{}", "─".repeat(60));
            println!("ID: {}", routine.id);

            if let Some(ref desc) = routine.description {
                println!("Description: {}", desc);
            }

            let mode = if routine.is_sequential {
                "sequential"
            } else {
                "parallel"
            };
            println!(
                "Mode: {}{}",
                mode,
                if routine.allow_randomization {
                    " (randomizable)"
                } else {
                    ""
                }
            );

            if let Some(ref start) = routine.default_start_time {
                print!("Schedule: {}", start);
                if let Some(ref end) = routine.default_end_time {
                    print!(" - {}", end);
                }
                println!();
            }

            if steps.is_empty() {
                println!("\nNo steps yet.");
                println!(
                    "Add steps with: subroutine-cli routines add-step \"{}\" <ACTION>",
                    routine.name
                );
            } else {
                println!("\n📝 Steps ({}):", steps.len());
                println!("{}", "─".repeat(40));

                for step in &steps {
                    let title = step.action_title.as_deref().unwrap_or("(unknown action)");
                    print!("  {}. {}", step.step_order, title);

                    let mut details = Vec::new();
                    if let Some(min) = step.min_duration_bucket {
                        if let Some(max) = step.max_duration_bucket {
                            details.push(format!("{}-{}min", min, max));
                        } else {
                            details.push(format!("≥{}min", min));
                        }
                    } else if let Some(max) = step.max_duration_bucket {
                        details.push(format!("≤{}min", max));
                    }

                    if !details.is_empty() {
                        print!(" ({})", details.join(", "));
                    }

                    println!();
                }
            }

            println!();
            Ok(())
        }

        RoutinesCommand::Delete { identifier } => {
            let routine = resolve_routine(conn, identifier)?;
            let step_count = count_routine_steps(conn, &routine.id)?;

            delete_routine(conn, &routine.id)?;

            println!("\n🗑️  Deleted routine: {}", routine.name);
            if step_count > 0 {
                println!("   ({} steps were also removed)", step_count);
            }

            println!();
            Ok(())
        }

        RoutinesCommand::AddStep {
            routine,
            action,
            position,
            min_duration,
            max_duration,
        } => {
            let routine = resolve_routine(conn, routine)?;
            let action = resolve_action(conn, action)?;

            // Determine the step order
            let step_order = if let Some(pos) = position {
                // Shift existing steps at or after this position
                shift_routine_steps(conn, &routine.id, *pos, 1)?;
                *pos
            } else {
                // Append at the end
                next_routine_step_order(conn, &routine.id)?
            };

            let mut step = RoutineStep::new(&routine.id, &action.id, step_order);

            if let Some(min) = min_duration {
                step = step.min_duration_bucket(*min);
            }

            if let Some(max) = max_duration {
                step = step.max_duration_bucket(*max);
            }

            insert_routine_step(conn, &step)?;

            println!("\n✅ Step added to '{}':", routine.name);
            println!("   Position: {}", step_order);
            println!("   Action: {}", action.title);

            if let Some(min) = min_duration {
                if let Some(max) = max_duration {
                    println!("   Duration: {}-{} min", min, max);
                } else {
                    println!("   Min Duration: {} min", min);
                }
            } else if let Some(max) = max_duration {
                println!("   Max Duration: {} min", max);
            }

            println!();
            Ok(())
        }

        RoutinesCommand::RemoveStep {
            routine,
            step_order,
        } => {
            let routine = resolve_routine(conn, routine)?;

            // Get the step to show what was deleted
            let steps = fetch_routine_steps(conn, &routine.id)?;
            let step = steps
                .iter()
                .find(|s| s.step_order == *step_order)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Step {} not found in routine '{}'",
                        step_order,
                        routine.name
                    )
                })?;

            let action_title = step
                .action_title
                .clone()
                .unwrap_or_else(|| "(unknown)".to_string());

            delete_routine_step_by_order(conn, &routine.id, *step_order)?;

            println!("\n🗑️  Removed step {} from '{}':", step_order, routine.name);
            println!("   Action: {}", action_title);
            println!("   (Remaining steps have been re-ordered)");

            println!();
            Ok(())
        }
    }
}
