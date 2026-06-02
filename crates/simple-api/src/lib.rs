//! HTTP API types shared between `simple-server` and client crates.
//!
//! [`ActionDto`] and [`ActionStateDto`] use a flat, lowercase-tagged serde
//! representation that differs from the in-memory `simple_core` types:
//!
//! - `state` is a flat object with a `"type"` discriminant (not Rust's default
//!   external tagging)
//! - `duration` is expressed as `duration_secs: Option<i64>` instead of
//!   `chrono::Duration`, avoiding the `{"secs":…,"nanos":…}` shape

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use simple_core::{Action, ActionState, ActionTarget, Event, RecurrenceRule, Routine};

// ── ActionStateDto ────────────────────────────────────────────────────────────

/// HTTP wire representation of [`ActionState`].
///
/// Uses `#[serde(tag = "type", rename_all = "lowercase")]` so the wire format
/// is a flat object with a lowercase `"type"` discriminant:
///
/// ```json
/// {"type": "queued",     "time": "2024-01-15T14:00:00Z", "is_static": false}
/// {"type": "backlogged", "date": null}
/// {"type": "completed",  "at": "2024-01-15T14:05:00Z"}
/// {"type": "skipped"}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ActionStateDto {
    Queued {
        time: DateTime<Utc>,
        is_static: bool,
    },
    Backlogged {
        date: Option<NaiveDate>,
    },
    Completed {
        at: DateTime<Utc>,
    },
    Skipped,
}

impl From<ActionState> for ActionStateDto {
    fn from(state: ActionState) -> Self {
        match state {
            ActionState::Queued(ActionTarget { time, is_static }) => {
                ActionStateDto::Queued { time, is_static }
            }
            ActionState::Backlogged(date) => ActionStateDto::Backlogged { date },
            ActionState::Completed(at) => ActionStateDto::Completed { at },
            ActionState::Skipped => ActionStateDto::Skipped,
        }
    }
}

impl From<ActionStateDto> for ActionState {
    fn from(dto: ActionStateDto) -> Self {
        match dto {
            ActionStateDto::Queued { time, is_static } => {
                ActionState::Queued(ActionTarget { time, is_static })
            }
            ActionStateDto::Backlogged { date } => ActionState::Backlogged(date),
            ActionStateDto::Completed { at } => ActionState::Completed(at),
            ActionStateDto::Skipped => ActionState::Skipped,
        }
    }
}

// ── ActionDto ─────────────────────────────────────────────────────────────────

/// HTTP wire representation of [`Action`].
///
/// Differences from the core type:
/// - `state` uses [`ActionStateDto`] (flat, lowercase-tagged object)
/// - `duration_secs` replaces `duration` to avoid chrono's `{"secs":…,"nanos":…}` shape
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDto {
    pub id: Uuid,
    pub lineage_id: Uuid,
    pub origin_routine_id: Option<Uuid>,
    pub title: String,
    pub content: Option<String>,
    /// Duration in whole seconds. `null` means no explicit duration.
    pub duration_secs: Option<i64>,
    pub recurrence: Option<RecurrenceRule>,
    pub saved: bool,
    pub state: ActionStateDto,
}

impl From<Action> for ActionDto {
    fn from(a: Action) -> Self {
        ActionDto {
            id: a.id,
            lineage_id: a.lineage_id,
            origin_routine_id: a.origin_routine_id,
            title: a.title,
            content: a.content,
            duration_secs: a.duration.map(|d| d.num_seconds()),
            recurrence: a.recurrence,
            saved: a.saved,
            state: a.state.into(),
        }
    }
}

impl From<ActionDto> for Action {
    fn from(dto: ActionDto) -> Self {
        Action {
            id: dto.id,
            lineage_id: dto.lineage_id,
            origin_routine_id: dto.origin_routine_id,
            title: dto.title,
            content: dto.content,
            duration: dto.duration_secs.map(Duration::seconds),
            recurrence: dto.recurrence,
            saved: dto.saved,
            state: dto.state.into(),
        }
    }
}

// ── AllData ───────────────────────────────────────────────────────────────────

/// Response body for `GET /api/data`.
#[derive(Debug, Serialize, Deserialize)]
pub struct AllData {
    pub actions: Vec<ActionDto>,
    pub events: Vec<Event>,
    pub routines: Vec<Routine>,
}

// ── CompleteResult ────────────────────────────────────────────────────────────

/// Response body for `POST /api/actions/{id}/complete`.
#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteResult {
    pub completed: ActionDto,
    pub next: Option<ActionDto>,
}
