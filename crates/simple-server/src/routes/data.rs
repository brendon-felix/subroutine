use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use simple_core::{Action, Event, Routine};

use crate::{db, error::Result, state::AppState};

#[derive(Serialize)]
pub struct AllData {
    pub actions: Vec<Action>,
    pub events: Vec<Event>,
    pub routines: Vec<Routine>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/data", get(get_all_data))
}

async fn get_all_data(State(state): State<AppState>) -> Result<Json<AllData>> {
    let (actions, events, routines) = tokio::try_join!(
        db::actions::fetch_all(&state.pool),
        db::events::fetch_all(&state.pool),
        db::routines::fetch_all(&state.pool),
    )?;
    Ok(Json(AllData {
        actions,
        events,
        routines,
    }))
}
