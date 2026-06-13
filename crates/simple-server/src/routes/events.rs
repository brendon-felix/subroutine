use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
};
use uuid::Uuid;

use simple_core::{ChangeEvent, Event, EventTemplate};

use crate::{db, error::Result, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/events", get(list_events))
        .route("/events", post(create_event))
        .route("/events/{id}", get(get_event))
        .route("/events/{id}", put(upsert_event))
        .route("/events/{id}", delete(trash_event))
        .route("/events/{id}/save", post(save_event))
        .route("/events/templates", get(list_event_templates))
        .route("/events/templates", post(create_event_template))
        .route("/events/templates/{id}", get(get_event_template))
        .route("/events/templates/{id}", put(upsert_event_template))
        .route("/events/templates/{id}", delete(trash_event_template))
}

/// GET /events — list all non-deleted events
async fn list_events(State(state): State<AppState>) -> Result<Json<Vec<Event>>> {
    let events = db::events::fetch_all(&state.pool).await?;
    Ok(Json(events))
}

/// POST /events — create a new event
async fn create_event(
    State(state): State<AppState>,
    Json(mut event): Json<Event>,
) -> Result<(StatusCode, Json<Event>)> {
    // If the client sent a nil UUID, generate one
    if event.id == Uuid::nil() {
        event = Event::new(event.title.clone(), event.time, event.duration);
    }
    db::events::upsert(&state.pool, &event).await?;
    let _ = state.changes.send(ChangeEvent::EventsChanged);
    Ok((StatusCode::CREATED, Json(event)))
}

/// GET /events/{id} — fetch a single event
async fn get_event(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Event>> {
    let event = db::events::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| crate::error::AppError::not_found(format!("event {id} not found")))?;
    Ok(Json(event))
}

/// PUT /events/{id} — upsert an event
async fn upsert_event(
    State(state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(event): Json<Event>,
) -> Result<Json<Event>> {
    db::events::upsert(&state.pool, &event).await?;
    let _ = state.changes.send(ChangeEvent::EventsChanged);
    Ok(Json(event))
}

/// DELETE /events/{id} — soft-delete an event
async fn trash_event(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<StatusCode> {
    let existed = db::events::soft_delete(&state.pool, id).await?;
    if !existed {
        return Err(crate::error::AppError::not_found(format!(
            "event {id} not found"
        )));
    }
    let _ = state.changes.send(ChangeEvent::EventsChanged);
    Ok(StatusCode::NO_CONTENT)
}

/// POST /events/{id}/save — create template from event
async fn save_event(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Json<Event>> {
    let event: Event = db::events::fetch_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| crate::error::AppError::not_found(format!("event {id} not found")))?;

    let template: EventTemplate = event.clone().into_template();
    db::events::upsert_event_template(&state.pool, &template).await?;
    let _ = state.changes.send(ChangeEvent::EventsChanged);
    let _ = state.changes.send(ChangeEvent::EventTemplatesChanged);
    Ok(Json(event))
}

// ── Event template routes ─────────────────────────────────────────────────────

/// GET /events/templates — list all non-deleted event templates
async fn list_event_templates(State(state): State<AppState>) -> Result<Json<Vec<EventTemplate>>> {
    let templates = db::events::fetch_all_event_templates(&state.pool).await?;
    Ok(Json(templates))
}

/// POST /events/templates — create a new event template
async fn create_event_template(
    State(state): State<AppState>,
    Json(template): Json<EventTemplate>,
) -> Result<(StatusCode, Json<EventTemplate>)> {
    db::events::upsert_event_template(&state.pool, &template).await?;
    let _ = state.changes.send(ChangeEvent::EventTemplatesChanged);
    Ok((StatusCode::CREATED, Json(template)))
}

/// GET /events/templates/{id} — fetch a single event template
async fn get_event_template(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<EventTemplate>> {
    let template = db::events::fetch_event_template_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| {
            crate::error::AppError::not_found(format!("event template {id} not found"))
        })?;
    Ok(Json(template))
}

/// PUT /events/templates/{id} — upsert an event template (sync-friendly)
async fn upsert_event_template(
    State(state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(template): Json<EventTemplate>,
) -> Result<Json<EventTemplate>> {
    db::events::upsert_event_template(&state.pool, &template).await?;
    let _ = state.changes.send(ChangeEvent::EventTemplatesChanged);
    Ok(Json(template))
}

/// DELETE /events/templates/{id} — soft-delete an event template
async fn trash_event_template(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let existed = db::events::soft_delete_event_template(&state.pool, id).await?;
    if !existed {
        return Err(crate::error::AppError::not_found(format!(
            "event template {id} not found"
        )));
    }
    let _ = state.changes.send(ChangeEvent::EventTemplatesChanged);
    Ok(StatusCode::NO_CONTENT)
}
