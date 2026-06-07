use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Action, ActionState, ActionTarget, Event, Routine};

pub const DEFAULT_ACTION_DURATION: Duration = Duration::minutes(5);
// const CONSECUTIVE_GAP_THRESHOLD: Duration = Duration::minutes(5);
// const SEMI_CONSECUTIVE_GAP_THRESHOLD: Duration = Duration::minutes(30);
pub const EXPEDITE_HORIZON: Duration = Duration::hours(6);

fn action_effective_duration(action: &Action) -> Duration {
    action.duration.unwrap_or(DEFAULT_ACTION_DURATION)
}

fn _quantize_floor(dt: DateTime<Utc>, segment_secs: i64) -> DateTime<Utc> {
    let total_seconds = dt.timestamp();
    let remainder = total_seconds.rem_euclid(segment_secs);
    DateTime::from_timestamp(total_seconds - remainder, 0).unwrap_or(dt)
}

/// Round `dt` down to the nearest 5-minute boundary.
pub fn quantize_floor(dt: DateTime<Utc>) -> DateTime<Utc> {
    _quantize_floor(dt, 300)
}

fn _quantize(dt: DateTime<Utc>, segment_secs: i64) -> DateTime<Utc> {
    let total_seconds = dt.timestamp();
    let step = segment_secs;
    let remainder = total_seconds.rem_euclid(step);
    let target = if remainder == 0 {
        total_seconds
    } else {
        total_seconds + (step - remainder)
    };
    DateTime::from_timestamp(target, 0).unwrap_or(dt)
}

pub fn quantize(dt: DateTime<Utc>) -> DateTime<Utc> {
    _quantize(dt, 300) // 5 minutes
}

