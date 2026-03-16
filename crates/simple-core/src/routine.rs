use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineStep {
    pub title: String,
    pub duration: Option<Duration>,
}

impl RoutineStep {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            duration: None,
        }
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Routine {
    pub id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub target: Option<DateTime<Utc>>,
    pub steps: Vec<RoutineStep>,
    pub recurrence: Option<Duration>,
}

impl Routine {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            title: title.into(),
            content: None,
            target: None,
            steps: Vec::new(),
            recurrence: None,
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_target(mut self, target: DateTime<Utc>) -> Self {
        self.target = Some(target);
        self
    }

    pub fn with_steps(mut self, steps: Vec<RoutineStep>) -> Self {
        self.steps = steps;
        self
    }

    pub fn with_recurrence(mut self, recurrence: Duration) -> Self {
        self.recurrence = Some(recurrence);
        self
    }

    pub fn add_step(&mut self, step: RoutineStep) {
        self.steps.push(step);
    }

    pub fn insert_step(&mut self, index: usize, step: RoutineStep) {
        self.steps.insert(index, step);
    }
}
