use chrono::{DateTime, Duration, NaiveTime, Utc};
use uuid::Uuid;

use crate::{Actionable, Constraints, RecurrenceRule, SavedConstraints};

#[derive(Debug, Clone, Copy, Default)]
pub struct ActionContext {
    /// How draining or energizing this action is per unit time.
    /// Negative values drain spoons; positive values restore them.
    /// Range: -2 (very draining) to +2 (very energizing).
    pub energy_rate: Option<i8>,
    /// How much focused attention this action requires.
    /// Range: 1 (minimal) to 5 (deep focus required).
    pub attention_level: Option<u8>,
    /// How hard it is to start and stop this action.
    /// High values mean this action resists interruption and is hard to begin.
    /// Range: 1 (easy to start/stop) to 5 (very hard).
    pub transition_difficulty: Option<u8>,
    /// How important this action is, independent of urgency.
    /// Range: 1 (low) to 5 (critical).
    pub importance: Option<u8>,
}

/// A reusable template for an Action. Recurring actions (e.g. daily exercise) are
/// represented as a SavedAction with a RecurrenceRule. One-off actions that the user
/// wants to remember as a template can also be saved without recurrence.
#[derive(Debug, Clone)]
pub struct SavedAction {
    pub id: Uuid,
    pub title: String,
    pub content: Option<String>,
    /// Preferred time of day to do this action, used to compute a concrete `target_time`
    /// on instantiation by resolving to the next upcoming occurrence.
    pub target_time: Option<NaiveTime>,
    pub context: ActionContext,
    pub constraints: SavedConstraints,
    pub recurrence: Option<RecurrenceRule>,
}

impl SavedAction {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            content: None,
            target_time: None,
            context: ActionContext::default(),
            constraints: SavedConstraints::default(),
            recurrence: None,
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Creates a new concrete `Action` from this template, materialized to the current time.
    /// The returned action is not yet persisted or added to the pipeline — the caller is
    /// responsible for both.
    pub fn instantiate(&self) -> Action {
        let now = Utc::now();

        let target_time = self.target_time.map(|time| {
            let today = now.date_naive();
            let candidate = today.and_time(time).and_utc();
            if candidate > now {
                candidate
            } else {
                (today + Duration::days(1)).and_time(time).and_utc()
            }
        });

        Action {
            id: Uuid::new_v4(),
            title: self.title.clone(),
            content: self.content.clone(),
            created_at: now,
            target_time,
            context: self.context,
            constraints: self.constraints.materialize(now),
            ephemeral: false,
            saved_action_id: Some(self.id),
            routine_id: None,
            subroutine_id: None,
        }
    }
}

/// A concrete instance of an action in the pipeline. Every Action exists in either
/// the backlog or the queue — there is no separate action list.
#[derive(Debug, Clone)]
pub struct Action {
    pub id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
    pub target_time: Option<DateTime<Utc>>,
    pub context: ActionContext,
    pub constraints: Constraints,
    /// If true, no SavedAction was created for this action and it will not appear
    /// as a reusable template. Useful for one-off actions.
    pub ephemeral: bool,
    /// The SavedAction template this action was instantiated from, if any.
    pub saved_action_id: Option<Uuid>,
    /// The Routine that spawned this action, if any.
    pub routine_id: Option<Uuid>,
    /// The Subroutine that spawned this action, if any.
    pub subroutine_id: Option<Uuid>,
}

impl Action {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            content: None,
            created_at: Utc::now(),
            target_time: None,
            context: ActionContext::default(),
            constraints: Constraints::default(),
            ephemeral: false,
            saved_action_id: None,
            routine_id: None,
            subroutine_id: None,
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn ephemeral(mut self, ephemeral: bool) -> Self {
        self.ephemeral = ephemeral;
        self
    }
}

impl Actionable for Action {
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
        self.target_time
    }

    fn constraints(&self) -> Constraints {
        self.constraints.clone()
    }

    fn actions(&self) -> Vec<Uuid> {
        vec![self.id]
    }
}
