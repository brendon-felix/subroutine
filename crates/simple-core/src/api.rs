//! Shared API response types. These are simple compositions of core types
//! that the server and clients use to agree on HTTP response shapes.

use serde::{Deserialize, Serialize};

use crate::{Action, Event, Routine};

/// Response body for `GET /api/data`.
#[derive(Debug, Serialize, Deserialize)]
pub struct AllData {
    pub actions: Vec<Action>,
    pub events: Vec<Event>,
    pub routines: Vec<Routine>,
}

/// Response body for `POST /api/actions/{id}/complete`.
#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteResult {
    pub completed: Action,
    pub next: Option<Action>,
}
