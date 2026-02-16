use anyhow::Result;
use chrono::{Datelike, Local, Timelike};
use dialoguer::{Confirm, Input, MultiSelect, Select, theme::ColorfulTheme};
use rusqlite::Connection;

/// Automatically detect time_of_day string from the system clock
fn auto_time_of_day() -> &'static str {
    let hour = Local::now().hour();
    match hour {
        5..=11 => "morning",
        12..=16 => "afternoon",
        17..=20 => "evening",
        _ => "night",
    }
}

/// Automatically detect day_type from the system clock
fn auto_day_type() -> &'static str {
    let weekday = Local::now().weekday();
    match weekday {
        chrono::Weekday::Sat | chrono::Weekday::Sun => "weekend",
        _ => "weekday",
    }
}

/// Ensure there is a recent context snapshot (created within the last hour).
/// If none exists, auto-create one with detected time/day info.
fn ensure_auto_context(conn: &Connection) -> Result<()> {
    let needs_snapshot = match database::fetch_current_context(conn)? {
        None => true,
        Some(snapshot) => {
            if let Some(ref recorded_at) = snapshot.recorded_at {
                if let Ok(recorded) = chrono::DateTime::parse_from_rfc3339(recorded_at) {
                    let age = Local::now().signed_duration_since(recorded);
                    age.num_hours() >= 1
                } else {
                    true
                }
            } else {
                true
            }
        }
    };

    if needs_snapshot {
        let time_of_day = auto_time_of_day().to_string();
        let day_type = auto_day_type().to_string();

        let snapshot = database::ContextSnapshot {
            id: uuid::Uuid::new_v4().to_string(),
            recorded_at: Some(Local::now().to_rfc3339()),
            time_of_day: Some(time_of_day),
            device: None,
            created_at: Some(Local::now().to_rfc3339()),
            day_type: Some(day_type),
            environment: None,
            location: None,
            active_mental_state: None,
            metadata: Some(serde_json::json!({"auto_detected": true}).to_string()),
        };

        database::insert_context_snapshot(conn, &snapshot)?;
    }

    Ok(())
}

/// Display a compact status dashboard showing current context and pipeline state
fn show_status_dashboard(conn: &Connection) -> Result<()> {
    println!("┌─────────────────────────────────────────────────┐");
    println!("│  📊 Status                                      │");
    println!("├─────────────────────────────────────────────────┤");

    let time_of_day = auto_time_of_day();
    let day_type = auto_day_type();

    let mut energy_str = "??".to_string();
    let mut attention_str = "??".to_string();

    if let Some(snapshot) = database::fetch_current_context(conn)? {
        if let Some(ref metadata) = snapshot.metadata {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(metadata) {
                if let Some(energy) = meta.get("energy").and_then(|v| v.as_f64()) {
                    energy_str = format!("{:.0}%", energy * 100.0);
                }
                if let Some(attention) = meta.get("attention").and_then(|v| v.as_f64()) {
                    attention_str = format!("{:.0}%", attention * 100.0);
                }
            }
        }
    }

    let mental_state_str = match database::fetch_current_mental_state(conn)? {
        Some(state) => state.name,
        None => "—".to_string(),
    };

    let pipeline_items = database::fetch_pipeline_items(conn, database::DEFAULT_PIPELINE_ID)?;
    let pipeline_count = pipeline_items.len();

    println!(
        "│  ⏰ {} ({})  ⚡ {}  🧠 {}  🎭 {}",
        time_of_day, day_type, energy_str, attention_str, mental_state_str
    );
    println!("│  📋 Pipeline: {} item(s)", pipeline_count);

    if pipeline_count > 0 {
        let top_items: Vec<_> = pipeline_items.iter().take(3).collect();
        for item in &top_items {
            let title = item.action_title.as_deref().unwrap_or("(no title)");
            let pos = item.position.unwrap_or(0);
            println!("│     {}. {}", pos, title);
        }
        if pipeline_count > 3 {
            println!("│     ... and {} more", pipeline_count - 3);
        }
    }

    println!("└─────────────────────────────────────────────────┘");
    println!();

    Ok(())
}

/// Main interactive mode - presents a menu of common workflows
pub fn interactive_mode(conn: &Connection) -> Result<()> {
    ensure_auto_context(conn)?;

    loop {
        let theme = ColorfulTheme::default();

        show_status_dashboard(conn)?;

        let choices = vec![
            "🚀 What should I do next?",
            "⚡ Quick check-in (update energy & attention)",
            "➕ Quick add to pipeline",
            "🎯 View and work with pipeline",
            "📝 Create a new action (detailed)",
            "📁 Work with routines",
            "🧠 Record mental state",
            "📊 Full context capture",
            "📋 Quick action capture (batch mode)",
            "🔍 Explore existing actions",
            "📈 View completion patterns",
            "🚪 Exit",
        ];

        let selection = Select::with_theme(&theme)
            .with_prompt("What would you like to do?")
            .items(&choices)
            .default(0)
            .interact()?;

        match selection {
            0 => whats_next_flow(conn)?,
            1 => quick_checkin(conn)?,
            2 => quick_add_to_pipeline(conn)?,
            3 => pipeline_interactive(conn)?,
            4 => create_action_interactive(conn)?,
            5 => routines_interactive(conn)?,
            6 => record_mental_state_interactive(conn)?,
            7 => capture_context_interactive(conn)?,
            8 => batch_capture_actions(conn)?,
            9 => explore_actions_interactive(conn)?,
            10 => view_patterns_interactive(conn)?,
            11 => {
                println!("👋 Goodbye!");
                break;
            }
            _ => unreachable!(),
        }

        println!();
    }

    Ok(())
}

