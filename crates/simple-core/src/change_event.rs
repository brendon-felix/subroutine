use serde::{Deserialize, Serialize};

/// A lightweight notification broadcast to all connected SSE clients whenever
/// server-side data changes. Clients use this as a signal to re-fetch the
/// relevant resource — the event carries no payload of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChangeEvent {
    ActionsChanged,
    RoutinesChanged,
    EventsChanged,
    PipelineChanged,
}
