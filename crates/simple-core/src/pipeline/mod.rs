use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Action, Event};

mod queue;
mod schedule;

use queue::{
    check_static_overlaps, collect_trailing_chain, displace_non_static_conflicts, expedite_queue,
    next_available_slot, reanchor_trailing_chain,
};
use schedule::{
    action_effective_duration, event_intervals, find_free_slot, group_consecutive, is_missed,
};

#[derive(Debug, Clone)]
pub struct OverlapWarning {
    pub inserted_title: String,
    pub conflicting_title: String,
}

const DEFAULT_ACTION_DURATION: Duration = Duration::minutes(5);
const CONSECUTIVE_GAP_THRESHOLD: Duration = Duration::minutes(5);
const SEMI_CONSECUTIVE_GAP_THRESHOLD: Duration = Duration::minutes(30);
const EXPEDITE_HORIZON: Duration = Duration::hours(6);

#[derive(Debug, Default)]
pub struct RefreshResult {
    pub new_events: Vec<Event>,
    pub new_actions: Vec<Action>,
    pub demoted_actions: Vec<Action>,
    pub promoted_actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueueItem {
    Action(Action),
    Event(Event),
}

impl QueueItem {
    pub fn title(&self) -> &str {
        match self {
            QueueItem::Action(a) => &a.title,
            QueueItem::Event(e) => &e.title,
        }
    }

    pub fn id(&self) -> Uuid {
        match self {
            QueueItem::Action(a) => a.id,
            QueueItem::Event(e) => e.id,
        }
    }

