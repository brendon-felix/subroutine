use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    Actionable, Constraints, Context, PipelineEntry, RecurrenceRule, SavedAction, SavedConstraints,
    score,
};

/// A small, dynamic pool of saved actions that tend to be done together. Lives in the
/// pipeline as a placeholder entry. When activated, the system selects the contextually
/// appropriate subset of steps and replaces the placeholder with those concrete entries.
#[derive(Debug, Clone)]
pub struct Subroutine {
    pub id: Uuid,
    pub title: String,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
    pub constraints: SavedConstraints,
    pub recurrence: Option<RecurrenceRule>,
    /// Pool of SavedAction IDs associated with this subroutine. The system picks
    /// which to instantiate based on context at activation time.
    pub steps: Vec<Uuid>,
}

impl Subroutine {
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

    /// Instantiates the contextually appropriate subset of steps given `context`,
    /// returning concrete Action entries tagged with this subroutine's ID.
    /// Steps scoring below `threshold` against the current context are omitted.
    ///
    /// Steps whose IDs are not found in `saved_actions` are silently skipped.
    pub fn instantiate(
        &self,
        saved_actions: &HashMap<Uuid, SavedAction>,
        context: &Context,
        threshold: f32,
    ) -> Vec<PipelineEntry> {
        let completed_ids = HashSet::new();

        self.steps
            .iter()
            .filter_map(|id| saved_actions.get(id))
            .filter_map(|saved| {
                let action = saved.instantiate();
                let entry = PipelineEntry::Action(action.clone());
                let action_score = score(&entry, context, &completed_ids).total;
                if action_score >= threshold {
                    Some(PipelineEntry::Action(crate::Action {
                        subroutine_id: Some(self.id),
                        ..action
                    }))
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Actionable for Subroutine {
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
        self.steps.clone()
    }
}
