use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
};
use chrono::Utc;
use uuid::Uuid;

use simple_core::{Action, ActionState, ChangeEvent, DEFAULT_ACTION_DURATION, quantize};

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
        .route("/actions/{id}/complete", post(complete_action))
        .route("/actions/{id}/clear_duration", post(clear_action_duration))
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
    let now = quantize(Utc::now());
    let mut all_actions: Vec<Action> = db::actions::fetch_all(&state.pool).await?;
    let events = db::events::fetch_all(&state.pool).await?;

    // Find the end of the current floating queue.
    let chain_end = all_actions
        .iter()
        .filter_map(|a| {
            if let ActionState::Queued(t) = a.state
                && !t.is_static
            {
                return Some(t.time + a.duration.unwrap_or(DEFAULT_ACTION_DURATION));
            }
            None
        })
        .max()
        .unwrap_or(now);

    let slot = quantize(chain_end.max(now));

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
    let rescheduled = simple_core::requeue_actions(&all_actions, &events, now);
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

/// POST /actions/{id}/complete — mark an action complete and compute next occurrence
async fn complete_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CompleteResult>> {
    let now = quantize(Utc::now());
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
