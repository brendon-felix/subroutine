use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::RecurrenceRule;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ActionTarget {
    pub time: DateTime<Utc>,
    pub is_static: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ActionState {
    Queued(ActionTarget),
    Backlogged(Option<NaiveDate>),
    Completed(DateTime<Utc>),
    Skipped,
}

impl ActionState {
    pub fn queued(time: DateTime<Utc>, is_static: bool) -> Self {
        ActionState::Queued(ActionTarget { time, is_static })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: Uuid,
    pub lineage_id: Uuid,
    pub origin_routine_id: Option<Uuid>,
    pub title: String,
    pub content: Option<String>,
    pub duration: Option<Duration>,
    pub recurrence: Option<RecurrenceRule>,
    pub saved: bool,
    pub state: ActionState,
}

impl Action {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            lineage_id: Uuid::now_v7(),
            origin_routine_id: None,
            title: title.into(),
            content: None,
            duration: None,
            recurrence: None,
            saved: false,
            state: ActionState::Backlogged(None),
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

    pub fn with_saved(mut self, saved: bool) -> Self {
        self.saved = saved;
        self
    }

    pub fn with_origin_routine(mut self, routine_id: Uuid) -> Self {
        self.origin_routine_id = Some(routine_id);
        self
    }

    pub fn with_state(mut self, state: ActionState) -> Self {
        self.state = state;
        self
    }

    pub fn is_saved(&self) -> bool {
        self.saved
    }

    pub fn is_ephemeral(&self) -> bool {
        !self.saved
    }

    pub fn is_queued(&self) -> bool {
        matches!(self.state, ActionState::Queued(_))
    }

    pub fn is_queued_static(&self) -> bool {
        matches!(
            self.state,
            ActionState::Queued(ActionTarget {
                is_static: true,
                ..
            })
        )
    }

    pub fn is_queued_floating(&self) -> bool {
        matches!(
            self.state,
            ActionState::Queued(ActionTarget {
                is_static: false,
                ..
            })
        )
    }

    pub fn is_missed(&self, now: DateTime<Utc>) -> bool {
        matches!(self.state, ActionState::Queued(ActionTarget { time, .. }) if time < now)
    }

    pub fn is_backlogged(&self) -> bool {
        matches!(self.state, ActionState::Backlogged(_))
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.state, ActionState::Completed(_))
    }

    pub fn is_skipped(&self) -> bool {
        matches!(self.state, ActionState::Skipped)
    }

    pub fn target(&self) -> Option<ActionTarget> {
        match self.state {
            ActionState::Queued(target) => Some(target),
            _ => None,
        }
    }

    pub fn set_content(&mut self, content: impl Into<String>) {
        self.content = Some(content.into());
    }

    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = Some(duration);
    }

    pub fn set_recurrence(&mut self, recurrence: RecurrenceRule) {
        self.recurrence = Some(recurrence);
    }

    pub fn set_saved(&mut self, saved: bool) {
        self.saved = saved;
    }

    pub fn set_origin_routine(&mut self, routine_id: Uuid) {
        self.origin_routine_id = Some(routine_id);
    }

    pub fn set_state(&mut self, state: ActionState) {
        self.state = state;
    }

    pub fn backlog(&mut self, backlog_date: Option<NaiveDate>) {
        self.state = ActionState::Backlogged(backlog_date);
    }

    pub fn complete(&mut self, completion_time: DateTime<Utc>) {
        self.state = ActionState::Completed(completion_time);
    }

    pub fn queue(&mut self, time: DateTime<Utc>) {
        let target = ActionTarget {
            time,
            is_static: false,
        };
        self.state = ActionState::Queued(target);
    }

    pub fn queue_static(&mut self, time: DateTime<Utc>) {
        let target = ActionTarget {
            time,
            is_static: true,
        };
        self.state = ActionState::Queued(target);
    }

    pub fn skip(&mut self) {
        self.state = ActionState::Skipped;
    }

    /// Create the next recurrence of this action, if a recurrence rule is set.
    ///
    /// The new instance gets a fresh `id`, the same `lineage_id`, and a
    /// `target` advanced by the recurrence rule from the current target.
    /// Returns `None` if either `recurrence` or `target` is unset.
    pub fn next_occurence(&self) -> Option<Self> {
        let rule = self.recurrence?;
        let last_target = self.target()?.time;
        Some(Self {
            id: Uuid::now_v7(),
            lineage_id: self.lineage_id,
            origin_routine_id: self.origin_routine_id,
            title: self.title.clone(),
            content: self.content.clone(),
            duration: self.duration,
            recurrence: self.recurrence,
            saved: self.saved,
            state: ActionState::Queued(ActionTarget {
                time: rule.next_after(last_target),
                is_static: false,
            }),
        })
    }

    pub fn new_saved_instance(self) -> Option<Self> {
        self.is_saved().then(|| {
            let mut clone = self.clone();
            clone.id = Uuid::now_v7();
            clone.saved = false;
            clone
        })
    }
}
