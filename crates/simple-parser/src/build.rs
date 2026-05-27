use anyhow::{Result, bail};
use serde::Serialize;
use simple_core::{Action, ActionState, ActionTarget, Event, RecurrenceRule};

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
            let target = ActionTarget {
                time: *dt,
                is_static: true,
            };
            action = action.with_state(ActionState::Queued(target));
        }
        // Date only → place the action in the backlog with a floating date hint.
        // The pipeline will promote it once the date arrives.
        Some(WhenSpec::NaiveDate(date)) => {
            action = action.with_state(ActionState::Backlogged(Some(*date)));
        }
        // No time at all → pure backlog item, no date hint.
        None => {}
    }

    if let Some(duration) = draft.duration {
        action = action.with_duration(duration);
    }

    if let Some(rule) = recurrence_to_rule(draft.recurrence.as_ref()) {
        action = action.with_recurrence(rule);
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

    if let Some(rule) = recurrence_to_rule(draft.recurrence.as_ref()) {
        event = event.with_recurrence(rule);
    }

    Ok(event)
}

/// Convert a [`RecurrenceSpec`] into a [`RecurrenceRule`] for use with
/// `simple_core` entities.
///
/// The mapping preserves semantic intent so that calendar-correct arithmetic
/// is used for months and years (via [`RecurrenceRule::next_after`]):
/// - `OnMonthDay` → monthly rule (the scheduler snaps to the correct day).
/// - `OnWeekdays` → daily rule (the scheduler filters by the weekday set).
pub fn recurrence_to_rule(spec: Option<&RecurrenceSpec>) -> Option<RecurrenceRule> {
    match spec {
        Some(RecurrenceSpec::EveryDays(n)) => Some(RecurrenceRule::days(*n as u32)),
        Some(RecurrenceSpec::EveryWeeks(n)) => Some(RecurrenceRule::weeks(*n as u32)),
        Some(RecurrenceSpec::EveryMonths(n)) => Some(RecurrenceRule::months(*n as u32)),
        Some(RecurrenceSpec::EveryYears(n)) => Some(RecurrenceRule::years(*n as u32)),
        Some(RecurrenceSpec::OnMonthDay(_)) => Some(RecurrenceRule::months(1)),
        Some(RecurrenceSpec::OnWeekdays(_)) => Some(RecurrenceRule::days(1)),
        None => None,
    }
}
