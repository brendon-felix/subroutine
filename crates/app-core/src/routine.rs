use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    Actionable, Constraints, PipelineEntry, RecurrenceRule, SavedAction, SavedConstraints,
    SavedEvent,
};

/// A single step in a Routine, referencing a saved template by type and ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavedStep {
    Action(Uuid),
    Event(Uuid),
}

/// A user-defined, ordered group of actions and events. Lives in the pipeline as a
/// placeholder entry. When activated, it is replaced by the instantiated concrete entries
/// for each of its steps.
#[derive(Debug, Clone)]
pub struct Routine {
    pub id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
    pub constraints: SavedConstraints,
    pub recurrence: Option<RecurrenceRule>,
    pub steps: Vec<SavedStep>,
}

impl Routine {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            content: None,
            created_at: Utc::now(),
            constraints: SavedConstraints::default(),
            recurrence: None,
            steps: Vec::new(),
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Instantiates all steps and returns the resulting concrete pipeline entries,
    /// each tagged with this routine's ID. The caller is responsible for adding them
    /// to the pipeline and persisting them.
    ///
    /// SavedEvent steps are scheduled at `Utc::now()` as a placeholder; callers
    /// that need specific scheduling should instantiate steps manually.
    ///
    /// Steps whose IDs are not found in the provided maps are silently skipped.
    pub fn instantiate(
        &self,
        saved_actions: &HashMap<Uuid, SavedAction>,
        saved_events: &HashMap<Uuid, SavedEvent>,
    ) -> Vec<PipelineEntry> {
        self.steps
            .iter()
            .filter_map(|step| match step {
                SavedStep::Action(id) => saved_actions.get(id).map(|saved| {
                    let action = crate::Action {
                        routine_id: Some(self.id),
                        ..saved.instantiate()
                    };
                    PipelineEntry::Action(action)
                }),
                SavedStep::Event(id) => saved_events.get(id).map(|saved| {
                    let event = crate::Event {
                        routine_id: Some(self.id),
                        ..saved.instantiate(Utc::now())
                    };
                    PipelineEntry::Event(event)
                }),
            })
            .collect()
    }
}

impl Actionable for Routine {
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
        None
    }

    fn constraints(&self) -> Constraints {
        self.constraints.materialize(Utc::now())
    }

    fn actions(&self) -> Vec<Uuid> {
        self.steps
            .iter()
            .map(|step| match step {
                SavedStep::Action(id) | SavedStep::Event(id) => *id,
            })
            .collect()
    }
}
