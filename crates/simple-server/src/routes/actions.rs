use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
};
use chrono::Utc;
use uuid::Uuid;

use simple_core::{Action, ActionState, ActionTemplate, ChangeEvent, pipeline};

use crate::{db, error::Result, state::AppState};

use simple_core::CompleteResult;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/actions", get(list_actions))
        .route("/actions", post(create_action))
        .route("/actions/{id}", get(get_action))
        .route("/actions/{id}", put(upsert_action))
        .route("/actions/{id}", delete(trash_action))
        .route("/actions/{id}/queue", post(queue_action))
        .route("/actions/{id}/backlog", post(backlog_action))
        .route("/actions/{id}/save", post(save_action))
        .route("/actions/{id}/complete", post(complete_action))
        .route("/actions/{id}/clear_duration", post(clear_action_duration))
        .route("/actions/templates", get(list_action_templates))
        .route("/actions/templates", post(create_action_template))
        .route("/actions/templates/{id}", get(get_action_template))
        .route("/actions/templates/{id}", put(upsert_action_template))
        .route("/actions/templates/{id}", delete(trash_action_template))
}

/// GET /actions — list all non-deleted actions
async fn list_actions(State(state): State<AppState>) -> Result<Json<Vec<Action>>> {
    let actions = db::actions::fetch_all(&state.pool).await?;
    Ok(Json(actions))
}

/// POST /actions — create a new action (server generates ID)
async fn create_action(
    State(state): State<AppState>,
    Json(mut action): Json<Action>,
) -> Result<(StatusCode, Json<Action>)> {
    // Ensure the action starts in a valid initial state
    if !action.is_backlogged() {
        action.backlog(None);
    }
    db::actions::upsert(&state.pool, &action).await?;
    let _ = state.changes.send(ChangeEvent::ActionsChanged);
    Ok((StatusCode::CREATED, Json(action)))
}

/// GET /actions/{id} — fetch a single action
async fn get_action(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Action>> {
    let action = db::actions::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| crate::error::AppError::not_found(format!("action {id} not found")))?;
    Ok(Json(action))
}

