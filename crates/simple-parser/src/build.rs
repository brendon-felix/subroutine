use anyhow::{Result, bail};
use chrono::Duration;
use serde::Serialize;
use simple_core::{Action, Event};

use crate::ast::{ParseDraft, RecurrenceSpec, WhenSpec};

#[derive(Debug, Clone, Copy)]
pub enum BuildTarget {
    Action,
    Event,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum BuiltEntity {
    Action(Action),
    Event(Event),
}

pub fn build_entity(draft: &ParseDraft, target: BuildTarget) -> Result<BuiltEntity> {
    match target {
        BuildTarget::Action => Ok(BuiltEntity::Action(build_action(draft))),
        BuildTarget::Event => Ok(BuiltEntity::Event(build_event(draft)?)),
    }
}

fn build_action(draft: &ParseDraft) -> Action {
    let mut action = Action::new(&draft.title);

    if let Some(content) = &draft.content {
        action = action.with_content(content.clone());
    }

    match &draft.when {
        // Full datetime → schedule the action (queue it with a target time).
        Some(WhenSpec::DateTime(dt)) => {
            action = action.with_target(*dt, true);
        }
        // Date only → place the action in the backlog with a floating date hint.
        // The pipeline will promote it once the date arrives.
        Some(WhenSpec::NaiveDate(date)) => {
            action = action.with_naive_date(*date);
        }
        // No time at all → pure backlog item, no date hint.
        None => {}
    }

    if let Some(duration) = draft.duration {
        action = action.with_duration(duration);
    }

    if let Some(recurrence) = recurrence_to_duration(draft.recurrence.as_ref()) {
        action = action.with_recurrence(recurrence);
    }

    action
}

fn build_event(draft: &ParseDraft) -> Result<Event> {
    // Events always need a concrete datetime. A NaiveDate (date-only) is
    // accepted and resolved to that date at the default time (09:00 local),
    // which the parser already converts to UTC via `date_at`. In practice,
    // `try_nl_when` for events always produces `WhenSpec::DateTime`, so the
    // NaiveDate arm here is a safety fallback.
    let time = match &draft.when {
        Some(WhenSpec::DateTime(dt)) => *dt,
        Some(WhenSpec::NaiveDate(_)) => {
            bail!("event build requires a concrete @ time; a date-only specifier is not enough");
        }
        None => {
            bail!("event build requires a concrete @ time");
        }
    };

    let mut event = Event::new(&draft.title, time);

    if let Some(content) = &draft.content {
        event = event.with_content(content.clone());
    }

    if let Some(duration) = draft.duration {
        event = event.with_duration(duration);
    }

    if let Some(recurrence) = recurrence_to_duration(draft.recurrence.as_ref()) {
        event = event.with_recurrence(recurrence);
    }

    Ok(event)
}

/// Convert a [`RecurrenceSpec`] into a [`Duration`] for use with
/// `simple_core` entities.
///
/// Notes on approximations:
/// - `EveryMonths(n)` uses 30 days per month. True calendar-month advancement
///   must be handled by the scheduler using the stored `RecurrenceSpec`.
/// - `EveryYears(n)` uses 365 days per year for the same reason.
/// - `OnMonthDay` ticks monthly (30-day approximation); the scheduler is
///   responsible for snapping to the correct day-of-month.
/// - `OnWeekdays` ticks daily; the scheduler checks whether the current day
///   is in the set.
pub fn recurrence_to_duration(spec: Option<&RecurrenceSpec>) -> Option<Duration> {
    match spec {
        Some(RecurrenceSpec::EveryDays(n)) => Some(Duration::days(*n)),
        Some(RecurrenceSpec::EveryWeeks(n)) => Some(Duration::weeks(*n)),
        Some(RecurrenceSpec::EveryMonths(n)) => Some(Duration::days(n * 30)),
        Some(RecurrenceSpec::EveryYears(n)) => Some(Duration::days(n * 365)),
        Some(RecurrenceSpec::OnMonthDay(_)) => Some(Duration::days(30)),
        Some(RecurrenceSpec::OnWeekdays(_)) => Some(Duration::days(1)),
        None => None,
    }
}
