use axum::{Json, Router, extract::State, routing::post};
use chrono::Utc;

use simple_core::{Action, EXPEDITE_HORIZON};

use crate::{db, error::Result, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/pipeline/refresh", post(refresh_pipeline))
        .route("/pipeline/expedite", post(expedite_pipeline))
}

async fn refresh_pipeline(State(state): State<AppState>) -> Result<Json<Vec<Action>>> {
    let now = Utc::now();
    let actions = db::actions::fetch_all(&state.pool).await?;
    let events = db::events::fetch_all(&state.pool).await?;

    let updated = simple_core::requeue_actions(&actions, &events, now);
    for action in &updated {
        db::actions::upsert(&state.pool, action).await?;
    }

    Ok(Json(updated))
}

async fn expedite_pipeline(State(state): State<AppState>) -> Result<Json<Vec<Action>>> {
    let now = Utc::now();
    let horizon = now + EXPEDITE_HORIZON;
    let actions = db::actions::fetch_all(&state.pool).await?;
    let events = db::events::fetch_all(&state.pool).await?;

    let updated = simple_core::expedite_actions(&actions, &events, now, horizon);
    for action in &updated {
        db::actions::upsert(&state.pool, action).await?;
    }

    Ok(Json(updated))
}