/// PUT /actions/{id} — upsert an action (sync-friendly, ID from body)
async fn upsert_action(
    State(state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(action): Json<Action>,
) -> Result<Json<Action>> {
    db::actions::upsert(&state.pool, &action).await?;
    let _ = state.changes.send(ChangeEvent::ActionsChanged);
    Ok(Json(action))
}

/// DELETE /actions/{id} — soft-delete an action
async fn trash_action(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode> {
    let existed = db::actions::soft_delete(&state.pool, id).await?;
    if !existed {
        return Err(crate::error::AppError::not_found(format!(
            "action {id} not found"
        )));
    }
    let _ = state.changes.send(ChangeEvent::ActionsChanged);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /actions/{id}/queue — move a backlogged action into the queue
async fn queue_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Action>>> {
    let now = pipeline::quantize(Utc::now());
    let mut all_actions: Vec<Action> = db::actions::fetch_all(&state.pool).await?;
    let events = db::events::fetch_all(&state.pool).await?;

    // Find the end of the current floating queue.
    let chain_end = all_actions
        .iter()
        .filter_map(|a| {
            if let ActionState::Scheduled(t) = a.state
                && !t.is_static
            {
                return Some(t.time + a.duration.unwrap_or(pipeline::DEFAULT_ACTION_DURATION));
            }
            None
        })
        .max()
        .unwrap_or(now);

    let slot = pipeline::quantize(chain_end.max(now));

    // Queue the target action.
    let action = all_actions
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or_else(|| crate::error::AppError::not_found(format!("action {id} not found")))?;

    if !action.is_backlogged() {
        return Err(anyhow::anyhow!("action {id} is not in the backlog").into());
    }

    action.queue(slot);
    let queued = action.clone();
    db::actions::upsert(&state.pool, &queued).await?;

    // Refresh pipeline to fix any ordering issues.
    let rescheduled = pipeline::requeue_actions(&all_actions, &events, now);
    for a in &rescheduled {
        db::actions::upsert(&state.pool, a).await?;
    }

    // Return all modified actions (the queued action + pipeline changes, deduped).
    let mut changed: Vec<Action> = rescheduled;
    if !changed.iter().any(|a| a.id == queued.id) {
        changed.push(queued);
    }

    let _ = state.changes.send(ChangeEvent::PipelineChanged);
    Ok(Json(changed))
}

/// POST /actions/{id}/backlog — return an action to the backlog
async fn backlog_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Action>> {
    let mut action: Action = db::actions::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| crate::error::AppError::not_found(format!("action {id} not found")))?;

    action.backlog(None);
    db::actions::upsert(&state.pool, &action).await?;
    let _ = state.changes.send(ChangeEvent::ActionsChanged);
    Ok(Json(action))
}

/// POST /actions/{id}/save — create template from action
async fn save_action(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Action>> {
    let action: Action = db::actions::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| crate::error::AppError::not_found(format!("action {id} not found")))?;

    let template = action.clone().into_template();
    db::actions::upsert_template(&state.pool, &template).await?;
    let _ = state.changes.send(ChangeEvent::ActionsChanged);
    let _ = state.changes.send(ChangeEvent::ActionTemplatesChanged);
    Ok(Json(action))
}

/// POST /actions/{id}/complete — mark an action complete and compute next occurrence
async fn complete_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CompleteResult>> {
    let now = pipeline::quantize(Utc::now());
    let mut action: Action = db::actions::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| crate::error::AppError::not_found(format!("action {id} not found")))?;

    let next = action.next_occurence();
    action.complete(now);

    db::actions::upsert(&state.pool, &action).await?;
    if let Some(ref next_action) = next {
        db::actions::upsert(&state.pool, next_action).await?;
    }

    let _ = state.changes.send(ChangeEvent::PipelineChanged);
    Ok(Json(CompleteResult {
        completed: action,
        next,
    }))
}

/// POST /actions/{id}/clear_duration — remove the explicit duration from an action
async fn clear_action_duration(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Action>> {
    let mut action: Action = db::actions::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| crate::error::AppError::not_found(format!("action {id} not found")))?;
    action.duration = None;
    db::actions::upsert(&state.pool, &action).await?;
    let _ = state.changes.send(ChangeEvent::ActionsChanged);
    Ok(Json(action))
}

// ── Action template routes ────────────────────────────────────────────────────

/// GET /actions/templates — list all non-deleted action templates
async fn list_action_templates(State(state): State<AppState>) -> Result<Json<Vec<ActionTemplate>>> {
    let templates = db::actions::fetch_all_templates(&state.pool).await?;
    Ok(Json(templates))
}

/// POST /actions/templates — create a new action template
async fn create_action_template(
    State(state): State<AppState>,
    Json(template): Json<ActionTemplate>,
) -> Result<(StatusCode, Json<ActionTemplate>)> {
    db::actions::upsert_template(&state.pool, &template).await?;
    let _ = state.changes.send(ChangeEvent::ActionTemplatesChanged);
    Ok((StatusCode::CREATED, Json(template)))
}

/// GET /actions/templates/{id} — fetch a single action template
async fn get_action_template(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ActionTemplate>> {
    let template = db::actions::fetch_template_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| {
            crate::error::AppError::not_found(format!("action template {id} not found"))
        })?;
    Ok(Json(template))
}

/// PUT /actions/templates/{id} — upsert an action template (sync-friendly)
async fn upsert_action_template(
    State(state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(template): Json<ActionTemplate>,
) -> Result<Json<ActionTemplate>> {
    db::actions::upsert_template(&state.pool, &template).await?;
    let _ = state.changes.send(ChangeEvent::ActionTemplatesChanged);
    Ok(Json(template))
}

/// DELETE /actions/templates/{id} — soft-delete an action template
async fn trash_action_template(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let existed = db::actions::soft_delete_template(&state.pool, id).await?;
    if !existed {
        return Err(crate::error::AppError::not_found(format!(
            "action template {id} not found"
        )));
    }
    let _ = state.changes.send(ChangeEvent::ActionTemplatesChanged);
    Ok(StatusCode::NO_CONTENT)
}
