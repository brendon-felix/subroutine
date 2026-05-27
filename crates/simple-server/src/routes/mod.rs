use axum::Router;

use crate::state::AppState;

mod actions;
mod data;
mod events;
mod pipeline;
mod routines;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(data::router())
        .merge(actions::router())
        .merge(events::router())
        .merge(routines::router())
        .merge(pipeline::router())
}
