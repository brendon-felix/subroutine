use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, post, put},
};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use simple_core::{Action, ActionState, DEFAULT_ACTION_DURATION};

use crate::{db, error::Result, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/actions/{id}", put(upsert_action))
        .route("/actions/{id}", delete(trash_action))
        .route("/actions/{id}/queue", post(queue_action))
        .route("/actions/{id}/backlog", post(backlog_action))
        .route("/actions/{id}/complete", post(complete_action))
}

async fn upsert_action(
    State(state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(action): Json<Action>,
) -> Result<Json<Action>> {
    db::actions::upsert(&state.pool, &action).await?;
    Ok(Json(action))
}

async fn trash_action(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode> {
    db::actions::soft_delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn queue_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Action>>> {
    let now = Utc::now();
    let mut all_actions: Vec<Action> = db::actions::fetch_all(&state.pool).await?;
    let events = db::events::fetch_all(&state.pool).await?;

    // Find the end of the current floating queue.
    let chain_end = all_actions
        .iter()
        .filter_map(|a| {
            if let ActionState::Queued(t) = a.state {
                if !t.is_static {
                    return Some(t.time + a.duration.unwrap_or(DEFAULT_ACTION_DURATION));
                }
            }
            None
        })
        .max()
        .unwrap_or(now);

    let slot = chain_end.max(now);

    // Queue the target action.
    let action = all_actions
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or_else(|| anyhow::anyhow!("action {id} not found"))?;

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

    Ok(Json(changed))
}

async fn backlog_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Action>> {
    let mut action: Action = db::actions::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("action {id} not found"))?;

    action.backlog(None);
    db::actions::upsert(&state.pool, &action).await?;
    Ok(Json(action))
}

#[derive(Serialize)]
pub struct CompleteResult {
    pub completed: Action,
    pub next: Option<Action>,
}

async fn complete_action(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CompleteResult>> {
    let now = Utc::now();
    let mut action: Action = db::actions::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("action {id} not found"))?;

    let next = action.next_occurence();
    action.complete(now);

    db::actions::upsert(&state.pool, &action).await?;
    if let Some(ref next_action) = next {
        db::actions::upsert(&state.pool, next_action).await?;
    }

    Ok(Json(CompleteResult {
        completed: action,
        next,
    }))
}
