use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{RecurrenceRule, duration_nanos_opt};

#[derive(Debug, Clone, Copy)]
pub struct ActionTarget {
    pub time: DateTime<Utc>,
    pub is_static: bool,
}

impl ActionTarget {
    pub fn new(time: DateTime<Utc>, is_static: bool) -> Self {
        Self { time, is_static }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ActionState {
    Scheduled(ActionTarget),
    Backlogged(Option<NaiveDate>),
    Completed(DateTime<Utc>),
    Skipped,
}

impl Serialize for ActionState {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        match self {
            ActionState::Scheduled(t) => {
                let mut st = s.serialize_struct("ActionState", 3)?;
                st.serialize_field("type", "queued")?;
                st.serialize_field("time", &t.time)?;
                st.serialize_field("is_static", &t.is_static)?;
                st.end()
            }
            ActionState::Backlogged(d) => {
                let mut st = s.serialize_struct("ActionState", 2)?;
                st.serialize_field("type", "backlogged")?;
                st.serialize_field("date", d)?;
                st.end()
            }
            ActionState::Completed(at) => {
                let mut st = s.serialize_struct("ActionState", 2)?;
                st.serialize_field("type", "completed")?;
                st.serialize_field("at", at)?;
                st.end()
            }
            ActionState::Skipped => {
                let mut st = s.serialize_struct("ActionState", 1)?;
                st.serialize_field("type", "skipped")?;
                st.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ActionState {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "type")]
        enum Helper {
            #[serde(rename = "queued")]
            Queued {
                time: DateTime<Utc>,
                is_static: bool,
            },
            #[serde(rename = "backlogged")]
            Backlogged { date: Option<NaiveDate> },
            #[serde(rename = "completed")]
            Completed { at: DateTime<Utc> },
            #[serde(rename = "skipped")]
            Skipped,
        }
        match Helper::deserialize(d)? {
            Helper::Queued { time, is_static } => {
                Ok(ActionState::Scheduled(ActionTarget { time, is_static }))
            }
            Helper::Backlogged { date } => Ok(ActionState::Backlogged(date)),
            Helper::Completed { at } => Ok(ActionState::Completed(at)),
            Helper::Skipped => Ok(ActionState::Skipped),
        }
    }
}

impl ActionState {
    pub fn queued(time: DateTime<Utc>, is_static: bool) -> Self {
        ActionState::Scheduled(ActionTarget { time, is_static })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: Uuid,
    /// used to group recurring actions
    pub lineage_id: Uuid,
    /// the `Routine` which built this action (if any)
    pub routine_id: Option<Uuid>,
    /// the `ActionTemplate` used to create this action (if any)
    pub template_id: Option<Uuid>,
    pub title: String,
    /// optional general purpose content
    pub content: Option<String>,
    #[serde(with = "duration_nanos_opt")]
    pub duration: Option<Duration>,
    pub recurrence: Option<RecurrenceRule>,
    pub state: ActionState,
}

impl Action {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            lineage_id: Uuid::now_v7(),
            routine_id: None,
            template_id: None,
            title: title.into(),
            content: None,
            duration: None,
            recurrence: None,
            state: ActionState::Backlogged(None),
        }
    }

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

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_recurrence_rule(mut self, rule: RecurrenceRule) -> Self {
        self.recurrence = Some(rule);
        self
    }

    // pub fn with_saved(mut self, saved: bool) -> Self {
    //     self.saved = saved;
    //     self
    // }

    pub fn with_origin_routine(mut self, routine_id: Uuid) -> Self {
        self.routine_id = Some(routine_id);
        self
    }

    pub fn with_state(mut self, state: ActionState) -> Self {
        self.state = state;
        self
    }

    // pub fn is_saved(&self) -> bool {
    //     // self.saved
    //     self.template_id.is_some()
    // }

    pub fn is_from_recurrence(&self) -> bool {
        self.lineage_id != self.id
    }

    pub fn is_from_routine(&self) -> bool {
        self.routine_id.is_some()
    }

    pub fn is_from_template(&self) -> bool {
        self.template_id.is_some()
    }

    pub fn parent_routine_id(&self) -> Option<Uuid> {
        self.routine_id
    }

    pub fn parent_template_id(&self) -> Option<Uuid> {
        self.template_id
    }

    pub fn is_queued(&self) -> bool {
        matches!(self.state, ActionState::Scheduled(_))
    }

    pub fn is_queued_static(&self) -> bool {
        matches!(
            self.state,
            ActionState::Scheduled(ActionTarget {
                is_static: true,
                ..
            })
        )
    }

    pub fn is_queued_floating(&self) -> bool {
        matches!(
            self.state,
            ActionState::Scheduled(ActionTarget {
                is_static: false,
                ..
            })
        )
    }

    pub fn is_missed(&self, now: DateTime<Utc>) -> bool {
        matches!(self.state, ActionState::Scheduled(ActionTarget { time, .. }) if time < now)
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
            ActionState::Scheduled(target) => Some(target),
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

    pub fn let_lineage_id(&mut self, lineage_id: Uuid) {
        self.lineage_id = lineage_id;
    }

    pub fn set_routine_id(&mut self, routine_id: Uuid) {
        self.routine_id = Some(routine_id);
    }

    pub fn set_template_id(&mut self, template_id: Uuid) {
        self.template_id = Some(template_id);
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
        self.state = ActionState::Scheduled(target);
    }

    pub fn queue_static(&mut self, time: DateTime<Utc>) {
        let target = ActionTarget {
            time,
            is_static: true,
        };
        self.state = ActionState::Scheduled(target);
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
            routine_id: self.routine_id,
            template_id: self.template_id,
            title: self.title.clone(),
            content: self.content.clone(),
            duration: self.duration,
            recurrence: self.recurrence,
            state: ActionState::Scheduled(ActionTarget {
                time: rule.next_after(last_target),
                is_static: false,
            }),
        })
    }

    // pub fn new_saved_instance(self) -> Option<Self> {
    //     self.is_saved().then(|| {
    //         let mut clone = self.clone();
    //         clone.id = Uuid::now_v7();
    //         clone.saved = false;
    //         clone
    //     })
    // }

    pub fn into_template(self) -> ActionTemplate {
        ActionTemplate {
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
pub struct ActionTemplate {
    pub id: Uuid,
    pub lineage_id: Uuid,
    pub title: String,
    pub content: Option<String>,
    #[serde(with = "duration_nanos_opt")]
    pub duration: Option<Duration>,
    pub recurrence: Option<RecurrenceRule>,
}

impl ActionTemplate {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            lineage_id: Uuid::now_v7(),
            title: title.into(),
            content: None,
            duration: None,
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

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn with_recurrence(mut self, recurrence: RecurrenceRule) -> Self {
        self.recurrence = Some(recurrence);
        self
    }

    pub fn build_scheduled(self, target: ActionTarget) -> Action {
        Action {
            id: self.id,
            lineage_id: self.lineage_id,
            routine_id: None,
            template_id: Some(self.id),
            title: self.title,
            content: self.content,
            duration: self.duration,
            recurrence: self.recurrence,
            state: ActionState::Scheduled(target),
        }
    }

    pub fn build_backlogged(self, date: Option<NaiveDate>) -> Action {
        Action {
            id: self.id,
            lineage_id: self.lineage_id,
            routine_id: None,
            template_id: Some(self.id),
            title: self.title,
            content: self.content,
            duration: self.duration,
            recurrence: self.recurrence,
            state: ActionState::Backlogged(date),
        }
    }
}
