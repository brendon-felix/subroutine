use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Unique identifier for this action instance
    pub id: Uuid,
    /// Stable identifier shared across all recurrences
    pub lineage_id: Uuid,
    /// The routine which spawned this action, if any
    pub origin_routine_id: Option<Uuid>,
    /// The user-facing title of this action
    pub title: String,
    /// Optional user-facing content with more details
    pub content: Option<String>,
    /// When the user should perform this action.
    ///
    /// `Some` → the action is in the queue (scheduled).
    /// `None` → the action is in the backlog (unscheduled).
    pub target: Option<DateTime<Utc>>,
    /// Whether the target time is static (if `false`, the action can be automatically rescheduled)
    pub target_static: bool,
    /// A calendar date without a specific time, used for backlog actions.
    ///
    /// When `target` is `None` and `naive_date` is `Some`, the pipeline will
    /// automatically promote the action to the queue (assigning a `target`)
    /// once `naive_date` is today or in the past (in the user's local timezone).
    pub naive_date: Option<NaiveDate>,
    /// How long the action typically takes to complete
    pub duration: Option<Duration>,
    /// If set, the action is automatically re-instantiated after completion with a new target time calculated by adding this interval to the completion time.
    pub recurrence: Option<Duration>,
    /// Whether this action is ephemeral. Ephemeral actions cannot be reused
    pub ephemeral: bool,
    /// When this action was completed, if it has been.
    ///
    /// Combined with pipeline membership this determines an action's state:
    /// - In pipeline + `target` is `Some`  → queued
    /// - In pipeline + `target` is `None`  → backlogged
    /// - Not in pipeline + `completed_at` is `Some` → completed
    /// - Not in pipeline + `completed_at` is `None` → skipped
    pub completed_at: Option<DateTime<Utc>>,
}

impl Action {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            lineage_id: Uuid::now_v7(),
            origin_routine_id: None,
            title: title.into(),
            content: None,
            target: None,
            target_static: false,
            naive_date: None,
            duration: None,
            recurrence: None,
            ephemeral: true,
            completed_at: None,
        }
    }

    pub fn new_saved(title: impl Into<String>) -> Self {
        Self {
            ephemeral: false,
            ..Self::new(title)
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_target(mut self, target: DateTime<Utc>, target_static: bool) -> Self {
        self.target = Some(target);
        self.target_static = target_static;
        // Clear naive_date when a full target is set — the action is now queued.
        self.naive_date = None;
        self
    }

    /// Set a floating calendar date for this action (backlog hint).
    ///
    /// Clears any existing `target` so the action is placed in the backlog.
    /// The pipeline will promote it to the queue once the date arrives.
    pub fn with_naive_date(mut self, date: NaiveDate) -> Self {
        self.naive_date = Some(date);
        // A naive date puts the action in the backlog — clear any prior target.
        self.target = None;
        self.target_static = false;
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

    pub fn with_origin_routine(mut self, routine_id: Uuid) -> Self {
        self.origin_routine_id = Some(routine_id);
        self
    }

    pub fn with_completed_at(mut self, completed_at: DateTime<Utc>) -> Self {
        self.completed_at = Some(completed_at);
        self
    }

    pub fn next_recurrence(&self) -> Option<Self> {
        self.recurrence.map(|recurrence| Self {
            id: Uuid::now_v7(),
            lineage_id: self.lineage_id,
            origin_routine_id: self.origin_routine_id,
            title: self.title.clone(),
            content: self.content.clone(),
            target: self.target.map(|t| t + recurrence),
            target_static: false,
            naive_date: None,
            duration: self.duration,
            recurrence: self.recurrence,
            ephemeral: self.ephemeral,
            completed_at: None,
        })
    }
}
