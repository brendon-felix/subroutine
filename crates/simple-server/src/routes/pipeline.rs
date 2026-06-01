use axum::{Json, Router, extract::State, routing::post};
use chrono::Utc;

use simple_core::ChangeEvent;

use crate::{db, error::Result, state::AppState};

use super::dto::ActionDto;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/pipeline/refresh", post(refresh_pipeline))
        .route("/pipeline/expedite", post(expedite_pipeline))
}

async fn refresh_pipeline(State(state): State<AppState>) -> Result<Json<Vec<ActionDto>>> {
    let now = Utc::now();
    let actions = db::actions::fetch_all(&state.pool).await?;
    let events = db::events::fetch_all(&state.pool).await?;

    let updated = simple_core::requeue_actions(&actions, &events, now);
    for action in &updated {
        db::actions::upsert(&state.pool, action).await?;
    }

    let _ = state.changes.send(ChangeEvent::PipelineChanged);
    Ok(Json(updated.into_iter().map(ActionDto::from).collect()))
}

async fn expedite_pipeline(State(state): State<AppState>) -> Result<Json<Vec<ActionDto>>> {
    let now = Utc::now();
    let horizon = now + simple_core::EXPEDITE_HORIZON;
    let actions = db::actions::fetch_all(&state.pool).await?;
    let events = db::events::fetch_all(&state.pool).await?;

    let updated = simple_core::expedite_actions(&actions, &events, now, horizon);
    for action in &updated {
        db::actions::upsert(&state.pool, action).await?;
    }

    let _ = state.changes.send(ChangeEvent::PipelineChanged);
    Ok(Json(updated.into_iter().map(ActionDto::from).collect()))
}
