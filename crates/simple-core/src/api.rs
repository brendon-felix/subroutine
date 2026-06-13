//! Shared API response types. These are simple compositions of core types
//! that the server and clients use to agree on HTTP response shapes.

use serde::{Deserialize, Serialize};

use crate::{Action, ActionTemplate, Event, EventTemplate, Routine};

pub(crate) mod duration_nanos {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        d.num_nanoseconds().map_or_else(
            || {
                Err(serde::ser::Error::custom(format!(
                    "duration out of range: {d}"
                )))
            },
            |n| s.serialize_i64(n),
        )
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::nanoseconds(i64::deserialize(d)?))
    }
}

pub(crate) mod duration_nanos_opt {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer, Serializer};

    use super::duration_nanos;

    pub fn serialize<S: Serializer>(d: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
        match d {
            Some(d) => duration_nanos::serialize(d, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Duration>, D::Error> {
        Ok(Option::<i64>::deserialize(d)?.map(Duration::nanoseconds))
    }
}

/// Response body for `GET /api/data`.
#[derive(Debug, Serialize, Deserialize)]
pub struct AllData {
    pub actions: Vec<Action>,
    pub events: Vec<Event>,
    pub routines: Vec<Routine>,
    pub action_templates: Vec<ActionTemplate>,
    pub event_templates: Vec<EventTemplate>,
}

/// Response body for `POST /api/actions/{id}/complete`.
#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteResult {
    pub completed: Action,
    pub next: Option<Action>,
}

/// A lightweight notification broadcast to all connected SSE clients whenever
/// server-side data changes. Clients use this as a signal to re-fetch the
/// relevant resource — the event carries no payload of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChangeEvent {
    ActionsChanged,
    RoutinesChanged,
    EventsChanged,
    ActionTemplatesChanged,
    EventTemplatesChanged,
    PipelineChanged,
}
