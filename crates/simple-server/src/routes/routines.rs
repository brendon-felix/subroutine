use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use uuid::Uuid;

use simple_core::{Action, ActionState, ActionTarget, ChangeEvent, Routine, pipeline};

use crate::{db, error::Result, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/routines", get(list_routines))
        .route("/routines", post(create_routine))
        .route("/routines/{id}", get(get_routine))
        .route("/routines/{id}", put(upsert_routine))
        .route("/routines/{id}", delete(trash_routine))
        .route("/routines/{id}/instantiate", post(instantiate_routine))
}

/// GET /routines — list all non-deleted routines
async fn list_routines(State(state): State<AppState>) -> Result<Json<Vec<Routine>>> {
    let routines = db::routines::fetch_all(&state.pool).await?;
    Ok(Json(routines))
}

/// POST /routines — create a new routine (server generates ID)
async fn create_routine(
    State(state): State<AppState>,
    Json(mut routine): Json<Routine>,
) -> Result<(StatusCode, Json<Routine>)> {
    // Generate a new ID so this is always an insert, not an update
    routine.id = Uuid::now_v7();
    db::routines::upsert(&state.pool, &routine).await?;
    let _ = state.changes.send(ChangeEvent::RoutinesChanged);
    Ok((StatusCode::CREATED, Json(routine)))
}

/// GET /routines/{id} — fetch a single routine
async fn get_routine(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Routine>> {
    let routine = db::routines::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| crate::error::AppError::not_found(format!("routine {id} not found")))?;
    Ok(Json(routine))
}

/// PUT /routines/{id} — upsert a routine
async fn upsert_routine(
    State(state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(routine): Json<Routine>,
) -> Result<Json<Routine>> {
    db::routines::upsert(&state.pool, &routine).await?;
    let _ = state.changes.send(ChangeEvent::RoutinesChanged);
    Ok(Json(routine))
}

/// DELETE /routines/{id} — soft-delete a routine
async fn trash_routine(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode> {
    let existed = db::routines::soft_delete(&state.pool, id).await?;
    if !existed {
        return Err(crate::error::AppError::not_found(format!(
            "routine {id} not found"
        )));
    }
    let _ = state.changes.send(ChangeEvent::RoutinesChanged);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InstantiateRequest {
    start_time: Option<DateTime<Utc>>,
}

/// POST /routines/{id}/instantiate — create queued actions from a routine's steps
async fn instantiate_routine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<InstantiateRequest>,
) -> Result<Json<Vec<Action>>> {
    let routine: Routine = db::routines::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| crate::error::AppError::not_found(format!("routine {id} not found")))?;

    let start = pipeline::quantize(body.start_time.unwrap_or_else(Utc::now));
    let default_duration = Duration::minutes(15);
    let mut cursor = start;
    let mut actions: Vec<Action> = Vec::new();

    for step in &routine.steps {
        let duration = pipeline::quantize_duration(step.duration.unwrap_or(default_duration));
        let action = Action::new(step.title.clone())
            .with_origin_routine(routine.id)
            .with_duration(duration)
            .with_state(ActionState::Scheduled(ActionTarget {
                time: cursor,
                is_static: true,
            }));
        cursor += duration;
        actions.push(action);
    }

    for action in &actions {
        db::actions::upsert(&state.pool, action).await?;
    }

    let _ = state.changes.send(ChangeEvent::PipelineChanged);
    Ok(Json(actions))
}
