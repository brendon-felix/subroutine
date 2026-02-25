use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::{ActionContext, Actionable, Constraints, RecurrenceRule, SavedConstraints};

/// A reusable template for an Event. Recurring events (e.g. a weekly meeting) are
/// represented as a SavedEvent with a RecurrenceRule. One-off events can also be saved
/// without recurrence.
#[derive(Debug, Clone)]
pub struct SavedEvent {
    pub id: Uuid,
    pub title: String,
    pub content: Option<String>,
    /// Default duration for instances of this event.
    pub default_duration: Duration,
    pub context: ActionContext,
    pub constraints: SavedConstraints,
    pub recurrence: Option<RecurrenceRule>,
    /// Ordered list of SavedAction IDs that form this event's agenda.
    pub steps: Vec<Uuid>,
}

impl SavedEvent {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            content: None,
            default_duration: Duration::zero(),
            context: ActionContext::default(),
            constraints: SavedConstraints::default(),
            recurrence: None,
            steps: Vec::new(),
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.default_duration = duration;
        self
    }

    /// Creates a concrete Event from this template, scheduled at `time`.
    /// Steps are NOT instantiated here — the caller instantiates the associated
    /// SavedActions separately and attaches their IDs to the returned Event.
    pub fn instantiate(&self, time: DateTime<Utc>) -> Event {
        let now = Utc::now();
        Event {
            id: Uuid::new_v4(),
            title: self.title.clone(),
            content: self.content.clone(),
            created_at: now,
            time,
            duration: self.default_duration,
            constraints: self.constraints.materialize(now),
            ephemeral: false,
            saved_event_id: Some(self.id),
            routine_id: None,
            subroutine_id: None,
            actions: Vec::new(),
        }
    }
}

/// A concrete instance of a scheduled event in the pipeline. Every Event exists in
/// either the backlog or the queue — there is no separate event list.
#[derive(Debug, Clone)]
pub struct Event {
    pub id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
    pub time: DateTime<Utc>,
    pub duration: Duration,
    pub constraints: Constraints,
    /// If true, no SavedEvent was created for this event and it will not appear
    /// as a reusable template. Useful for one-off events.
    pub ephemeral: bool,
    /// The SavedEvent template this event was instantiated from, if any.
    pub saved_event_id: Option<Uuid>,
    /// The Routine that spawned this event, if any.
    pub routine_id: Option<Uuid>,
    /// The Subroutine that spawned this event, if any.
    pub subroutine_id: Option<Uuid>,
    /// Concrete Action IDs that are part of this event's agenda.
    pub actions: Vec<Uuid>,
}

impl Event {
    pub fn new(title: impl Into<String>, time: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            content: None,
            created_at: Utc::now(),
            time,
            duration: Duration::zero(),
            constraints: Constraints::default(),
            ephemeral: false,
            saved_event_id: None,
            routine_id: None,
            subroutine_id: None,
            actions: Vec::new(),
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_constraints(mut self, constraints: Constraints) -> Self {
        self.constraints = constraints;
        self
    }

    pub fn with_actions(mut self, actions: Vec<Uuid>) -> Self {
        self.actions = actions;
        self
    }

    pub fn set_time(&mut self, time: DateTime<Utc>) {
        self.time = time;
    }

    pub fn push_action(&mut self, action_id: Uuid) {
        self.actions.push(action_id);
    }

    pub fn insert_action(&mut self, index: usize, action_id: Uuid) {
        self.actions.insert(index, action_id);
    }

    pub fn remove_action(&mut self, action_id: Uuid) {
        self.actions.retain(|&id| id != action_id);
    }
}

impl Actionable for Event {
    fn id(&self) -> Uuid {
        self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    fn created_time(&self) -> DateTime<Utc> {
        self.created_at
    }

    fn target_time(&self) -> Option<DateTime<Utc>> {
        Some(self.time)
    }

    fn constraints(&self) -> Constraints {
        self.constraints.clone()
    }

    fn actions(&self) -> Vec<Uuid> {
        self.actions.clone()
    }
}
