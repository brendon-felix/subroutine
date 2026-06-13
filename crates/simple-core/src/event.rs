use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{RecurrenceRule, duration_nanos};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    /// used to group recurring events
    pub lineage_id: Uuid,
    /// the `EventTemplate` used to create this event (if any)
    pub template_id: Option<Uuid>,
    pub title: String,
    pub content: Option<String>,
    pub time: DateTime<Utc>,
    #[serde(with = "duration_nanos")]
    pub duration: Duration,
    pub recurrence: Option<RecurrenceRule>,
}

impl Event {
    pub fn new(title: impl Into<String>, time: DateTime<Utc>, duration: Duration) -> Self {
        let id = Uuid::now_v7();
        Self {
            id,
            lineage_id: id,
            template_id: None,
            title: title.into(),
            content: None,
            time,
            duration,
            recurrence: None,
        }
    }

    // pub fn saved(title: impl Into<String>, time: DateTime<Utc>) -> Self {
    //     Self {
    //         saved: true,
    //         ..Self::new(title, time)
    //     }
    // }

    pub fn with_lineage_id(mut self, lineage_id: Uuid) -> Self {
        self.lineage_id = lineage_id;
        self
    }

    pub fn with_template_id(mut self, template_id: Uuid) -> Self {
        self.template_id = Some(template_id);
        self
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_recurrence_rule(mut self, rule: RecurrenceRule) -> Self {
        self.recurrence = Some(rule);
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn end_time(&self) -> DateTime<Utc> {
        self.time + self.duration
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
            template_id: self.template_id,
            title: self.title.clone(),
            content: self.content.clone(),
            time: rule.next_after(self.end_time()),
            duration: self.duration,
            recurrence: self.recurrence,
        })
    }

    pub fn into_template(self) -> EventTemplate {
        EventTemplate {
            id: self.id,
            lineage_id: self.lineage_id,
            title: self.title,
            content: self.content,
            duration: self.duration,
            recurrence: self.recurrence,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTemplate {
    pub id: Uuid,
    pub lineage_id: Uuid,
    pub title: String,
    pub content: Option<String>,
    #[serde(with = "duration_nanos")]
    pub duration: Duration,
    pub recurrence: Option<RecurrenceRule>,
}

impl EventTemplate {
    pub fn new(title: impl Into<String>, duration: Duration) -> Self {
        Self {
            id: Uuid::now_v7(),
            lineage_id: Uuid::now_v7(),
            title: title.into(),
            content: None,
            duration,
            recurrence: None,
        }
    }

    pub fn with_lineage_id(mut self, lineage_id: Uuid) -> Self {
        self.lineage_id = lineage_id;
        self
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_recurrence(mut self, recurrence: RecurrenceRule) -> Self {
        self.recurrence = Some(recurrence);
        self
    }

    pub fn build(self, time: DateTime<Utc>) -> Event {
        Event {
            id: self.id,
            lineage_id: self.lineage_id,
            template_id: Some(self.id),
            title: self.title,
            content: self.content,
            time,
            duration: self.duration,
            recurrence: self.recurrence,
        }
    }
}
