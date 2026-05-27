use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, post, put},
};
use chrono::{Duration, Utc};
use uuid::Uuid;

use simple_core::{Action, ActionState, ActionTarget, Routine};

use crate::{db, error::Result, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/routines/{id}", put(upsert_routine))
        .route("/routines/{id}", delete(trash_routine))
        .route("/routines/{id}/instantiate", post(instantiate_routine))
}

async fn upsert_routine(
    State(state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(routine): Json<Routine>,
) -> Result<Json<Routine>> {
    db::routines::upsert(&state.pool, &routine).await?;
    Ok(Json(routine))
}

async fn trash_routine(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode> {
    db::routines::soft_delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn instantiate_routine(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Action>>> {
    let routine: Routine = db::routines::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("routine {id} not found"))?;

    let now = Utc::now();
    let default_duration = Duration::minutes(15);
    let mut cursor = now;
    let mut actions: Vec<Action> = Vec::new();

    for step in &routine.steps {
        let duration = step.duration.unwrap_or(default_duration);
        let action = Action::new(step.title.clone())
            .with_origin_routine(routine.id)
            .with_duration(duration)
            .with_state(ActionState::Queued(ActionTarget {
                time: cursor,
                is_static: true,
            }));
        cursor = cursor + duration;
        actions.push(action);
    }

    for action in &actions {
        db::actions::upsert(&state.pool, action).await?;
    }

    Ok(Json(actions))
}
