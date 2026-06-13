use anyhow::{Result, bail};
use chrono::{Duration, Local, NaiveTime, TimeZone, Utc};
use clap::Subcommand;
use reqwest::blocking::Client;
use simple_core::{Action, ActionState, ActionTarget, Routine, RoutineStep};
use uuid::Uuid;

use crate::actions::{fetch_api_data, id_matches, short_id};

pub fn fetch_all_routines(client: &Client, base: &str) -> Result<Vec<Routine>> {
    Ok(fetch_api_data(client, base)?.routines)
}

#[derive(Debug, Subcommand)]
pub enum RoutinesCommand {
    /// List all routines
    List,
    /// Show details of a routine (by UUID prefix or title prefix)
    Show { identifier: String },
    /// Add a new routine
    Add {
        title: String,
        #[arg(short, long)]
        content: Option<String>,
        /// Steps as a comma-separated list, optionally with durations in minutes
        /// using "name:minutes" syntax (e.g. "Wake up:5,Shower:10,Breakfast:15")
        #[arg(short, long)]
        steps: Option<String>,
    },
    /// Delete a routine (by UUID prefix or title prefix)
    Delete { identifier: String },
    /// Instantiate a routine into ephemeral actions and add them to the queue.
    /// Each step becomes an action with a static target time. Steps are placed
    /// consecutively starting at the given time, each occupying its duration
    /// (or a default of 15 minutes if unspecified).
    Run {
        identifier: String,
        /// Start time in HH:MM (local time, today)
        time: String,
        /// Default duration in minutes for steps that have no duration set
        #[arg(short, long, default_value = "15")]
        default_duration: i64,
    },
}

pub fn handle_routines(command: &RoutinesCommand, client: &Client, base: &str) -> Result<()> {
    match command {
        RoutinesCommand::List => {
            let routines = fetch_all_routines(client, base)?;
            if routines.is_empty() {
                println!("No routines.");
                return Ok(());
            }
            for routine in &routines {
                println!(
                    "  {} {}  [{} steps]",
                    short_id(routine.id),
                    routine.title,
                    routine.steps.len()
                );
            }
        }
        RoutinesCommand::Show { identifier } => {
            let routine = resolve_routine(client, base, identifier)?;
            println!("Routine: {}", routine.title);
            println!("ID:      {}", routine.id);
            if let Some(ref content) = routine.content {
                println!("Content: {}", content);
            }
            if routine.steps.is_empty() {
                println!("Steps:   (none)");
            } else {
                println!("Steps:");
                for (i, step) in routine.steps.iter().enumerate() {
                    let duration_str = step
                        .duration
                        .map(|d| format!(" ({}m)", d.num_minutes()))
                        .unwrap_or_default();
                    println!("  {}. {}{}", i + 1, step.title, duration_str);
                }
            }
        }
        RoutinesCommand::Add {
            title,
            content,
            steps,
        } => {
            let mut routine = Routine::new(title);
            if let Some(c) = content {
                routine = routine.with_content(c);
            }
            if let Some(steps_str) = steps {
                let steps_vec: Vec<RoutineStep> = steps_str
                    .split(',')
                    .map(|s| {
                        let s = s.trim();
                        if let Some((name, mins_str)) = s.split_once(':') {
                            let mut step = RoutineStep::new(name.trim());
                            if let Ok(mins) = mins_str.trim().parse::<i64>() {
                                step = step.with_duration(Duration::minutes(mins));
                            }
                            step
                        } else {
                            RoutineStep::new(s)
                        }
                    })
                    .collect();
                routine = routine.with_steps(steps_vec);
            }
            let id = routine.id;
            client
                .put(format!("{}/api/routines/{}", base, id))
                .json(&routine)
                .send()?
                .error_for_status()?;
            println!("Added routine '{}' ({})", title, short_id(id));
        }
        RoutinesCommand::Delete { identifier } => {
            let routine = resolve_routine(client, base, identifier)?;
            client
                .delete(format!("{}/api/routines/{}", base, routine.id))
                .send()?
                .error_for_status()?;
            println!(
                "Deleted routine '{}' ({})",
                routine.title,
                short_id(routine.id)
            );
        }
        RoutinesCommand::Run {
            identifier,
            time,
            default_duration,
        } => {
            run(client, base, identifier, time, *default_duration)?;
        }
    }
    Ok(())
}

fn run(
    client: &Client,
    base: &str,
    identifier: &str,
    time: &str,
    default_duration_mins: i64,
) -> Result<()> {
    let routine = resolve_routine(client, base, identifier)?;

    if routine.steps.is_empty() {
        bail!("Routine '{}' has no steps to run.", routine.title);
    }

    let naive_time = NaiveTime::parse_from_str(time, "%H:%M")
        .map_err(|_| anyhow::anyhow!("Invalid time '{}'. Use HH:MM format.", time))?;
    let today = Local::now().date_naive();
    let naive_dt = today.and_time(naive_time);
    let local_dt = Local
        .from_local_datetime(&naive_dt)
        .single()
        .ok_or_else(|| anyhow::anyhow!("Could not convert local time to UTC."))?;
    let mut cursor = local_dt.with_timezone(&Utc);

    let default_step_duration = Duration::minutes(default_duration_mins);

    let mut spawned: Vec<(String, Uuid)> = Vec::new();

    for step in &routine.steps {
        let step_duration = step.duration.unwrap_or(default_step_duration);

        let action = Action::new(&step.title)
            .with_state(ActionState::Scheduled(ActionTarget {
                time: cursor,
                is_static: true,
            }))
            .with_duration(step_duration)
            .with_origin_routine(routine.id);

        let title = action.title.clone();
        let id = action.id;

        client
            .put(format!("{}/api/actions/{}", base, id))
            .json(&action)
            .send()?
            .error_for_status()?;

        spawned.push((title, id));
        cursor += step_duration;
    }

    println!(
        "Running '{}' — added {} action(s) to the queue starting at {}:",
        routine.title,
        spawned.len(),
        time,
    );
    for (title, id) in &spawned {
        println!("  {} {}", short_id(*id), title);
    }

    Ok(())
}

pub fn resolve_routine(client: &Client, base: &str, identifier: &str) -> Result<Routine> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        let routines = fetch_all_routines(client, base)?;
        if let Some(routine) = routines.into_iter().find(|r| r.id == uuid) {
            return Ok(routine);
        }
        bail!("No routine found with id '{}'", identifier);
    }

    let routines = fetch_all_routines(client, base)?;
    let matches: Vec<Routine> = routines
        .into_iter()
        .filter(|r| id_matches(r.id, &r.title, identifier))
        .collect();

    match matches.len() {
        0 => bail!("No routine found matching '{}'", identifier),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => bail!(
            "Multiple routines match '{}'. Use a more specific identifier.",
            identifier
        ),
    }
}
