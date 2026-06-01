use axum::{
    Router,
    extract::State,
    response::{Sse, sse::Event},
    routing::get,
};
use futures_util::StreamExt;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/changes/stream", get(changes_stream))
}

/// Long-lived SSE endpoint.
///
/// Each connected client receives a `ChangeEvent` JSON object whenever any
/// mutation is committed to the database. The client should treat each event
/// as a signal to re-fetch the relevant resource — the event carries no
/// payload beyond its type tag.
///
/// Example event on the wire:
/// ```
/// data: {"type":"actions_changed"}
/// ```
async fn changes_stream(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    // Subscribe before we do anything else so we don't miss events that fire
    // between subscribe() and the first poll.
    let rx = state.changes.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(event) => {
                // Serialize the ChangeEvent to JSON and wrap it in an SSE frame.
                match serde_json::to_string(&event) {
                    Ok(json) => Some(Ok(Event::default().data(json))),
                    Err(e) => {
                        tracing::error!("failed to serialize ChangeEvent: {e}");
                        None
                    }
                }
            }
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                // The client was too slow and missed n events. That's okay —
                // it will re-sync on its next re-fetch. Log and skip.
                tracing::warn!("SSE client lagged, dropped {n} change events");
                None
            }
        }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
