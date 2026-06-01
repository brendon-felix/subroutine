use axum::Router;

use crate::state::AppState;

mod actions;
mod data;
pub(super) mod dto;
mod events;
mod pipeline;
mod routines;
mod sse;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(data::router())
        .merge(actions::router())
        .merge(events::router())
        .merge(routines::router())
        .merge(pipeline::router())
        .merge(sse::router())
}
