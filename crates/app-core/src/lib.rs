use std::collections::HashSet;

mod action;
mod event;
mod mental_state;
mod pipeline;
mod routine;
mod scheduler;
mod scoring;
mod subroutine;

pub use action::*;
pub use event::*;
pub use mental_state::*;
pub use pipeline::*;
pub use routine::*;
pub use scheduler::*;
pub use scoring::*;
pub use subroutine::*;

use bitflags::bitflags;
use chrono::{DateTime, Duration, NaiveTime, Timelike, Utc};
use uuid::Uuid;

/// A snapshot of the user's current situation, passed into constraint checking,
/// scoring, and pipeline refresh. Constructed fresh from live app state each time
/// it is needed.
#[derive(Debug, Clone)]
pub struct Context {
    pub current_time: DateTime<Utc>,
    pub mental_state: MentalState,
}

impl Context {
    pub fn new(mental_state: MentalState) -> Self {
        Self {
            current_time: Utc::now(),
            mental_state,
        }
    }
}

/// Describes how and when a saved type should be re-instantiated after completion.
#[derive(Debug, Clone)]
pub struct RecurrenceRule {
    /// Minimum time that must elapse after completion before the next instance
    /// is created or suggested. None means no minimum — re-instantiate immediately.
    pub min_interval: Option<Duration>,
    /// Maximum time allowed between completions. Once this elapses the action
    /// is considered overdue and its urgency score rises. None means no maximum.
    pub max_interval: Option<Duration>,
    /// If true, a new concrete instance is automatically created and added to the
    /// pipeline backlog when the previous one is completed.
    /// If false, the user must manually re-instantiate.
    pub auto_reschedule: bool,
}

/// An Actionable is something that can be added to The Pipeline.
pub trait Actionable {
    fn id(&self) -> Uuid;
    fn title(&self) -> &str;
    fn content(&self) -> Option<&str>;
    fn created_time(&self) -> DateTime<Utc>;
    fn target_time(&self) -> Option<DateTime<Utc>>;
    /// Returns the concrete constraints for this entry. For types backed by
    /// `SavedConstraints` (Routine, Subroutine), this materializes the saved
    /// constraints to the current moment.
    fn constraints(&self) -> Constraints;
    fn actions(&self) -> Vec<Uuid>;
}

/// Checks whether an actionable's constraints are satisfied given the current context.
/// Returns `true` if all constraints pass — the entry is eligible to be surfaced.
///
/// Deadline is intentionally not checked here — a past deadline means the action is
/// overdue and should score higher, not be hidden.
///
/// Note: `valid_times_of_day` currently uses the UTC time component of `context.current_time`.
/// This should use local time once timezone support is added to `Context`.
pub fn check_constraints(
    actionable: &dyn Actionable,
    context: &Context,
    completed_ids: &HashSet<Uuid>,
) -> bool {
    let constraints = actionable.constraints();

    if let Some(valid_times) = constraints.valid_times_of_day {
        let current_time_of_day = context.current_time.naive_utc().time();
        if !valid_times.includes(current_time_of_day) {
            return false;
        }
    }

    if let Some(earliest_start) = constraints.earliest_start {
        if context.current_time < earliest_start {
            return false;
        }
    }

    if let Some(spoons_required) = constraints.spoons_required {
        if context.mental_state.remaining_spoons < spoons_required {
            return false;
        }
    }

    if !constraints
        .dependencies
        .iter()
        .all(|dep| completed_ids.contains(dep))
    {
        return false;
    }

    true
}

/// Template constraints stored on saved types (SavedAction, SavedEvent, Routine, Subroutine).
/// Unlike `Constraints`, deadlines are expressed as times-of-day rather than absolute datetimes,
/// and there is no `earliest_start` (which is always context-specific).
#[derive(Debug, Clone)]
pub struct SavedConstraints {
    pub valid_times_of_day: Option<TimesOfDay>,
    /// Time-of-day deadline. Materialized to the next upcoming occurrence of this
    /// wall-clock time when `materialize()` is called.
    pub deadline: Option<NaiveTime>,
    pub minimum_duration: Option<Duration>,
    pub transition_time: Option<Duration>,
    pub spoons_required: Option<u32>,
    pub dependencies: Vec<Uuid>,
}