/// Quick check-in: just energy + attention, auto-detect time/day
fn quick_checkin(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n⚡ Quick Check-in\n");

    let time_of_day = auto_time_of_day();
    let day_type = auto_day_type();
    println!("  Auto-detected: {} ({})", time_of_day, day_type);

    let energy_options = vec![
        "🔴 Very low (10%)",
        "🟠 Low (30%)",
        "🟡 Moderate (50%)",
        "🟢 Good (70%)",
        "💚 High (90%)",
    ];
    let energy_idx = Select::with_theme(&theme)
        .with_prompt("How's your energy?")
        .items(&energy_options)
        .default(2)
        .interact()?;
    let energy = match energy_idx {
        0 => 0.1,
        1 => 0.3,
        2 => 0.5,
        3 => 0.7,
        4 => 0.9,
        _ => 0.5,
    };

    let attention_options = vec![
        "🔴 Can't focus at all (10%)",
        "🟠 Very scattered (30%)",
        "🟡 Okay (50%)",
        "🟢 Focused (70%)",
        "💚 Locked in (90%)",
    ];
    let attention_idx = Select::with_theme(&theme)
        .with_prompt("How's your focus?")
        .items(&attention_options)
        .default(2)
        .interact()?;
    let attention = match attention_idx {
        0 => 0.1,
        1 => 0.3,
        2 => 0.5,
        3 => 0.7,
        4 => 0.9,
        _ => 0.5,
    };

    let mut metadata = serde_json::Map::new();
    metadata.insert("energy".to_string(), serde_json::json!(energy));
    metadata.insert("attention".to_string(), serde_json::json!(attention));

    let snapshot = database::ContextSnapshot {
        id: uuid::Uuid::new_v4().to_string(),
        recorded_at: Some(Local::now().to_rfc3339()),
        time_of_day: Some(time_of_day.to_string()),
        device: None,
        created_at: Some(Local::now().to_rfc3339()),
        day_type: Some(day_type.to_string()),
        environment: None,
        location: None,
        active_mental_state: None,
        metadata: Some(serde_json::to_string(&metadata)?),
    };

    database::insert_context_snapshot(conn, &snapshot)?;

    println!(
        "\n✅ Updated! Energy: {:.0}% | Focus: {:.0}%",
        energy * 100.0,
        attention * 100.0
    );

    Ok(())
}

