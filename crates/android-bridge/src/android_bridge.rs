use anyhow::Result;
use app_core::{PipelineEntry, SavedAction};
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jstring};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Serializable mirror types for crossing the JNI boundary
// ---------------------------------------------------------------------------

/// Flattened, serializable representation of a pipeline action sent to Kotlin.
/// Uses the same field names as the app-core Action struct so serde produces
/// a consistent shape regardless of which layer reads it.
#[derive(Serialize)]
struct ActionPayload {
    id: String,
    title: String,
    content: Option<String>,
    created_at: String,
    target_time: Option<String>,
    ephemeral: bool,
    saved_action_id: Option<String>,
}

/// A pipeline entry as seen by Kotlin. Only Action entries are supported in the
/// POC — Routine, Subroutine, and Event entries are included as opaque stubs so
/// the pipeline round-trip does not lose data.
#[derive(Serialize)]
struct PipelineEntryPayload {
    id: String,
    entry_type: String, // "action" | "routine" | "subroutine" | "event"
    title: String,
    /// Populated only when entry_type == "action"
    action: Option<ActionPayload>,
}

#[derive(Serialize)]
struct PipelinePayload {
    backlog: Vec<PipelineEntryPayload>,
    queue: Vec<PipelineEntryPayload>,
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn entry_to_payload(entry: &PipelineEntry) -> PipelineEntryPayload {
    let id = entry.id().to_string();
    let title = entry.title().to_string();

    match entry {
        PipelineEntry::Action(action) => PipelineEntryPayload {
            id,
            entry_type: "action".to_string(),
            title,
            action: Some(ActionPayload {
                id: action.id.to_string(),
                title: action.title.clone(),
                content: action.content.clone(),
                created_at: action.created_at.to_rfc3339(),
                target_time: action.target_time.map(|t| t.to_rfc3339()),
                ephemeral: action.ephemeral,
                saved_action_id: action.saved_action_id.map(|id| id.to_string()),
            }),
        },
        PipelineEntry::Routine(_) => PipelineEntryPayload {
            id,
            entry_type: "routine".to_string(),
            title,
            action: None,
        },
        PipelineEntry::Subroutine(_) => PipelineEntryPayload {
            id,
            entry_type: "subroutine".to_string(),
            title,
            action: None,
        },
        PipelineEntry::Event(_) => PipelineEntryPayload {
            id,
            entry_type: "event".to_string(),
            title,
            action: None,
        },
        PipelineEntry::Transition(_) => PipelineEntryPayload {
            id,
            entry_type: "transition".to_string(),
            title,
            action: None,
        },
    }
}

// ---------------------------------------------------------------------------
// Implementation functions (no JNI types — easy to test and reason about)
// ---------------------------------------------------------------------------

fn fetch_saved_actions_impl(db_path: &str) -> Result<String> {
    let conn = database::connect_and_migrate_at(db_path)?;
    let actions = database::fetch_saved_actions(&conn.lock().unwrap())?;
    Ok(serde_json::to_string(&actions)?)
}

fn insert_saved_action_impl(db_path: &str, title: String, content: Option<String>) -> Result<()> {
    let conn = database::connect_and_migrate_at(db_path)?;
    let mut action = SavedAction::new(title);
    if let Some(content) = content {
        action = action.with_content(content);
    }
    database::insert_saved_action(&conn.lock().unwrap(), &action)?;
    Ok(())
}

fn delete_saved_action_impl(db_path: &str, id: &str) -> Result<()> {
    let uuid = uuid::Uuid::parse_str(id)?;
    let conn = database::connect_and_migrate_at(db_path)?;
    database::delete_saved_action(&conn.lock().unwrap(), uuid)?;
    Ok(())
}

/// Instantiates a SavedAction into a concrete Action, inserts it into the
/// actions table, adds it to the pipeline backlog, and persists the pipeline.
fn instantiate_saved_action_impl(db_path: &str, saved_action_id: &str) -> Result<String> {
    let uuid = uuid::Uuid::parse_str(saved_action_id)?;
    let conn = database::connect_and_migrate_at(db_path)?;
    let guard = conn.lock().unwrap();

    let saved = database::fetch_saved_action_by_id(&guard, uuid)?
        .ok_or_else(|| anyhow::anyhow!("SavedAction '{}' not found", saved_action_id))?;

    let action = saved.instantiate();
    let action_id = action.id.to_string();
    database::insert_action(&guard, &action)?;

    let mut pipeline = database::load_pipeline(&guard)?;
    pipeline.push(PipelineEntry::Action(action))?;
    database::save_pipeline(&guard, &pipeline)?;

    Ok(action_id)
}

fn load_pipeline_impl(db_path: &str) -> Result<String> {
    let conn = database::connect_and_migrate_at(db_path)?;
    let guard = conn.lock().unwrap();
    let pipeline = database::load_pipeline(&guard)?;

    let payload = PipelinePayload {
        backlog: pipeline.backlog().iter().map(entry_to_payload).collect(),
        queue: pipeline.queue().iter().map(entry_to_payload).collect(),
    };

    Ok(serde_json::to_string(&payload)?)
}

/// Promotes an entry from the pipeline backlog into the queue and persists.
fn promote_pipeline_entry_impl(db_path: &str, entry_id: &str) -> Result<()> {
    let uuid = uuid::Uuid::parse_str(entry_id)?;
    let conn = database::connect_and_migrate_at(db_path)?;
    let guard = conn.lock().unwrap();

    let mut pipeline = database::load_pipeline(&guard)?;
    pipeline.promote(uuid)?;
    database::save_pipeline(&guard, &pipeline)?;
    Ok(())
}

/// Demotes an entry from the pipeline queue back into the backlog and persists.
fn demote_pipeline_entry_impl(db_path: &str, entry_id: &str) -> Result<()> {
    let uuid = uuid::Uuid::parse_str(entry_id)?;
    let conn = database::connect_and_migrate_at(db_path)?;
    let guard = conn.lock().unwrap();

    let mut pipeline = database::load_pipeline(&guard)?;
    pipeline.demote(uuid)?;
    database::save_pipeline(&guard, &pipeline)?;
    Ok(())
}

/// Removes an action from the pipeline entirely and deletes it from the actions
/// table. Accepts an action ID (not a saved_action_id).
fn delete_pipeline_action_impl(db_path: &str, action_id: &str) -> Result<()> {
    let uuid = uuid::Uuid::parse_str(action_id)?;
    let conn = database::connect_and_migrate_at(db_path)?;
    let guard = conn.lock().unwrap();

    // Remove from pipeline first so save_pipeline does not re-reference the row.
    let mut pipeline = database::load_pipeline(&guard)?;
    let in_queue = pipeline.queue().iter().any(|e| e.id() == uuid);
    let in_backlog = pipeline.backlog().iter().any(|e| e.id() == uuid);

    if in_queue {
        pipeline.demote(uuid)?;
    }
    if in_queue || in_backlog {
        // After demotion it is guaranteed to be in the backlog; remove by
        // rebuilding without that entry. Pipeline has no direct remove method,
        // so we rebuild from the remaining entries.
        let remaining_backlog: Vec<PipelineEntry> = pipeline
            .backlog()
            .iter()
            .filter(|e| e.id() != uuid)
            .cloned()
            .collect();
        let remaining_queue: Vec<PipelineEntry> = pipeline
            .queue()
            .iter()
            .filter(|e| e.id() != uuid && !e.is_transition())
            .cloned()
            .collect();

        let mut new_pipeline = app_core::Pipeline::new();
        for entry in remaining_backlog {
            new_pipeline.push(entry)?;
        }
        for entry in remaining_queue {
            let id = entry.id();
            new_pipeline.push(entry)?;
            new_pipeline.promote(id)?;
        }
        database::save_pipeline(&guard, &new_pipeline)?;
    }

    database::delete_action(&guard, uuid)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// JNI exports
// ---------------------------------------------------------------------------

fn get_string(env: &mut JNIEnv, arg: &JString) -> Option<String> {
    env.get_string(arg).ok().map(|s| s.into())
}

fn return_json_or_throw(env: &mut JNIEnv, result: Result<String>, fallback: &str) -> jstring {
    match result {
        Ok(json) => env
            .new_string(json)
            .expect("Failed to create JNI string")
            .into_raw(),
        Err(err) => {
            let _ = env.throw_new("java/lang/RuntimeException", err.to_string());
            env.new_string(fallback)
                .expect("Failed to create fallback JNI string")
                .into_raw()
        }
    }
}

fn return_bool_or_throw(env: &mut JNIEnv, result: Result<()>) -> jboolean {
    match result {
        Ok(()) => 1,
        Err(err) => {
            let _ = env.throw_new("java/lang/RuntimeException", err.to_string());
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_RustBridge_fetchSavedActions(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
) -> jstring {
    let Some(db_path) = get_string(&mut env, &db_path) else {
        return env.new_string("[]").expect("alloc").into_raw();
    };
    return_json_or_throw(&mut env, fetch_saved_actions_impl(&db_path), "[]")
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_RustBridge_insertSavedAction(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
    title: JString,
    content: JString,
) -> jboolean {
    let (Some(db_path), Some(title), Some(content_str)) = (
        get_string(&mut env, &db_path),
        get_string(&mut env, &title),
        get_string(&mut env, &content),
    ) else {
        return 0;
    };
    let content = if content_str.is_empty() {
        None
    } else {
        Some(content_str)
    };
    return_bool_or_throw(&mut env, insert_saved_action_impl(&db_path, title, content))
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_RustBridge_deleteSavedAction(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
    id: JString,
) -> jboolean {
    let (Some(db_path), Some(id)) = (get_string(&mut env, &db_path), get_string(&mut env, &id))
    else {
        return 0;
    };
    return_bool_or_throw(&mut env, delete_saved_action_impl(&db_path, &id))
}

/// Instantiates a SavedAction into the pipeline backlog.
/// Returns the new Action's UUID string on success, or throws on failure.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_RustBridge_instantiateSavedAction(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
    saved_action_id: JString,
) -> jstring {
    let (Some(db_path), Some(saved_action_id)) = (
        get_string(&mut env, &db_path),
        get_string(&mut env, &saved_action_id),
    ) else {
        return env.new_string("").expect("alloc").into_raw();
    };
    return_json_or_throw(
        &mut env,
        instantiate_saved_action_impl(&db_path, &saved_action_id),
        "",
    )
}

/// Returns the full pipeline as a JSON object with "backlog" and "queue" arrays.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_RustBridge_loadPipeline(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
) -> jstring {
    let Some(db_path) = get_string(&mut env, &db_path) else {
        return env
            .new_string(r#"{"backlog":[],"queue":[]}"#)
            .expect("alloc")
            .into_raw();
    };
    return_json_or_throw(
        &mut env,
        load_pipeline_impl(&db_path),
        r#"{"backlog":[],"queue":[]}"#,
    )
}

/// Promotes a backlog entry into the queue by its entry ID.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_RustBridge_promotePipelineEntry(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
    entry_id: JString,
) -> jboolean {
    let (Some(db_path), Some(entry_id)) = (
        get_string(&mut env, &db_path),
        get_string(&mut env, &entry_id),
    ) else {
        return 0;
    };
    return_bool_or_throw(&mut env, promote_pipeline_entry_impl(&db_path, &entry_id))
}

/// Demotes a queue entry back into the backlog by its entry ID.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_RustBridge_demotePipelineEntry(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
    entry_id: JString,
) -> jboolean {
    let (Some(db_path), Some(entry_id)) = (
        get_string(&mut env, &db_path),
        get_string(&mut env, &entry_id),
    ) else {
        return 0;
    };
    return_bool_or_throw(&mut env, demote_pipeline_entry_impl(&db_path, &entry_id))
}

/// Removes an action from the pipeline and deletes it from the actions table.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_RustBridge_deletePipelineAction(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
    action_id: JString,
) -> jboolean {
    let (Some(db_path), Some(action_id)) = (
        get_string(&mut env, &db_path),
        get_string(&mut env, &action_id),
    ) else {
        return 0;
    };
    return_bool_or_throw(&mut env, delete_pipeline_action_impl(&db_path, &action_id))
}
