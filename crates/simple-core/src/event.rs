use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub lineage_id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub time: DateTime<Utc>,
    pub duration: Option<Duration>,
    pub recurrence: Option<Duration>,
    pub ephemeral: bool,
}

impl Event {
    pub fn new(title: impl Into<String>, time: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::now_v7(),
            lineage_id: Uuid::now_v7(),
            title: title.into(),
            content: None,
            time,
            duration: None,
            recurrence: None,
            ephemeral: true,
        }
    }

    pub fn saved(title: impl Into<String>, time: DateTime<Utc>) -> Self {
        Self {
            ephemeral: false,
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

    pub fn with_recurrence(mut self, recurrence: Duration) -> Self {
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

    pub fn next_recurrence(&self) -> Option<Self> {
        self.recurrence.map(|recurrence| Self {
            id: Uuid::now_v7(),
            lineage_id: self.lineage_id,
            title: self.title.clone(),
            content: self.content.clone(),
            time: self.end_time() + recurrence,
            duration: self.duration,
            recurrence: self.recurrence,
            ephemeral: self.ephemeral,
        })
    }
}
