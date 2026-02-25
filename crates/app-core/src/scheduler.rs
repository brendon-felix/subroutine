use chrono::{DateTime, Utc};

use crate::{Action, Event, SavedAction, SavedEvent};

/// Returns true if a new concrete instance of `saved` should be created now,
/// given when the last instance was completed.
pub fn should_reschedule(
    saved: &SavedAction,
    last_completed: DateTime<Utc>,
    now: DateTime<Utc>,
) -> bool {
    let recurrence = match &saved.recurrence {
        Some(r) => r,
        None => return false,
    };
    if !recurrence.auto_reschedule {
        return false;
    }
    if let Some(min_interval) = recurrence.min_interval {
        if now - last_completed < min_interval {
            return false;
        }
    }
    true
}

/// Creates a new concrete Action from the saved action, materialized to the current time.
/// The caller is responsible for adding it to the pipeline and persisting it.
pub fn reschedule_action(saved: &SavedAction) -> Action {
    saved.instantiate()
}

/// Returns true if a new concrete Event instance of `saved` should be created now,
/// given when the last instance was completed.
pub fn should_reschedule_event(
    saved: &SavedEvent,
    last_completed: DateTime<Utc>,
    now: DateTime<Utc>,
) -> bool {
    let recurrence = match &saved.recurrence {
        Some(r) => r,
        None => return false,
    };
    if !recurrence.auto_reschedule {
        return false;
    }
    if let Some(min_interval) = recurrence.min_interval {
        if now - last_completed < min_interval {
            return false;
        }
    }
    true
}

/// Creates a new concrete Event from the saved event, scheduled at `time`.
/// The caller is responsible for adding it to the pipeline and persisting it.
pub fn reschedule_event(saved: &SavedEvent, time: DateTime<Utc>) -> Event {
    saved.instantiate(time)
}