/// Streamlined "What should I do next?" flow
fn whats_next_flow(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n🚀 What Should I Do Next?\n");

    // Check if we have energy/attention info
    let has_context_info = if let Some(snapshot) = database::fetch_current_context(conn)? {
        if let Some(ref metadata) = snapshot.metadata {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(metadata) {
                meta.get("energy").and_then(|v| v.as_f64()).is_some()
                    && meta.get("attention").and_then(|v| v.as_f64()).is_some()
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    if !has_context_info {
        println!("💡 No energy/attention info yet. Let's do a quick check-in first.\n");
        quick_checkin(conn)?;
        println!();
    }

    // Check pipeline first
    let pipeline_items = database::fetch_pipeline_items(conn, database::DEFAULT_PIPELINE_ID)?;

    if pipeline_items.is_empty() {
        println!("📭 Your pipeline is empty!");
        println!();

        let choices = vec![
            "➕ Quick add a task to pipeline",
            "📝 Create a detailed action",
            "📁 Start a routine",
            "🔙 Back to main menu",
        ];

        let selection = Select::with_theme(&theme)
            .with_prompt("What would you like to do?")
            .items(&choices)
            .interact()?;

        match selection {
            0 => quick_add_to_pipeline(conn)?,
            1 => create_action_interactive(conn)?,
            2 => routines_interactive(conn)?,
            3 => {}
            _ => unreachable!(),
        }
        return Ok(());
    }

    // Score and show suggestions
    let suggestions = database::suggest_best_instances(conn, 3)?;

    if suggestions.is_empty() {
        println!("No scoreable instances found in pipeline.");
        println!("Your pipeline items may all be completed or lack associated actions.");
        return Ok(());
    }

    println!("Based on your current context, here are my top picks:\n");

    for (i, (_instance, action, score)) in suggestions.iter().enumerate() {
        let score_bar = score_to_bar(*score);
        println!("  {}. {} {}", i + 1, action.title, score_bar);

        let mut details = Vec::new();
        if let Some(duration) = action.duration_bucket {
            details.push(format!("~{} min", duration));
        }
        if let Some(energy) = action.energy_rate {
            let energy_label = if energy <= -3 {
                "draining"
            } else if energy >= 3 {
                "energizing"
            } else {
                "moderate"
            };
            details.push(energy_label.to_string());
        }
        if let Some(attention) = action.attention_level {
            details.push(format!("focus: {}/5", attention));
        }
        if !details.is_empty() {
            println!("     {}", details.join(" · "));
        }
    }

    println!();

    let mut action_choices: Vec<String> = suggestions
        .iter()
        .map(|(_, action, _)| format!("▶️  Start: {}", action.title))
        .collect();
    action_choices.push("📊 See detailed scoring".to_string());
    action_choices.push("🔙 Back to main menu".to_string());

    let choice = Select::with_theme(&theme)
        .with_prompt("Pick a task to start")
        .items(&action_choices)
        .default(0)
        .interact()?;

    if choice < suggestions.len() {
        let (instance, action, _score) = &suggestions[choice];
        start_task_flow(conn, &instance.id, &action.title)?;
    } else if choice == suggestions.len() {
        show_scoring_details(conn, &suggestions)?;
    }

    Ok(())
}

/// Visual score bar
fn score_to_bar(score: f64) -> String {
    let filled = (score * 10.0).round() as usize;
    let filled = filled.min(10);
    let empty = 10 - filled;
    format!(
        "[{}{}] {:.0}%",
        "█".repeat(filled),
        "░".repeat(empty),
        score * 100.0
    )
}

/// Show detailed scoring for suggestions
fn show_scoring_details(
    conn: &Connection,
    suggestions: &[(database::Instance, database::Action, f64)],
) -> Result<()> {
    let theme = ColorfulTheme::default();

    let names: Vec<String> = suggestions
        .iter()
        .map(|(_, action, score)| format!("{} [Score: {:.2}]", action.title, score))
        .collect();

    let selection = Select::with_theme(&theme)
        .with_prompt("Which task to explain?")
        .items(&names)
        .interact()?;

    let (instance, action, _) = &suggestions[selection];
    let scored = database::score_instance_with_context(conn, &instance.id)?;

    println!("\n📊 Scoring Breakdown: {}\n", action.title);
    println!("Total Score: {}\n", score_to_bar(scored.total_score));

    println!(
        "{:<20} {:>8} {:>8} {:>10}",
        "Factor", "Raw", "Weight", "Weighted"
    );
    println!("{}", "─".repeat(50));

    for factor in &scored.factor_scores {
        println!(
            "{:<20} {:>8.2} {:>8.2} {:>10.2}",
            factor.factor_name, factor.raw_score, factor.weight, factor.weighted_score
        );
        if let Some(ref explanation) = factor.explanation {
            println!("  └─ {}", explanation);
        }
    }

    Ok(())
}

/// Start a task: mark as active, then offer completion flow
fn start_task_flow(conn: &Connection, instance_id: &str, action_title: &str) -> Result<()> {
    let theme = ColorfulTheme::default();

    database::set_instance_status(conn, instance_id, "active")?;

    println!("\n▶️  Started: {}", action_title);
    println!("   Take your time. Come back when you're done.\n");

    let choices = vec![
        "✅ Done! Mark as completed",
        "⏭️  Skip this task",
        "😴 Snooze (do it later)",
        "🔙 Leave running (back to menu)",
    ];

    let selection = Select::with_theme(&theme)
        .with_prompt("When you're ready")
        .items(&choices)
        .interact()?;

    match selection {
        0 => complete_task_flow(conn, instance_id, action_title)?,
        1 => {
            database::set_instance_status(conn, instance_id, "scheduled")?;
            auto_record_event(conn, instance_id, database::EventType::Skipped, None)?;
            println!("⏭️  Skipped. It'll stay in your pipeline for later.");
        }
        2 => {
            database::set_instance_status(conn, instance_id, "scheduled")?;
            auto_record_event(conn, instance_id, database::EventType::Snoozed, None)?;
            println!("😴 Snoozed. We'll suggest it again later.");
        }
        3 => {
            println!("🏃 Task is still running. You can complete it from the pipeline menu.");
        }
        _ => unreachable!(),
    }

    Ok(())
}

/// Complete a task: mark completed, record event, remove from pipeline, suggest next
fn complete_task_flow(conn: &Connection, instance_id: &str, action_title: &str) -> Result<()> {
    let theme = ColorfulTheme::default();

    // Mark instance as completed
    database::set_instance_status(conn, instance_id, "completed")?;

    // Find and remove the pipeline item for this instance
    let pipeline_items = database::fetch_pipeline_items(conn, database::DEFAULT_PIPELINE_ID)?;
    for item in &pipeline_items {
        if item.instance_id.as_deref() == Some(instance_id) {
            database::delete_pipeline_item(conn, &item.id)?;
            break;
        }
    }

    // Get the action_id from the instance for event recording
    let instances = database::fetch_instances(conn)?;
    let action_id = instances
        .iter()
        .find(|i| i.id == instance_id)
        .map(|i| i.action_id.clone());

    // Auto-record a completed event
    database::insert_tracked_event(
        conn,
        database::EventType::Completed,
        Some(instance_id),
        action_id.as_deref(),
        None,
    )?;

    println!("\n🎉 Completed: {}!", action_title);
    println!("   (Event recorded automatically)");

    // Show what's next
    let remaining = database::fetch_pipeline_items(conn, database::DEFAULT_PIPELINE_ID)?;

    if remaining.is_empty() {
        println!("\n🏆 Pipeline is clear! Great work!");
        return Ok(());
    }

    println!("\n📋 {} item(s) remaining in pipeline.", remaining.len());

    let continue_working = Confirm::with_theme(&theme)
        .with_prompt("Want to see what's next?")
        .default(true)
        .interact()?;

    if continue_working {
        whats_next_flow(conn)?;
    }

    Ok(())
}

/// Record a tracked event automatically (without prompting)
fn auto_record_event(
    conn: &Connection,
    instance_id: &str,
    event_type: database::EventType,
    note: Option<&str>,
) -> Result<()> {
    let instances = database::fetch_instances(conn)?;
    let action_id = instances
        .iter()
        .find(|i| i.id == instance_id)
        .map(|i| i.action_id.clone());

    database::insert_tracked_event(
        conn,
        event_type,
        Some(instance_id),
        action_id.as_deref(),
        note,
    )?;

    Ok(())
}

/// Quick add: enter a title, immediately create action + instance + add to pipeline
fn quick_add_to_pipeline(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n➕ Quick Add to Pipeline\n");

    let title: String = Input::with_theme(&theme)
        .with_prompt("What do you need to do?")
        .interact_text()?;

    if title.trim().is_empty() {
        println!("❌ No title provided.");
        return Ok(());
    }

    let title = title.trim().to_string();

    // Quick optional duration
    let duration_options = vec![
        "Skip (decide later)",
        "~1 min",
        "~5 min",
        "~13 min",
        "~21 min",
        "~34 min",
        "~55 min",
    ];
    let duration_idx = Select::with_theme(&theme)
        .with_prompt("Roughly how long?")
        .items(&duration_options)
        .default(0)
        .interact()?;
    let duration = match duration_idx {
        1 => Some(1),
        2 => Some(5),
        3 => Some(13),
        4 => Some(21),
        5 => Some(34),
        6 => Some(55),
        _ => None,
    };

    // Create action
    let mut action = database::Action::new("task", &title);
    if let Some(d) = duration {
        action = action.duration_bucket(d);
    }
    let action_id = action.id.clone();
    database::insert_action(conn, &action)?;

    // Create instance and enqueue
    let mut instance = database::Instance::new(&action_id);
    instance.source = Some("manual".to_string());
    let instance_id = instance.id.clone();
    database::insert_instance(conn, &instance)?;
    database::enqueue_instance(conn, &instance_id, Some(&title))?;

    println!("\n✅ Added to pipeline: {}", title);
    if let Some(d) = duration {
        println!("   Duration: ~{} min", d);
    }

    // Offer to add more
    let add_more = Confirm::with_theme(&theme)
        .with_prompt("Add another?")
        .default(false)
        .interact()?;

    if add_more {
        quick_add_to_pipeline(conn)?;
    }

    Ok(())
}

/// View completion patterns
fn view_patterns_interactive(conn: &Connection) -> Result<()> {
    println!("\n📈 Completion Patterns\n");

    let patterns = database::fetch_event_patterns(conn, None, Some(10))?;

    if patterns.is_empty() {
        println!("No event data yet. Complete some tasks to see patterns!");
        return Ok(());
    }

    println!(
        "{:<30} {:>6} {:>6} {:>6} {:>6} {:>8}",
        "Action", "Done", "Skip", "Snz", "Abnd", "Rate"
    );
    println!("{}", "─".repeat(70));

    for pattern in &patterns {
        let title = if pattern.action_title.len() > 28 {
            format!("{}…", &pattern.action_title[..27])
        } else {
            pattern.action_title.clone()
        };

        let rate_bar = {
            let filled = (pattern.completion_rate * 10.0).round() as usize;
            let filled = filled.min(10);
            let empty = 10 - filled;
            format!("{}{}", "█".repeat(filled), "░".repeat(empty),)
        };

        println!(
            "{:<30} {:>6} {:>6} {:>6} {:>6} {} {:.0}%",
            title,
            pattern.completed_count,
            pattern.skipped_count,
            pattern.snoozed_count,
            pattern.abandoned_count,
            rate_bar,
            pattern.completion_rate * 100.0,
        );
    }

    Ok(())
}

/// Interactive action creation with guided questions
fn create_action_interactive(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n🎯 Let's create a new action!\n");

    let title: String = Input::with_theme(&theme)
        .with_prompt("What is this action?")
        .interact_text()?;

    let action_types = vec!["task", "activity", "habit", "event"];
    let action_type_idx = Select::with_theme(&theme)
        .with_prompt("What type of action is this?")
        .items(&action_types)
        .default(0)
        .interact()?;
    let action_type = action_types[action_type_idx];

    let description: String = Input::with_theme(&theme)
        .with_prompt("Description (optional, press Enter to skip)")
        .allow_empty(true)
        .interact_text()?;

    println!("\n📏 Now let's understand the executive function aspects...\n");

    let duration_options = vec![
        "1 min", "2 min", "3 min", "5 min", "8 min", "13 min", "21 min", "34 min", "55 min",
        "89 min", "144 min", "Skip",
    ];
    let duration_idx = Select::with_theme(&theme)
        .with_prompt("About how long does this take?")
        .items(&duration_options)
        .default(3)
        .interact()?;
    let duration = if duration_idx < duration_options.len() - 1 {
        Some(match duration_idx {
            0 => 1,
            1 => 2,
            2 => 3,
            3 => 5,
            4 => 8,
            5 => 13,
            6 => 21,
            7 => 34,
            8 => 55,
            9 => 89,
            10 => 144,
            _ => unreachable!(),
        })
    } else {
        None
    };

    let energy_options = vec![
        "-5 (Very draining)",
        "-3 (Somewhat draining)",
        "0 (Neutral)",
        "+3 (Somewhat energizing)",
        "+5 (Very energizing)",
        "Skip",
    ];
    let energy_idx = Select::with_theme(&theme)
        .with_prompt("How does this affect your energy?")
        .items(&energy_options)
        .default(2)
        .interact()?;
    let energy = if energy_idx < energy_options.len() - 1 {
        Some(match energy_idx {
            0 => -5,
            1 => -3,
            2 => 0,
            3 => 3,
            4 => 5,
            _ => unreachable!(),
        })
    } else {
        None
    };

    let attention_options = vec![
        "1 (Minimal - can do while distracted)",
        "2 (Light - some focus needed)",
        "3 (Moderate - steady focus required)",
        "4 (High - deep focus needed)",
        "5 (Intense - complete concentration)",
        "Skip",
    ];
    let attention_idx = Select::with_theme(&theme)
        .with_prompt("How much attention does this require?")
        .items(&attention_options)
        .default(2)
        .interact()?;
    let attention = if attention_idx < attention_options.len() - 1 {
        Some((attention_idx + 1) as i64)
    } else {
        None
    };

    let transition_options = vec![
        "1 (Easy to start and stop)",
        "2 (Fairly easy transitions)",
        "3 (Moderate transition difficulty)",
        "4 (Hard to start or stop)",
        "5 (Very difficult to transition)",
        "Skip",
    ];
    let transition_idx = Select::with_theme(&theme)
        .with_prompt("How hard is this to start or stop?")
        .items(&transition_options)
        .default(2)
        .interact()?;
    let transition_difficulty = if transition_idx < transition_options.len() - 1 {
        Some((transition_idx + 1) as i64)
    } else {
        None
    };

    let enjoyment_options = vec![
        "-5 (Really unpleasant)",
        "-3 (Somewhat unpleasant)",
        "0 (Neutral)",
        "+3 (Somewhat enjoyable)",
        "+5 (Really enjoyable)",
        "Skip",
    ];
    let enjoyment_idx = Select::with_theme(&theme)
        .with_prompt("How enjoyable is this once you've started?")
        .items(&enjoyment_options)
        .default(2)
        .interact()?;
    let enjoyment = if enjoyment_idx < enjoyment_options.len() - 1 {
        Some(match enjoyment_idx {
            0 => -5,
            1 => -3,
            2 => 0,
            3 => 3,
            4 => 5,
            _ => unreachable!(),
        })
    } else {
        None
    };

    println!("\n🎯 Finally, let's consider importance and urgency...\n");

    let importance_options = vec![
        "1 (Low importance)",
        "2 (Some importance)",
        "3 (Moderate importance)",
        "4 (High importance)",
        "5 (Critical importance)",
        "Skip",
    ];
    let importance_idx = Select::with_theme(&theme)
        .with_prompt("How important is this action?")
        .items(&importance_options)
        .default(2)
        .interact()?;
    let importance = if importance_idx < importance_options.len() - 1 {
        Some((importance_idx + 1) as i64)
    } else {
        None
    };

    let urgency_growth = Confirm::with_theme(&theme)
        .with_prompt("Does this become more urgent over time?")
        .default(false)
        .interact()?;

    let mut action = database::Action::new(action_type, &title);

    if !description.is_empty() {
        action = action.description(&description);
    }
    if let Some(d) = duration {
        action = action.duration_bucket(d);
    }
    if let Some(e) = energy {
        action = action.energy_rate(e);
    }
    if let Some(a) = attention {
        action = action.attention_level(a);
    }
    if let Some(t) = transition_difficulty {
        action = action.transition_difficulty(t);
    }
    if let Some(e) = enjoyment {
        action = action.enjoyment_after_start(e);
    }
    if let Some(i) = importance {
        action = action.importance(i);
    }
    if urgency_growth {
        action = action.urgency_growth(true);
    }

    let id = action.id.clone();
    database::insert_action(conn, &action)?;

    println!("\n✅ Created action '{}' ({})", title, &id[..8]);

    // Offer to add to pipeline immediately
    let add_to_pipeline = Confirm::with_theme(&theme)
        .with_prompt("Add to pipeline now?")
        .default(true)
        .interact()?;

    if add_to_pipeline {
        let mut instance = database::Instance::new(&id);
        instance.source = Some("manual".to_string());
        let instance_id = instance.id.clone();
        database::insert_instance(conn, &instance)?;
        database::enqueue_instance(conn, &instance_id, Some(&title))?;
        println!("📋 Added to pipeline!");
    }

    Ok(())
}

/// Full context capture with all the details
fn capture_context_interactive(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n📊 Full Context Capture\n");

    let detected_time = auto_time_of_day();
    let detected_day = auto_day_type();
    println!("  Auto-detected: {} ({})\n", detected_time, detected_day);

    let time_options = vec!["morning", "afternoon", "evening", "night"];
    let time_indices = MultiSelect::with_theme(&theme)
        .with_prompt("Time of day (auto-selected, adjust if needed)")
        .items(&time_options)
        .defaults(
            &time_options
                .iter()
                .map(|t| *t == detected_time)
                .collect::<Vec<_>>(),
        )
        .interact()?;
    let time_of_day: Vec<String> = time_indices
        .iter()
        .map(|&i| time_options[i].to_string())
        .collect();

    let env_options = vec![
        "quiet", "noisy", "social", "solitary", "indoors", "outdoors", "home", "work",
    ];
    let env_indices = MultiSelect::with_theme(&theme)
        .with_prompt("Describe your environment (select all that apply)")
        .items(&env_options)
        .interact()?;
    let environment: Vec<String> = env_indices
        .iter()
        .map(|&i| env_options[i].to_string())
        .collect();

    let location_options = vec!["home", "work", "transit", "store", "outdoors", "other"];
    let location_indices = MultiSelect::with_theme(&theme)
        .with_prompt("Where are you? (select all that apply)")
        .items(&location_options)
        .interact()?;
    let location: Vec<String> = location_indices
        .iter()
        .map(|&i| location_options[i].to_string())
        .collect();

    let energy: f64 = Input::with_theme(&theme)
        .with_prompt("Energy level (0.0 = depleted, 1.0 = full energy)")
        .default(0.5)
        .interact_text()?;

    let attention: f64 = Input::with_theme(&theme)
        .with_prompt("Attention capacity (0.0 = can't focus, 1.0 = peak focus)")
        .default(0.5)
        .interact_text()?;

    let mental_states = database::fetch_mental_states(conn)?;
    let active_mental_state = if !mental_states.is_empty() {
        let add_mental_state = Confirm::with_theme(&theme)
            .with_prompt("Link to a mental state?")
            .default(false)
            .interact()?;

        if add_mental_state {
            let state_names: Vec<String> = mental_states.iter().map(|s| s.name.clone()).collect();
            let state_idx = Select::with_theme(&theme)
                .with_prompt("Which mental state?")
                .items(&state_names)
                .interact()?;
            Some(mental_states[state_idx].id.clone())
        } else {
            None
        }
    } else {
        None
    };

    let mut metadata = serde_json::Map::new();
    metadata.insert("energy".to_string(), serde_json::json!(energy));
    metadata.insert("attention".to_string(), serde_json::json!(attention));

    let snapshot = database::ContextSnapshot {
        id: uuid::Uuid::new_v4().to_string(),
        recorded_at: Some(Local::now().to_rfc3339()),
        time_of_day: if time_of_day.is_empty() {
            Some(detected_time.to_string())
        } else {
            Some(time_of_day.join(","))
        },
        device: None,
        created_at: Some(Local::now().to_rfc3339()),
        day_type: Some(detected_day.to_string()),
        environment: if environment.is_empty() {
            None
        } else {
            Some(environment.join(","))
        },
        location: if location.is_empty() {
            None
        } else {
            Some(location.join(","))
        },
        active_mental_state,
        metadata: Some(serde_json::to_string(&metadata)?),
    };

    database::insert_context_snapshot(conn, &snapshot)?;

    println!("\n✅ Context snapshot captured!");
    println!("   Energy: {:.0}%", energy * 100.0);
    println!("   Attention: {:.0}%", attention * 100.0);
    if !time_of_day.is_empty() {
        println!("   Time: {}", time_of_day.join(", "));
    }
    if !environment.is_empty() {
        println!("   Environment: {}", environment.join(", "));
    }
    if !location.is_empty() {
        println!("   Location: {}", location.join(", "));
    }

    Ok(())
}

/// Interactive mental state recording
fn record_mental_state_interactive(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n🧠 How are you feeling?\n");

    let mental_states = database::fetch_mental_states(conn)?;

    if mental_states.is_empty() {
        println!("No mental states defined yet. Let's create one first!\n");

        let name: String = Input::with_theme(&theme)
            .with_prompt("Name for this mental state")
            .interact_text()?;

        let description: String = Input::with_theme(&theme)
            .with_prompt("Description (optional)")
            .allow_empty(true)
            .interact_text()?;

        let state = database::MentalState {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.clone(),
            description: if description.is_empty() {
                None
            } else {
                Some(description)
            },
            created_at: Some(Local::now().to_rfc3339()),
        };

        database::insert_mental_state(conn, &state)?;
        println!("✅ Created mental state '{}'", name);

        record_mental_state_event(conn, &state)?;
    } else {
        let mut state_names: Vec<String> = mental_states.iter().map(|s| s.name.clone()).collect();
        state_names.push("➕ Create new mental state".to_string());

        let state_idx = Select::with_theme(&theme)
            .with_prompt("How are you feeling?")
            .items(&state_names)
            .interact()?;

        if state_idx == mental_states.len() {
            let name: String = Input::with_theme(&theme)
                .with_prompt("Name for this mental state")
                .interact_text()?;

            let description: String = Input::with_theme(&theme)
                .with_prompt("Description (optional)")
                .allow_empty(true)
                .interact_text()?;

            let state = database::MentalState {
                id: uuid::Uuid::new_v4().to_string(),
                name: name.clone(),
                description: if description.is_empty() {
                    None
                } else {
                    Some(description)
                },
                created_at: Some(Local::now().to_rfc3339()),
            };

            database::insert_mental_state(conn, &state)?;
            println!("✅ Created mental state '{}'", name);
            record_mental_state_event(conn, &state)?;
        } else {
            let state = &mental_states[state_idx];
            record_mental_state_event(conn, state)?;
        }
    }

    Ok(())
}

fn record_mental_state_event(conn: &Connection, state: &database::MentalState) -> Result<()> {
    let theme = ColorfulTheme::default();

    let intensity_options = vec![
        "1 (Very mild)",
        "2 (Mild)",
        "3 (Moderate)",
        "4 (Strong)",
        "5 (Very strong)",
    ];
    let intensity_idx = Select::with_theme(&theme)
        .with_prompt("How intensely?")
        .items(&intensity_options)
        .default(2)
        .interact()?;
    let intensity = (intensity_idx + 1) as i64;

    let event = database::MentalStateEvent {
        id: uuid::Uuid::new_v4().to_string(),
        mental_state_id: state.id.clone(),
        intensity: Some(intensity),
        recorded_at: Some(Local::now().to_rfc3339()),
        context_snapshot_id: None,
    };

    database::insert_mental_state_event(conn, &event)?;
    println!("\n✅ Recorded '{}' at intensity {}", state.name, intensity);

    Ok(())
}

fn pipeline_interactive(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n🎯 Pipeline Management\n");

    let items = database::fetch_pipeline_items(conn, database::DEFAULT_PIPELINE_ID)?;

    if items.is_empty() {
        println!("📭 Pipeline is empty.\n");

        let choices = vec![
            "💡 Get smart task suggestions",
            "➕ Quick add to pipeline",
            "📁 Start a routine",
            "🔙 Back to main menu",
        ];

        let selection = Select::with_theme(&theme)
            .with_prompt("What would you like to do?")
            .items(&choices)
            .interact()?;

        match selection {
            0 => whats_next_flow(conn)?,
            1 => quick_add_to_pipeline(conn)?,
            2 => routines_interactive(conn)?,
            3 => {}
            _ => unreachable!(),
        }
        return Ok(());
    }

    // Show scored pipeline items
    let scored_items = database::score_pipeline_items(conn, database::DEFAULT_PIPELINE_ID)?;
    let score_map: std::collections::HashMap<_, _> = scored_items
        .into_iter()
        .map(|(item, score)| (item.id.clone(), score))
        .collect();

    println!("Pipeline has {} item(s):\n", items.len());
    let instances = database::fetch_instances(conn)?;
    for item in &items {
        let position = item.position.unwrap_or(0);
        let title = item.action_title.as_deref().unwrap_or("(no title)");
        let status = if let Some(instance_id) = &item.instance_id {
            instances
                .iter()
                .find(|i| &i.id == instance_id)
                .map(|i| i.status.as_str())
                .unwrap_or("unknown")
        } else {
            "no instance"
        };
        let score = score_map.get(&item.id).copied().unwrap_or(0.0);
        let bar = score_to_bar(score);
        println!("  {}. {} ({}) {}", position, title, status, bar);
    }

    println!();

    let choices = vec![
        "▶️  Start working on a task",
        "🔄 Refresh pipeline (re-score and re-order)",
        "📊 Explain scoring for an item",
        "✅ Complete an item",
        "⏭️  Skip an item",
        "➕ Quick add to pipeline",
        "🔙 Back to main menu",
    ];

    let selection = Select::with_theme(&theme)
        .with_prompt("What would you like to do?")
        .items(&choices)
        .interact()?;

    match selection {
        0 => {
            let item_names: Vec<String> = items
                .iter()
                .map(|item| {
                    let title = item.action_title.as_deref().unwrap_or("(no title)");
                    let score = score_map.get(&item.id).copied().unwrap_or(0.0);
                    format!("{} {}", title, score_to_bar(score))
                })
                .collect();

            let item_idx = Select::with_theme(&theme)
                .with_prompt("Which task to start?")
                .items(&item_names)
                .interact()?;

            let item = &items[item_idx];
            if let Some(ref instance_id) = item.instance_id {
                let title = item.action_title.as_deref().unwrap_or("(no title)");
                start_task_flow(conn, instance_id, title)?;
            } else {
                println!("⚠️  This pipeline item has no associated instance.");
            }
        }
        1 => {
            println!("\n🔄 Refreshing pipeline based on current context...\n");

            let scored_items = database::score_pipeline_items(conn, database::DEFAULT_PIPELINE_ID)?;
            if scored_items.is_empty() {
                println!("Pipeline is empty - nothing to refresh.");
                return Ok(());
            }

            let mut sorted_items = scored_items;
            sorted_items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (new_position, (item, score)) in sorted_items.iter().enumerate() {
                let new_pos = (new_position + 1) as i64;
                database::update_pipeline_item_position(conn, &item.id, new_pos)?;

                let title = item.action_title.as_deref().unwrap_or("(no title)");
                let old_pos = item.position.unwrap_or(0);

                if old_pos != new_pos {
                    println!(
                        "  {} moved: {} → {} (score: {:.2})",
                        title, old_pos, new_pos, score
                    );
                } else {
                    println!(
                        "  {} stayed at position {} (score: {:.2})",
                        title, new_pos, score
                    );
                }
            }

            println!("\n✅ Pipeline refreshed and reordered by score!");
        }
        2 => {
            let item_names: Vec<String> = items
                .iter()
                .map(|item| {
                    let title = item.action_title.as_deref().unwrap_or("(no title)");
                    let score = score_map.get(&item.id).copied().unwrap_or(0.0);
                    format!("{} [Score: {:.2}]", title, score)
                })
                .collect();

            let item_idx = Select::with_theme(&theme)
                .with_prompt("Which item to explain?")
                .items(&item_names)
                .interact()?;

            let item = &items[item_idx];
            let instance_id = item
                .instance_id
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Pipeline item has no associated instance"))?;

            let scored = database::score_instance_with_context(conn, instance_id)?;

            println!(
                "\n📊 Scoring Breakdown for: {}\n",
                item.action_title.as_deref().unwrap_or("(no title)")
            );
            println!("Total Score: {}\n", score_to_bar(scored.total_score));

            println!(
                "{:<20} {:>8} {:>8} {:>10}",
                "Factor", "Raw", "Weight", "Weighted"
            );
            println!("{}", "─".repeat(50));

            for factor in &scored.factor_scores {
                println!(
                    "{:<20} {:>8.2} {:>8.2} {:>10.2}",
                    factor.factor_name, factor.raw_score, factor.weight, factor.weighted_score
                );
                if let Some(ref explanation) = factor.explanation {
                    println!("  └─ {}", explanation);
                }
            }
        }
        3 => {
            let item_names: Vec<String> = items
                .iter()
                .map(|item| {
                    item.action_title
                        .as_deref()
                        .unwrap_or("(no title)")
                        .to_string()
                })
                .collect();

            let item_idx = Select::with_theme(&theme)
                .with_prompt("Which item to complete?")
                .items(&item_names)
                .interact()?;

            let item = &items[item_idx];
            let title = item
                .action_title
                .as_deref()
                .unwrap_or("(no title)")
                .to_string();

            if let Some(ref instance_id) = item.instance_id {
                complete_task_flow(conn, instance_id, &title)?;
            } else {
                database::delete_pipeline_item(conn, &item.id)?;
                println!("\n✅ Removed from pipeline!");
            }
        }
        4 => {
            let item_names: Vec<String> = items
                .iter()
                .map(|item| {
                    item.action_title
                        .as_deref()
                        .unwrap_or("(no title)")
                        .to_string()
                })
                .collect();

            let item_idx = Select::with_theme(&theme)
                .with_prompt("Which item to skip?")
                .items(&item_names)
                .interact()?;

            let item = &items[item_idx];
            if let Some(ref instance_id) = item.instance_id {
                auto_record_event(conn, instance_id, database::EventType::Skipped, None)?;
                println!(
                    "\n⏭️  Skipped: {}",
                    item.action_title.as_deref().unwrap_or("(no title)")
                );
                println!("   (Event recorded automatically)");
            }
        }
        5 => {
            quick_add_to_pipeline(conn)?;
        }
        6 => {}
        _ => unreachable!(),
    }

    Ok(())
}

fn routines_interactive(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n📁 Routine Management\n");

    let routines = database::fetch_routines(conn)?;

    if routines.is_empty() {
        println!("📭 No routines yet.\n");

        let choices = vec!["➕ Create a new routine", "🔙 Back to main menu"];

        let selection = Select::with_theme(&theme)
            .with_prompt("What would you like to do?")
            .items(&choices)
            .interact()?;

        match selection {
            0 => create_routine_interactive(conn)?,
            1 => {}
            _ => unreachable!(),
        }
        return Ok(());
    }

    // List routines with step counts
    println!("Available routines:\n");
    for routine in &routines {
        let step_count = database::count_routine_steps(conn, &routine.id)?;
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
        println!(
            "  📁 {} ({} steps, {}{})",
            routine.name, step_count, mode, randomize
        );
    }
    println!();

    let choices = vec![
        "🚀 Start a routine (add to pipeline)",
        "👁️  View routine details",
        "➕ Create a new routine",
        "📝 Add step to a routine",
        "🗑️  Delete a routine",
        "🔙 Back to main menu",
    ];

    let selection = Select::with_theme(&theme)
        .with_prompt("What would you like to do?")
        .items(&choices)
        .interact()?;

    match selection {
        0 => start_routine_interactive(conn, &routines)?,
        1 => view_routine_interactive(conn, &routines)?,
        2 => create_routine_interactive(conn)?,
        3 => add_step_to_routine_interactive(conn, &routines)?,
        4 => delete_routine_interactive(conn, &routines)?,
        5 => {}
        _ => unreachable!(),
    }

    Ok(())
}

/// Start a routine by instantiating it into the pipeline
fn start_routine_interactive(conn: &Connection, routines: &[database::Routine]) -> Result<()> {
    let theme = ColorfulTheme::default();

    let routine_names: Vec<String> = routines.iter().map(|r| r.name.clone()).collect();

    let routine_idx = Select::with_theme(&theme)
        .with_prompt("Which routine do you want to start?")
        .items(&routine_names)
        .interact()?;

    let routine = &routines[routine_idx];
    let step_count = database::count_routine_steps(conn, &routine.id)?;

    if step_count == 0 {
        println!("\n⚠️  Routine '{}' has no steps.", routine.name);
        println!("Add steps first with the 'Add step to a routine' option.");
        return Ok(());
    }

    // Ask about randomization if the routine allows it
    let randomize = if routine.allow_randomization {
        Confirm::with_theme(&theme)
            .with_prompt("This routine allows randomization. Randomize step order?")
            .default(false)
            .interact()?
    } else {
        false
    };

    let options = database::InstantiateRoutineOptions {
        randomize: Some(randomize),
        start_position: None,
        pipeline_id: None,
    };

    let result = database::instantiate_routine(conn, routine, options)?;

    println!("\n🚀 Started routine: {}", routine.name);
    if result.was_randomized {
        println!("   (Step order was randomized)");
    }
    println!(
        "\n📋 Added {} items to pipeline:",
        result.created_items.len()
    );

    for (i, (_instance, pipeline_item, action_title)) in result.created_items.iter().enumerate() {
        let pos = pipeline_item.position.unwrap_or((i + 1) as i64);
        println!("   {}. {}", pos, action_title);
    }

    // Offer to start working immediately
    let start_now = Confirm::with_theme(&theme)
        .with_prompt("Start working on the first task now?")
        .default(true)
        .interact()?;

    if start_now {
        if let Some((instance, _pipeline_item, action_title)) = result.created_items.first() {
            start_task_flow(conn, &instance.id, action_title)?;
        }
    } else {
        println!(
            "\nUse the Pipeline menu or 'What should I do next?' to work through these tasks!"
        );
    }

    Ok(())
}

/// View details of a routine
fn view_routine_interactive(conn: &Connection, routines: &[database::Routine]) -> Result<()> {
    let theme = ColorfulTheme::default();

    let routine_names: Vec<String> = routines.iter().map(|r| r.name.clone()).collect();

    let routine_idx = Select::with_theme(&theme)
        .with_prompt("Which routine do you want to view?")
        .items(&routine_names)
        .interact()?;

    let routine = &routines[routine_idx];
    let steps = database::fetch_routine_steps(conn, &routine.id)?;

    println!("\n📁 {}", routine.name);
    println!("{}", "─".repeat(50));

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

    if steps.is_empty() {
        println!("\nNo steps yet.");
    } else {
        println!("\nSteps ({}):", steps.len());
        for step in &steps {
            let title = step.action_title.as_deref().unwrap_or("(unknown action)");
            print!("  {}. {}", step.step_order, title);

            if let Some(min) = step.min_duration_bucket {
                if let Some(max) = step.max_duration_bucket {
                    print!(" ({}-{}min)", min, max);
                } else {
                    print!(" (≥{}min)", min);
                }
            } else if let Some(max) = step.max_duration_bucket {
                print!(" (≤{}min)", max);
            }

            println!();
        }
    }

    Ok(())
}

/// Create a new routine interactively
fn create_routine_interactive(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n➕ Create a New Routine\n");

    let name: String = Input::with_theme(&theme)
        .with_prompt("Routine name")
        .interact_text()?;

    let description: String = Input::with_theme(&theme)
        .with_prompt("Description (optional, press Enter to skip)")
        .allow_empty(true)
        .interact_text()?;

    let is_parallel = Confirm::with_theme(&theme)
        .with_prompt("Is this a parallel routine? (steps can be done in any order)")
        .default(false)
        .interact()?;

    let allow_randomization = if !is_parallel {
        Confirm::with_theme(&theme)
            .with_prompt("Allow randomizing step order when starting?")
            .default(false)
            .interact()?
    } else {
        false
    };

    let mut routine = database::Routine::new(&name).is_sequential(!is_parallel);

    if !description.is_empty() {
        routine = routine.description(&description);
    }

    if allow_randomization {
        routine = routine.allow_randomization(true);
    }

    database::insert_routine(conn, &routine)?;

    println!("\n✅ Routine '{}' created!", name);

    // Offer to add steps immediately
    let add_steps = Confirm::with_theme(&theme)
        .with_prompt("Add steps now?")
        .default(true)
        .interact()?;

    if add_steps {
        let actions = database::fetch_actions(conn)?;
        if actions.is_empty() {
            println!("\n⚠️  No actions available yet. Create some actions first, then add steps.");
        } else {
            loop {
                let action_names: Vec<String> = actions.iter().map(|a| a.title.clone()).collect();
                let mut items = action_names.clone();
                items.push("✅ Done adding steps".to_string());

                let action_idx = Select::with_theme(&theme)
                    .with_prompt("Add which action as a step?")
                    .items(&items)
                    .interact()?;

                if action_idx == actions.len() {
                    break;
                }

                let action = &actions[action_idx];
                let next_order = database::next_routine_step_order(conn, &routine.id)?;
                let step = database::RoutineStep::new(&routine.id, &action.id, next_order);
                database::insert_routine_step(conn, &step)?;
                println!("  ✓ Added '{}' as step {}", action.title, next_order);
            }
        }
    } else {
        println!("Add steps with the 'Add step to a routine' option.");
    }

    Ok(())
}

/// Add a step to an existing routine
fn add_step_to_routine_interactive(
    conn: &Connection,
    routines: &[database::Routine],
) -> Result<()> {
    let theme = ColorfulTheme::default();

    let routine_names: Vec<String> = routines.iter().map(|r| r.name.clone()).collect();

    let routine_idx = Select::with_theme(&theme)
        .with_prompt("Which routine to add a step to?")
        .items(&routine_names)
        .interact()?;

    let routine = &routines[routine_idx];

    // Get available actions
    let actions = database::fetch_actions(conn)?;

    if actions.is_empty() {
        println!("\n⚠️  No actions available. Create some actions first!");
        return Ok(());
    }

    let action_names: Vec<String> = actions.iter().map(|a| a.title.clone()).collect();

    let action_idx = Select::with_theme(&theme)
        .with_prompt("Which action to add as a step?")
        .items(&action_names)
        .interact()?;

    let action = &actions[action_idx];

    // Get current step count
    let next_order = database::next_routine_step_order(conn, &routine.id)?;

    let step = database::RoutineStep::new(&routine.id, &action.id, next_order);
    database::insert_routine_step(conn, &step)?;

    println!(
        "\n✅ Added '{}' as step {} in '{}'",
        action.title, next_order, routine.name
    );

    Ok(())
}

/// Delete a routine
fn delete_routine_interactive(conn: &Connection, routines: &[database::Routine]) -> Result<()> {
    let theme = ColorfulTheme::default();

    let routine_names: Vec<String> = routines.iter().map(|r| r.name.clone()).collect();

    let routine_idx = Select::with_theme(&theme)
        .with_prompt("Which routine to delete?")
        .items(&routine_names)
        .interact()?;

    let routine = &routines[routine_idx];
    let step_count = database::count_routine_steps(conn, &routine.id)?;

    let confirm_msg = if step_count > 0 {
        format!(
            "Delete '{}' and its {} steps? This cannot be undone.",
            routine.name, step_count
        )
    } else {
        format!("Delete '{}'? This cannot be undone.", routine.name)
    };

    let confirmed = Confirm::with_theme(&theme)
        .with_prompt(&confirm_msg)
        .default(false)
        .interact()?;

    if confirmed {
        database::delete_routine(conn, &routine.id)?;
        println!("\n🗑️  Deleted routine: {}", routine.name);
    } else {
        println!("\n❌ Deletion cancelled.");
    }

    Ok(())
}

/// Quickly capture multiple actions in a row
fn batch_capture_actions(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n📋 Batch Action Capture\n");
    println!(
        "Quickly add multiple tasks. Enter titles, they'll be created and added to your pipeline."
    );
    println!("Press Enter with an empty title when done.\n");

    let mut count = 0;

    loop {
        let title: String = Input::with_theme(&theme)
            .with_prompt(format!("Task #{} (or press Enter to finish)", count + 1))
            .allow_empty(true)
            .interact_text()?;

        if title.trim().is_empty() {
            break;
        }

        let trimmed = title.trim();
        let action = database::Action::new("task", trimmed);
        let action_id = action.id.clone();
        database::insert_action(conn, &action)?;

        // Auto-create instance and enqueue
        let mut instance = database::Instance::new(&action_id);
        instance.source = Some("manual".to_string());
        let instance_id = instance.id.clone();
        database::insert_instance(conn, &instance)?;
        database::enqueue_instance(conn, &instance_id, Some(trimmed))?;

        count += 1;
        println!("  ✓ Added '{}' to pipeline", trimmed);
    }

    println!("\n✅ Created and enqueued {} task(s)", count);

    Ok(())
}

/// Browse and explore existing actions
fn explore_actions_interactive(conn: &Connection) -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n🔍 Explore Actions\n");

    let actions = database::fetch_actions(conn)?;

    if actions.is_empty() {
        println!("No actions found.");
        return Ok(());
    }

    loop {
        let action_names: Vec<String> = actions
            .iter()
            .map(|a| format!("{} ({})", a.title, a.action_type))
            .collect();

        let mut items = action_names.clone();
        items.push("🔙 Back to main menu".to_string());

        let selection = Select::with_theme(&theme)
            .with_prompt(format!(
                "Actions ({}) - select to view details",
                actions.len()
            ))
            .items(&items)
            .interact()?;

        if selection == items.len() - 1 {
            break;
        }

        let action = &actions[selection];

        println!("\n📋 Action Details:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("  Title:       {}", action.title);
        println!("  Type:        {}", action.action_type);
        if let Some(ref desc) = action.description {
            println!("  Description: {}", desc);
        }
        if let Some(d) = action.duration_bucket {
            println!("  Duration:    {} minutes", d);
        }
        if let Some(e) = action.energy_rate {
            println!(
                "  Energy:      {} ({})",
                e,
                if e > 0 {
                    "energizing"
                } else if e < 0 {
                    "draining"
                } else {
                    "neutral"
                }
            );
        }
        if let Some(a) = action.attention_level {
            println!("  Attention:   {}/5", a);
        }
        if let Some(t) = action.transition_difficulty {
            println!("  Transition:  {}/5 difficulty", t);
        }
        if let Some(e) = action.enjoyment_after_start {
            println!("  Enjoyment:   {}", e);
        }
        if let Some(i) = action.importance {
            println!("  Importance:  {}/5", i);
        }
        if let Some(u) = action.urgency_growth {
            println!(
                "  Urgency:     {}",
                if u { "grows over time" } else { "static" }
            );
        }
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        let action_choices = vec!["📋 Add to pipeline", "🗑️  Delete", "🔙 Back to list"];

        let choice = Select::with_theme(&theme)
            .with_prompt("What would you like to do?")
            .items(&action_choices)
            .interact()?;

        match choice {
            0 => {
                let mut instance = database::Instance::new(&action.id);
                instance.source = Some("manual".to_string());
                let instance_id = instance.id.clone();
                database::insert_instance(conn, &instance)?;
                database::enqueue_instance(conn, &instance_id, Some(&action.title))?;
                println!("\n✅ Added to pipeline!");
            }
            1 => {
                let confirm = Confirm::with_theme(&theme)
                    .with_prompt(format!(
                        "Are you sure you want to delete '{}'?",
                        action.title
                    ))
                    .default(false)
                    .interact()?;

                if confirm {
                    database::delete_action(conn, &action.id)?;
                    println!("\n🗑️  Deleted action '{}'", action.title);
                    break;
                }
            }
            2 => continue,
            _ => unreachable!(),
        }

        println!();
    }

    Ok(())
}
