use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, put},
};
use uuid::Uuid;

use simple_core::Event;

use crate::{db, error::Result, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/events/{id}", put(upsert_event))
        .route("/events/{id}", delete(trash_event))
}

async fn upsert_event(
    State(state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(event): Json<Event>,
) -> Result<Json<Event>> {
    db::events::upsert(&state.pool, &event).await?;
    Ok(Json(event))
}

async fn trash_event(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode> {
    db::events::soft_delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