    pub fn time(&self) -> Option<DateTime<Utc>> {
        match self {
            QueueItem::Action(a) => a.target,
            QueueItem::Event(e) => Some(e.time),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Pipeline {
    pub backlog: Vec<Action>,
    pub queue: Vec<QueueItem>,
}

impl Pipeline {
    pub fn next_available_slot(&self, now: DateTime<Utc>, duration: Duration) -> DateTime<Utc> {
        next_available_slot(&self.queue, now, duration)
    }

    /// Inserts a non-static action at the next available slot. Does not check
    /// for overlaps — refresh will reschedule it if something moves in the way.
    pub fn queue_action_auto(&mut self, mut action: Action, now: DateTime<Utc>) {
        let duration = action_effective_duration(&action);
        let target = next_available_slot(&self.queue, now, duration);
        action.target = Some(target);
        action.target_static = false;
        self.queue.push(QueueItem::Action(action));
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));
    }

    /// Inserts a static action, then displaces any non-static actions that
    /// now conflict with it. Returns warnings for immovable conflicts.
    pub fn queue_action_static(
        &mut self,
        mut action: Action,
        now: DateTime<Utc>,
    ) -> Vec<OverlapWarning> {
        action.target_static = true;
        let (start, end) = match action.target {
            Some(t) => (t, t + action_effective_duration(&action)),
            None => return Vec::new(),
        };
        let warnings = check_static_overlaps(&self.queue, &action.title, start, end);
        self.queue.push(QueueItem::Action(action));
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));
        displace_non_static_conflicts(&mut self.queue, now);
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));
        warnings
    }

    /// Inserts an event, then displaces any non-static actions that now
    /// conflict with it. Returns warnings for immovable conflicts.
    pub fn queue_event(&mut self, event: Event, now: DateTime<Utc>) -> Vec<OverlapWarning> {
        let warnings =
            check_static_overlaps(&self.queue, &event.title, event.time, event.end_time());
        self.queue.push(QueueItem::Event(event));
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));
        displace_non_static_conflicts(&mut self.queue, now);
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));
        warnings
    }

    /// Updates an existing action in-place. Non-static actions that were
    /// consecutive with the old end time are re-anchored to the new end time;
    /// anything else that now conflicts with an anchor is displaced.
    /// Returns warnings for immovable conflicts.
    pub fn update_queue_action(
        &mut self,
        mut updated: Action,
        now: DateTime<Utc>,
    ) -> Vec<OverlapWarning> {
        let old_end = self.queue.iter().find_map(|item| {
            if let QueueItem::Action(a) = item {
                if a.id == updated.id {
                    return Some(a.target.map(|t| t + action_effective_duration(a)));
                }
            }
            None
        });
        let old_end = old_end.flatten();

        self.queue.retain(|item| item.id() != updated.id);

        let trailing = match old_end {
            Some(old_end) => collect_trailing_chain(&mut self.queue, old_end),
            None => Vec::new(),
        };

        if updated.target.is_some() {
            updated.target_static = true;
        }

        let warnings = match updated.target {
            Some(t) => check_static_overlaps(
                &self.queue,
                &updated.title,
                t,
                t + action_effective_duration(&updated),
            ),
            None => Vec::new(),
        };

        self.queue.push(QueueItem::Action(updated.clone()));
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));

        if let Some(new_end) = updated
            .target
            .map(|t| t + action_effective_duration(&updated))
        {
            reanchor_trailing_chain(&mut self.queue, trailing, new_end);
            self.queue
                .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));
        }

        displace_non_static_conflicts(&mut self.queue, now);
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));

        warnings
    }

    /// Updates an existing event in-place. Non-static actions consecutive with
    /// the old end time are re-anchored to the new end time; anything else that
    /// now conflicts with an anchor is displaced. Returns warnings for
    /// immovable conflicts.
    pub fn update_queue_event(
        &mut self,
        updated: Event,
        now: DateTime<Utc>,
    ) -> Vec<OverlapWarning> {
        let old_end = self.queue.iter().find_map(|item| {
            if let QueueItem::Event(e) = item {
                if e.id == updated.id {
                    return Some(e.end_time());
                }
            }
            None
        });

        self.queue.retain(|item| item.id() != updated.id);

        let trailing = match old_end {
            Some(old_end) => collect_trailing_chain(&mut self.queue, old_end),
            None => Vec::new(),
        };

        let warnings = check_static_overlaps(
            &self.queue,
            &updated.title,
            updated.time,
            updated.end_time(),
        );
        let new_end = updated.end_time();
        self.queue.push(QueueItem::Event(updated));
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));

        reanchor_trailing_chain(&mut self.queue, trailing, new_end);
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));

        displace_non_static_conflicts(&mut self.queue, now);
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));

        warnings
    }

    pub fn promote_action(&mut self, id: Uuid, now: DateTime<Utc>) -> bool {
        let pos = match self.backlog.iter().position(|a| a.id == id) {
            Some(pos) => pos,
            None => return false,
        };
        let mut action = self.backlog.remove(pos);
        let duration = action_effective_duration(&action);
        let target = next_available_slot(&self.queue, now, duration);
        action.target = Some(target);
        action.target_static = false;
        self.queue.push(QueueItem::Action(action));
        self.queue
            .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));
        true
    }

    pub fn refresh(&mut self, now: DateTime<Utc>) -> RefreshResult {
        let mut result = RefreshResult::default();

        // --- Promote backlog actions whose naive_date has arrived ---
        //
        // A backlog action with a naive_date set is scheduled to be promoted
        // to the queue once that date is today or in the past (in local time).
        // We remove it from the backlog, assign it the next available slot,
        // and record it in `result.promoted_actions` so callers can persist it.
        let today_local = Local::now().date_naive();
        let mut to_promote: Vec<Action> = Vec::new();

        self.backlog.retain(|action| {
            if let Some(naive_date) = action.naive_date {
                if naive_date <= today_local {
                    to_promote.push(action.clone());
                    return false;
                }
            }
            true
        });

        for mut action in to_promote {
            let duration = action_effective_duration(&action);
            let target = next_available_slot(&self.queue, now, duration);
            action.target = Some(target);
            action.naive_date = None;
            action.target_static = false;
            result.promoted_actions.push(action.clone());
            self.queue.push(QueueItem::Action(action));
        }
        if !result.promoted_actions.is_empty() {
            self.queue
                .sort_by_key(|item| item.time().unwrap_or(DateTime::<Utc>::MAX_UTC));
        }

        let mut next_events: Vec<Event> = Vec::new();

        self.queue.retain(|item| match item {
            QueueItem::Event(event) => {
                if event.is_expired(now) {
                    if let Some(next) = event.next_recurrence() {
                        next_events.push(next);
                    }
                    false
                } else {
                    true
                }
            }
            QueueItem::Action(_) => true,
        });

        for next in next_events {
            result.new_events.push(next.clone());
            self.queue.push(QueueItem::Event(next));
        }

        let mut to_reschedule: Vec<Action> = Vec::new();

        self.queue.retain(|item| match item {
            QueueItem::Action(action) if !action.target_static => {
                if action.target.is_none() || is_missed(action, now) {
                    to_reschedule.push(action.clone());
                    false
                } else {
                    true
                }
            }
            _ => true,
        });

        to_reschedule.sort_by_key(|a| a.target.map(|t| t.timestamp()).unwrap_or(i64::MAX));

        let intervals = event_intervals(&self.queue);
        let groups = group_consecutive(to_reschedule);
        let mut cursor = now;

        for group in groups {
            let total_duration: Duration = group
                .iter()
                .map(action_effective_duration)
                .fold(Duration::zero(), |acc, d| acc + d);

            let slot_start = find_free_slot(cursor, total_duration, &intervals);

            let base_offset: Duration = match group.first().and_then(|a| a.target) {
                Some(original_first_target) => slot_start - original_first_target,
                None => Duration::zero(),
            };

            let mut group_cursor = slot_start;

            for mut action in group {
                let new_target = match action.target {
                    Some(original_target) => original_target + base_offset,
                    None => group_cursor,
                };

                group_cursor = new_target + action_effective_duration(&action);
                action.target = Some(new_target);
                self.queue.push(QueueItem::Action(action));
            }

            cursor = slot_start + total_duration;
        }

        self.queue
            .sort_by_key(|item| item.time().map(|t| t.timestamp()).unwrap_or(i64::MAX));

        result
    }

    pub fn expedite_actions(&mut self, now: DateTime<Utc>) {
        expedite_queue(&mut self.queue, now);
        self.queue
            .sort_by_key(|item| item.time().map(|t| t.timestamp()).unwrap_or(i64::MAX));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionCompletion {
    pub id: Uuid,
    pub action_id: Uuid,
    pub lineage_id: Uuid,
    pub completed_at: DateTime<Utc>,
    pub notes: Option<String>,
}

impl ActionCompletion {
    pub fn new(action: &Action) -> Self {
        Self {
            id: Uuid::now_v7(),
            action_id: action.id,
            lineage_id: action.lineage_id,
            completed_at: Utc::now(),
            notes: None,
        }
    }

    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }
}
