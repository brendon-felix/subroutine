use std::collections::HashSet;

use anyhow::{Result, bail};
use chrono::Duration;
use uuid::Uuid;

use crate::{Action, Actionable, Context, Event, Routine, Subroutine, score};

pub const DEFAULT_PROMOTION_THRESHOLD: f32 = 0.5;

/// Buffer time between pipeline entries. Not something the user actively does —
/// it is time reserved for mental and physical preparation between actions.
/// Does not implement Actionable.
#[derive(Debug, Clone)]
pub struct Transition {
    pub id: Uuid,
    pub duration: Duration,
}

impl Transition {
    pub fn new(duration: Duration) -> Self {
        Self {
            id: Uuid::new_v4(),
            duration,
        }
    }
}

/// A single entry in the Pipeline. Covers everything that can appear in the backlog
/// or queue. Transition is the only variant that does not implement Actionable.
#[derive(Debug, Clone)]
pub enum PipelineEntry {
    Action(Action),
    Event(Event),
    Routine(Routine),
    Subroutine(Subroutine),
    Transition(Transition),
}

impl PipelineEntry {
    pub fn id(&self) -> Uuid {
        match self {
            PipelineEntry::Action(a) => a.id(),
            PipelineEntry::Event(e) => e.id(),
            PipelineEntry::Routine(r) => r.id(),
            PipelineEntry::Subroutine(s) => s.id(),
            PipelineEntry::Transition(t) => t.id,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            PipelineEntry::Action(a) => a.title(),
            PipelineEntry::Event(e) => e.title(),
            PipelineEntry::Routine(r) => r.title(),
            PipelineEntry::Subroutine(s) => s.title(),
            PipelineEntry::Transition(_) => "Transition",
        }
    }

    /// Returns a reference to the inner Actionable, if this entry implements it.
    /// Returns None for Transition.
    pub fn as_actionable(&self) -> Option<&dyn Actionable> {
        match self {
            PipelineEntry::Action(a) => Some(a),
            PipelineEntry::Event(e) => Some(e),
            PipelineEntry::Routine(r) => Some(r),
            PipelineEntry::Subroutine(s) => Some(s),
            PipelineEntry::Transition(_) => None,
        }
    }

    pub fn is_transition(&self) -> bool {
        matches!(self, PipelineEntry::Transition(_))
    }
}

/// The central system through which all actions flow. Maintains two lists:
///
/// - `backlog`: a semi-ordered pool of entries that should eventually be acted on.
///   Things are added here when the user captures them.
///
/// - `queue`: the active, fully ordered list of entries the user can act on now.
///   Ordered by score. Includes auto-generated transitions between entries.
///
/// Items move between backlog and queue automatically based on score relative to
/// `promotion_threshold`, or manually at the user's request.
pub struct Pipeline {
    backlog: Vec<PipelineEntry>,
    queue: Vec<PipelineEntry>,
    promotion_threshold: f32,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            backlog: Vec::new(),
            queue: Vec::new(),
            promotion_threshold: DEFAULT_PROMOTION_THRESHOLD,
        }
    }

    pub fn with_promotion_threshold(mut self, threshold: f32) -> Self {
        self.promotion_threshold = threshold;
        self
    }

    pub fn promotion_threshold(&self) -> f32 {
        self.promotion_threshold
    }

    pub fn backlog(&self) -> &[PipelineEntry] {
        &self.backlog
    }

    pub fn queue(&self) -> &[PipelineEntry] {
        &self.queue
    }

    /// Adds an entry to the backlog. Returns an error if the entry is a Transition,
    /// since transitions are ephemeral and only exist within the queue.
    pub fn push(&mut self, entry: PipelineEntry) -> Result<()> {
        if entry.is_transition() {
            tracing::warn!(
                "Attempted to push a Transition into the backlog — transitions are ephemeral and belong only in the queue"
            );
            bail!("Transitions cannot be added to the backlog");
        }
        self.backlog.push(entry);
        Ok(())
    }

    /// Moves an entry from the backlog into the queue by ID. The entry is appended
    /// to the end of the queue. Position will be score-based once scoring is implemented.
    /// Returns an error if no entry with the given ID exists in the backlog.
    pub fn promote(&mut self, id: Uuid) -> Result<()> {
        let position = self
            .backlog
            .iter()
            .position(|entry| entry.id() == id)
            .ok_or_else(|| anyhow::anyhow!("No entry with id {} found in the backlog", id))?;

        let entry = self.backlog.remove(position);
        self.queue.push(entry);
        Ok(())
    }

    /// Scores all entries in both lists against the current context and drives
    /// automatic promotion and demotion:
    ///
    /// 1. Queue entries scoring below `promotion_threshold` are demoted to the backlog.
    /// 2. Backlog entries scoring at or above `promotion_threshold` are promoted to the queue.
    /// 3. The queue is re-sorted by score descending.
    ///
    /// Transitions in the queue are skipped — they are ephemeral and unscored.
    /// Demotion runs before promotion so that a newly demoted entry is not
    /// immediately re-promoted in the same pass.
    pub fn refresh(&mut self, context: &Context, completed_ids: &HashSet<Uuid>) {
        let to_demote: Vec<Uuid> = self
            .queue
            .iter()
            .filter(|entry| !entry.is_transition())
            .filter_map(|entry| {
                let total = score(entry, context, completed_ids).total;
                if total < self.promotion_threshold {
                    Some(entry.id())
                } else {
                    None
                }
            })
            .collect();

        for id in to_demote {
            if let Some(position) = self.queue.iter().position(|entry| entry.id() == id) {
                let entry = self.queue.remove(position);
                self.backlog.push(entry);
            }
        }

        let to_promote: Vec<Uuid> = self
            .backlog
            .iter()
            .filter_map(|entry| {
                let total = score(entry, context, completed_ids).total;
                if total >= self.promotion_threshold {
                    Some(entry.id())
                } else {
                    None
                }
            })
            .collect();

        for id in to_promote {
            if let Some(position) = self.backlog.iter().position(|entry| entry.id() == id) {
                let entry = self.backlog.remove(position);
                self.queue.push(entry);
            }
        }

        self.queue.sort_by(|a, b| {
            let score_a = score(a, context, completed_ids).total;
            let score_b = score(b, context, completed_ids).total;
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Moves an entry from the queue back into the backlog by ID. Returns an error
    /// if no entry with the given ID exists in the queue, or if the entry is a Transition.
    pub fn demote(&mut self, id: Uuid) -> Result<()> {
        let position = self
            .queue
            .iter()
            .position(|entry| entry.id() == id)
            .ok_or_else(|| anyhow::anyhow!("No entry with id {} found in the queue", id))?;

        if self.queue[position].is_transition() {
            tracing::warn!(
                "Attempted to demote a Transition — transitions are ephemeral and cannot be moved to the backlog"
            );
            bail!("Transitions cannot be demoted to the backlog");
        }

        let entry = self.queue.remove(position);
        self.backlog.push(entry);
        Ok(())
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}