/// Round  up to the nearest 5-minute boundary.
pub fn quantize_duration(duration: Duration) -> Duration {
    let total_seconds = duration.num_seconds();
    let step: i64 = 300; // 5 minutes
    let remainder = total_seconds.rem_euclid(step);
    if remainder == 0 {
        duration
    } else {
        duration + Duration::seconds(step - remainder)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnyItem {
    Action(Action),
    Event(Event),
    Routine(Routine),
}

impl AnyItem {
    pub fn title(&self) -> &str {
        match self {
            AnyItem::Action(a) => &a.title,
            AnyItem::Event(e) => &e.title,
            AnyItem::Routine(r) => &r.title,
        }
    }

    pub fn id(&self) -> Uuid {
        match self {
            AnyItem::Action(a) => a.id,
            AnyItem::Event(e) => e.id,
            AnyItem::Routine(r) => r.id,
        }
    }

    pub fn truncated_id(&self) -> u64 {
        // use second half of v7 UUID (timestamp based)
        match self {
            AnyItem::Action(a) => a.id.as_u64_pair().1,
            AnyItem::Event(e) => e.id.as_u64_pair().1,
            AnyItem::Routine(r) => r.id.as_u64_pair().1,
        }
    }

    pub fn time(&self) -> Option<DateTime<Utc>> {
        match self {
            AnyItem::Action(a) => match a.state {
                ActionState::Queued(t) => Some(t.time),
                _ => None,
            },
            AnyItem::Event(e) => Some(e.time),
            AnyItem::Routine(_) => None,
        }
    }

    pub fn time_local(&self) -> Option<DateTime<Local>> {
        match self {
            AnyItem::Action(a) => match a.state {
                ActionState::Queued(t) => Some(t.time.with_timezone(&Local)),
                _ => None,
            },
            AnyItem::Event(e) => Some(e.time.with_timezone(&Local)),
            AnyItem::Routine(_) => None,
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        match self {
            AnyItem::Action(a) => a.duration,
            AnyItem::Event(e) => e.duration,
            AnyItem::Routine(r) => r.duration(),
        }
    }

    pub fn is_action(&self) -> bool {
        matches!(self, AnyItem::Action(_))
    }

    pub fn is_event(&self) -> bool {
        matches!(self, AnyItem::Event(_))
    }
}

pub fn filter_queued(actions: &[Action]) -> Vec<Action> {
    actions.iter().filter(|a| a.is_queued()).cloned().collect()
}

pub fn filter_backlogged(actions: &[Action]) -> Vec<Action> {
    actions
        .iter()
        .filter(|a| a.is_backlogged())
        .cloned()
        .collect()
}

// actions in the same subroutine or routine must be rescheduled or skipped as a single unit
//
// actions causing conflicts can be:
// - rescheduled to occur now or ASAP after event(s), displacing other actions/routines
// - rescheduled to occur at the end of the queue
// - demoted to the backlog
// - skipped

// enum ActionConflictResolution {
//     RescheduleAsap,
//     RescheduleToEnd,
//     DemoteToBacklog,
//     Skip,
// }

/// Returns the next available start time for a new floating (non-static) action.
///
/// The slot is placed after the end of all currently-floating queued actions, then
/// displaced forward around any static actions or events that overlap.
pub fn next_queue_slot(actions: &[Action], events: &[Event], now: DateTime<Utc>) -> DateTime<Utc> {
    // Find the latest end time of all currently-floating queued actions.
    let chain_end = actions
        .iter()
        .filter_map(|a| {
            if let ActionState::Queued(t) = a.state {
                if !t.is_static {
                    return Some(t.time + action_effective_duration(a));
                }
            }
            None
        })
        .max()
        .unwrap_or(now);

    // Anchors: future static actions + events. These don't move.
    let anchors = build_anchors(actions, events, Some(now));

    // Start from the later of now and the chain end, then skip any overlapping anchors.
    find_free_slot(
        quantize(chain_end.max(now)),
        DEFAULT_ACTION_DURATION,
        &anchors,
    )
}

/// Reschedule queued actions forward in time.
///
/// - **Future static actions** (`is_static && time >= now`) are left untouched and act as fixed
///   anchors that floating/missed actions are scheduled around.
/// - **Past static actions** (`is_static && time < now`) are treated as missed and rescheduled
///   from `now` like floating actions, but retain their `is_static` flag.
/// - **Floating actions** are always rescheduled from `now`.
///
/// All missed actions (past-static + floating) are sorted by their original scheduled time so
/// relative order is preserved and static actions are displaced as little as possible.
pub fn requeue_actions(actions: &[Action], events: &[Event], now: DateTime<Utc>) -> Vec<Action> {
    let now = quantize(now);

    // Collect actions that need rescheduling (floating + past-static), sorted by original time.
    let mut missed: Vec<Action> = actions
        .iter()
        .filter(|a| a.is_missed(now))
        .cloned()
        .collect();
    missed.sort_by_key(|a| match a.state {
        ActionState::Queued(t) => t.time,
        _ => unreachable!(),
    });

    // Anchors: only future static actions + events. These don't move.
    let anchors = build_anchors(actions, events, Some(now));

    let mut cursor = now;
    let mut updates = Vec::with_capacity(missed.len());

    for action in missed {
        let duration = action_effective_duration(&action);
        let start = find_free_slot(cursor, duration, &anchors);
        cursor = start + duration;
        let is_static = action.is_queued_static();
        updates.push(action.with_state(ActionState::Queued(ActionTarget {
            time: start,
            is_static,
        })));
    }

    updates
}

/// Reschedule floating queued actions backwards in time.
///
/// Starting from `horizon` and working backwards, each floating (non-static)
/// action is placed as early as possible while still fitting before its
/// successor.  Static actions and events act as immovable anchors.
///
/// Only actions whose new scheduled time is **earlier** than their current
/// time are included in the returned list (i.e. this function never delays
/// an action).
///
/// Actions are not moved before `now`.
pub fn expedite_actions(
    actions: &[Action],
    events: &[Event],
    now: DateTime<Utc>,
    horizon: DateTime<Utc>,
) -> Vec<Action> {
    let now = quantize(now);
    let horizon = quantize(horizon);

    // Only floating queued actions are candidates for expediting.
    let mut candidates: Vec<Action> = actions
        .iter()
        .filter(|a| a.is_queued_floating())
        .cloned()
        .collect();

    // Preserve relative order by sorting by current scheduled time.
    candidates.sort_by_key(|a| match a.state {
        ActionState::Queued(t) => t.time,
        _ => unreachable!(),
    });

    // Anchors: static queued actions + events (neither group moves).
    let anchors = build_anchors(actions, events, None);

    // Work backwards from the horizon, placing the last action first.
    let mut cursor = horizon;
    let mut updates: Vec<Action> = Vec::new();

    for action in candidates.iter().rev() {
        let duration = action_effective_duration(action);
        let start = find_free_slot_backward(cursor, duration, &anchors, now);
        // The next (earlier) action must end at or before this action's start.
        cursor = start;

        let old_time = match action.state {
            ActionState::Queued(t) => t.time,
            _ => unreachable!(),
        };

        // Only include this action if it actually moved earlier.
        if start < old_time {
            updates.push(action.clone().with_state(ActionState::Queued(ActionTarget {
                time: start,
                is_static: false,
            })));
        }
    }

    updates
}

/// Build a sorted list of `(start, end)` anchor intervals from static queued
/// actions and events.  Pass `Some(now)` to restrict static actions to those
/// scheduled at or after `now` (used by `requeue_actions`); pass `None` to
/// include all static actions (used by `expedite_actions`).
fn build_anchors(
    actions: &[Action],
    events: &[Event],
    future_only_from: Option<DateTime<Utc>>,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut v: Vec<(DateTime<Utc>, DateTime<Utc>)> = events
        .iter()
        .map(|e| {
            (
                e.time,
                e.time + e.duration.unwrap_or(DEFAULT_ACTION_DURATION),
            )
        })
        .chain(actions.iter().filter_map(|a| {
            if let ActionState::Queued(t) = a.state {
                if t.is_static {
                    if let Some(from) = future_only_from {
                        if t.time < from {
                            return None;
                        }
                    }
                    return Some((t.time, t.time + action_effective_duration(a)));
                }
            }
            None
        }))
        .collect();
    v.sort_by_key(|(start, _)| *start);
    v
}

/// Returns the latest start time such that `[start, start + duration)` fits
/// entirely before `cursor` without overlapping any anchor.  The result is
/// never earlier than `floor`.
fn find_free_slot_backward(
    cursor: DateTime<Utc>,
    duration: Duration,
    anchors: &[(DateTime<Utc>, DateTime<Utc>)],
    floor: DateTime<Utc>,
) -> DateTime<Utc> {
    let mut candidate_end = cursor;

    loop {
        let candidate_start = (candidate_end - duration).max(floor);

        // Find an anchor that overlaps [candidate_start, candidate_end).
        // Iterate in reverse so we find the latest conflicting anchor first.
        let conflict = anchors
            .iter()
            .rev()
            .find(|(a_start, a_end)| candidate_start < *a_end && candidate_end > *a_start);

        match conflict {
            None => return candidate_start,
            // Jump the candidate window to end just before the conflicting anchor,
            // quantized down to the nearest 5-minute boundary.
            Some((a_start, _)) => candidate_end = quantize_floor(*a_start),
        }
    }
}

/// Returns the earliest start time >= `earliest` where a block of `duration`
/// fits without overlapping any interval in `anchors` (sorted by start time).
fn find_free_slot(
    earliest: DateTime<Utc>,
    duration: Duration,
    anchors: &[(DateTime<Utc>, DateTime<Utc>)],
) -> DateTime<Utc> {
    let mut candidate = earliest;

    loop {
        let candidate_end = candidate + duration;

        // Find an anchor that overlaps the proposed window [candidate, candidate_end).
        let conflict = anchors
            .iter()
            .find(|(a_start, a_end)| candidate < *a_end && candidate_end > *a_start);

        match conflict {
            None => return candidate,
            // Jump past the conflicting anchor, quantized to the next 5-min boundary.
            Some((_, a_end)) => candidate = quantize(*a_end),
        }
    }
}
