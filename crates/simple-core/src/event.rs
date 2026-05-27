use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::RecurrenceRule;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub lineage_id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub time: DateTime<Utc>,
    pub duration: Option<Duration>,
    pub recurrence: Option<RecurrenceRule>,
    pub saved: bool,
}

impl Event {
    pub fn new(title: impl Into<String>, time: DateTime<Utc>) -> Self {
        let id = Uuid::now_v7();
        Self {
            id,
            lineage_id: id,
            title: title.into(),
            content: None,
            time,
            duration: None,
            recurrence: None,
            saved: false,
        }
    }

    pub fn saved(title: impl Into<String>, time: DateTime<Utc>) -> Self {
        Self {
            saved: true,
            ..Self::new(title, time)
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_recurrence(mut self, recurrence: RecurrenceRule) -> Self {
        self.recurrence = Some(recurrence);
        self
    }

    pub fn end_time(&self) -> DateTime<Utc> {
        match self.duration {
            Some(duration) => self.time + duration,
            None => self.time,
        }
    }

    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.end_time() < now
    }

    /// Create the next recurrence of this event, if a recurrence rule is set.
    ///
    /// The new instance gets a fresh `id`, the same `lineage_id`, and a
    /// `time` advanced by the recurrence rule from the end of this event.
    /// Returns `None` if `recurrence` is unset.
    pub fn next_recurrence(&self) -> Option<Self> {
        let rule = self.recurrence?;
        Some(Self {
            id: Uuid::now_v7(),
            lineage_id: self.lineage_id,
            title: self.title.clone(),
            content: self.content.clone(),
            time: rule.next_after(self.end_time()),
            duration: self.duration,
            recurrence: self.recurrence,
            saved: self.saved,
        })
    }
}