impl SavedConstraints {
    /// Converts this template's constraints into concrete `Constraints` anchored to `now`.
    ///
    /// - `deadline: Option<NaiveTime>` → the next upcoming wall-clock occurrence of that time.
    ///   If the time has already passed today (in UTC), it resolves to tomorrow's occurrence.
    /// - `valid_times_of_day`, `minimum_duration`, `transition_time`, `spoons_required`,
    ///   `dependencies` — carried over directly.
    /// - `earliest_start` — always `None` on the result (not representable in SavedConstraints).
    pub fn materialize(&self, now: DateTime<Utc>) -> Constraints {
        let deadline = self.deadline.map(|time| {
            let today = now.date_naive();
            let candidate = today.and_time(time).and_utc();
            if candidate > now {
                candidate
            } else {
                (today + Duration::days(1)).and_time(time).and_utc()
            }
        });

        Constraints {
            valid_times_of_day: self.valid_times_of_day,
            earliest_start: None,
            deadline,
            minimum_duration: self.minimum_duration,
            transition_time: self.transition_time,
            spoons_required: self.spoons_required,
            dependencies: self.dependencies.clone(),
        }
    }
}

impl Default for SavedConstraints {
    fn default() -> Self {
        Self {
            valid_times_of_day: None,
            deadline: None,
            minimum_duration: None,
            transition_time: None,
            spoons_required: None,
            dependencies: Vec::new(),
        }
    }
}

/// Constraints that determine how and when an Actionable is surfaced by the Pipeline.
#[derive(Debug, Clone, Default)]
pub struct Constraints {
    pub valid_times_of_day: Option<TimesOfDay>,
    pub earliest_start: Option<DateTime<Utc>>,
    pub deadline: Option<DateTime<Utc>>,
    pub minimum_duration: Option<Duration>,
    pub transition_time: Option<Duration>,
    pub spoons_required: Option<u32>,
    pub dependencies: Vec<Uuid>,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct TimesOfDay: u8 {
        /// 4:00am - 6:59am
        const EARLY_MORNING = 0b00000001;
        /// 7:00am - 10:59am
        const MORNING       = 0b00000010;
        /// 11:00am - 12:59pm
        const MIDDAY        = 0b00000100;
        /// 1:00pm - 3:59pm
        const AFTERNOON     = 0b00001000;
        /// 4:00pm - 7:59pm
        const EVENING       = 0b00010000;
        /// 8:00pm - 11:59pm
        const NIGHT         = 0b00100000;
        /// 12:00am - 3:59am
        const LATE_NIGHT    = 0b01000000;
    }
}

impl TimesOfDay {
    pub fn includes(&self, time: NaiveTime) -> bool {
        let hour = time.hour();
        match hour {
            4..=6 => self.contains(TimesOfDay::EARLY_MORNING),
            7..=10 => self.contains(TimesOfDay::MORNING),
            11..=12 => self.contains(TimesOfDay::MIDDAY),
            13..=15 => self.contains(TimesOfDay::AFTERNOON),
            16..=19 => self.contains(TimesOfDay::EVENING),
            20..=23 => self.contains(TimesOfDay::NIGHT),
            0..=3 => self.contains(TimesOfDay::LATE_NIGHT),
            _ => false,
        }
    }
}

impl From<NaiveTime> for TimesOfDay {
    fn from(time: NaiveTime) -> Self {
        let hour = time.hour();
        match hour {
            4..=6 => TimesOfDay::EARLY_MORNING,
            7..=10 => TimesOfDay::MORNING,
            11..=12 => TimesOfDay::MIDDAY,
            13..=15 => TimesOfDay::AFTERNOON,
            16..=19 => TimesOfDay::EVENING,
            20..=23 => TimesOfDay::NIGHT,
            0..=3 => TimesOfDay::LATE_NIGHT,
            _ => TimesOfDay::empty(),
        }
    }
}
