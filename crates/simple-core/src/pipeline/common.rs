use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Action, ActionState, Event, Routine, pipeline::DEFAULT_ACTION_DURATION};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnyItem {
    Action(Action),
    Event(Event),
    Routine(Routine),
}

impl AnyItem {
    pub fn title(&self) -> &str {
        match self {
            AnyItem::Action(a) => &a.title,
            AnyItem::Event(e) => &e.title,
            AnyItem::Routine(r) => &r.title,
        }
    }

    pub fn id(&self) -> Uuid {
        match self {
            AnyItem::Action(a) => a.id,
            AnyItem::Event(e) => e.id,
            AnyItem::Routine(r) => r.id,
        }
    }

    pub fn truncated_id(&self) -> u64 {
        // use second half of v7 UUID (timestamp based)
        match self {
            AnyItem::Action(a) => a.id.as_u64_pair().1,
            AnyItem::Event(e) => e.id.as_u64_pair().1,
            AnyItem::Routine(r) => r.id.as_u64_pair().1,
        }
    }

    pub fn start_time(&self) -> Option<DateTime<Utc>> {
        match self {
            AnyItem::Action(a) => match a.state {
                ActionState::Scheduled(t) => Some(t.time),
                _ => None,
            },
            AnyItem::Event(e) => Some(e.time),
            AnyItem::Routine(_) => None,
        }
    }

    pub fn start_time_local(&self) -> Option<DateTime<Local>> {
        match self {
            AnyItem::Action(a) => match a.state {
                ActionState::Scheduled(t) => Some(t.time.with_timezone(&Local)),
                _ => None,
            },
            AnyItem::Event(e) => Some(e.time.with_timezone(&Local)),
            AnyItem::Routine(_) => None,
        }
    }

    pub fn end_time(&self) -> Option<DateTime<Utc>> {
        let duration = self.duration().unwrap_or(DEFAULT_ACTION_DURATION);
        match self {
            AnyItem::Action(a) => match a.state {
                ActionState::Scheduled(t) => Some(t.time + duration),
                _ => None,
            },
            AnyItem::Event(e) => Some(e.time + duration),
            AnyItem::Routine(_) => None,
        }
    }

    pub fn time_local(&self) -> Option<DateTime<Local>> {
        match self {
            AnyItem::Action(a) => match a.state {
                ActionState::Scheduled(t) => Some(t.time.with_timezone(&Local)),
                _ => None,
            },
            AnyItem::Event(e) => Some(e.time.with_timezone(&Local)),
            AnyItem::Routine(_) => None,
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        match self {
            AnyItem::Action(a) => a.duration,
            AnyItem::Event(e) => e.duration.into(),
            AnyItem::Routine(r) => r.duration(),
        }
    }

    pub fn is_action(&self) -> bool {
        matches!(self, AnyItem::Action(_))
    }

    pub fn is_event(&self) -> bool {
        matches!(self, AnyItem::Event(_))
    }
}
