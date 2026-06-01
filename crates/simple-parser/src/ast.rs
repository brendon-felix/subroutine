use chrono::{DateTime, Duration, NaiveDate, Utc, Weekday};
use serde::Serialize;

/// Which entity this draft should be built into.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum EntityKind {
    Action,
    Event,
    RoutineStep,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[allow(dead_code)] // Priority variants are defined for future sigil parsing
pub enum Priority {
    Low,
    Medium,
    High,
}

// ---------------------------------------------------------------------------
// WeekdaySet — compact bitmask representation
// ---------------------------------------------------------------------------

/// A set of weekdays stored as a `u8` bitmask.
///
/// Bit layout (LSB = 0):
///   bit 0 → Monday
///   bit 1 → Tuesday
///   bit 2 → Wednesday
///   bit 3 → Thursday
///   bit 4 → Friday
///   bit 5 → Saturday
///   bit 6 → Sunday
///
/// This avoids the `Ord` requirement that `BTreeSet<Weekday>` would impose,
/// while remaining compact, `Copy`, and trivially comparable.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
pub struct WeekdaySet(pub u8);

fn weekday_bit(day: Weekday) -> u8 {
    match day {
        Weekday::Mon => 1 << 0,
        Weekday::Tue => 1 << 1,
        Weekday::Wed => 1 << 2,
        Weekday::Thu => 1 << 3,
        Weekday::Fri => 1 << 4,
        Weekday::Sat => 1 << 5,
        Weekday::Sun => 1 << 6,
    }
}

#[allow(dead_code)] // All members used by tests and future % sigil handler
impl WeekdaySet {
    pub const EMPTY: Self = Self(0);

    pub fn new(days: impl IntoIterator<Item = Weekday>) -> Self {
        let mut bits = 0u8;
        for d in days {
            bits |= weekday_bit(d);
        }
        Self(bits)
    }

    /// Mon–Fri
    pub fn weekdays() -> Self {
        Self::new([
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
        ])
    }

    /// Sat–Sun
    pub fn weekends() -> Self {
        Self::new([Weekday::Sat, Weekday::Sun])
    }

    /// All seven days
    pub fn every_day() -> Self {
        Self::new([
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ])
    }

    pub fn contains(&self, day: Weekday) -> bool {
        self.0 & weekday_bit(day) != 0
    }

    pub fn len(&self) -> usize {
        self.0.count_ones() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Iterate over the days in the set, Mon → Sun order.
    #[allow(dead_code)] // Provided for future callers
    pub fn iter(&self) -> impl Iterator<Item = Weekday> + '_ {
        [
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ]
        .into_iter()
        .filter(|d| self.contains(*d))
    }
}

// ---------------------------------------------------------------------------
// RecurrenceSpec
// ---------------------------------------------------------------------------

#[allow(dead_code)] // Variants used by tests and future % sigil handler
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum RecurrenceSpec {
    /// Repeat every N days regardless of which day of the week it is.
    EveryDays(i64),
    /// Repeat every N weeks.
    EveryWeeks(i64),
    /// Repeat every N calendar months.
    EveryMonths(i64),
    /// Repeat every N years.
    EveryYears(i64),
    /// Repeat on a specific day-of-month every month (1–31).
    ///
    /// e.g. `OnMonthDay(15)` → "every 15th", "every month on the 15th".
    /// For months shorter than the stored day the scheduler should use the
    /// last day of that month.
    OnMonthDay(u32),
    /// Repeat on a specific set of weekdays (e.g. Mon/Wed/Fri, weekdays,
    /// weekends, or any custom combination).
    OnWeekdays(WeekdaySet),
}

#[allow(dead_code)] // All methods used by tests and future % sigil handler
impl RecurrenceSpec {
    /// Daily — every 1 day.
    pub fn daily() -> Self {
        Self::EveryDays(1)
    }

    /// Weekly — every 7 days (1 week).
    pub fn weekly() -> Self {
        Self::EveryWeeks(1)
    }

    /// Monthly — every 1 calendar month.
    pub fn monthly() -> Self {
        Self::EveryMonths(1)
    }

    /// Yearly — every 1 year.
    pub fn yearly() -> Self {
        Self::EveryYears(1)
    }

    /// Mon–Fri only.
    pub fn weekdays() -> Self {
        Self::OnWeekdays(WeekdaySet::weekdays())
    }

    /// Sat–Sun only.
    pub fn weekends() -> Self {
        Self::OnWeekdays(WeekdaySet::weekends())
    }
}

// ---------------------------------------------------------------------------
// WhenSpec — what the parser resolved for the time/date clause
// ---------------------------------------------------------------------------

/// The semantic role of a span of input text, used for syntax highlighting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum HighlightKind {
    Title,
    When,
    Duration,
    Recurrence,
    Tag,
    Location,
    People,
    Priority,
    Sigil, // the bare sigil character (@, ~, %, #, &, !)
}

/// The result of parsing a date/time expression.
///
/// - `DateTime` — a full, timezone-aware moment (used for events and for
///   actions that had an explicit time specified).
/// - `NaiveDate` — a calendar date with no time component (used for actions
///   whose user input contained only a date, e.g. "tomorrow" or "friday").
///   The actual scheduling time is left floating and will be assigned later.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum WhenSpec {
    DateTime(DateTime<Utc>),
    NaiveDate(NaiveDate),
}

impl WhenSpec {
    /// Returns the full datetime if this is a `DateTime` variant.
    pub fn as_datetime(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::DateTime(dt) => Some(*dt),
            Self::NaiveDate(_) => None,
        }
    }

    /// Returns the naive date regardless of variant.
    pub fn date(&self) -> NaiveDate {
        match self {
            Self::DateTime(dt) => dt.date_naive(),
            Self::NaiveDate(d) => *d,
        }
    }

    /// Returns `true` if this spec carries an explicit time component.
    pub fn has_time(&self) -> bool {
        matches!(self, Self::DateTime(_))
    }

    /// Unwraps the inner `DateTime<Utc>`, panicking if this is a `NaiveDate`.
    ///
    /// Intended for use in tests where the caller knows the spec must be a
    /// full datetime.
    pub fn unwrap_datetime(self) -> DateTime<Utc> {
        match self {
            Self::DateTime(dt) => dt,
            Self::NaiveDate(d) => {
                panic!("called unwrap_datetime() on a WhenSpec::NaiveDate({d})")
            }
        }
    }

    /// Unwraps the inner `NaiveDate`, panicking if this is a `DateTime`.
    ///
    /// Intended for use in tests where the caller knows the spec must be
    /// a date-only value.
    pub fn unwrap_naive_date(self) -> NaiveDate {
        match self {
            Self::NaiveDate(d) => d,
            Self::DateTime(dt) => {
                panic!("called unwrap_naive_date() on a WhenSpec::DateTime({dt})")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ParseDraft
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ParseDraft {
    pub raw: String,
    pub kind: EntityKind,
    pub title: String,

    pub when: Option<WhenSpec>,
    pub duration: Option<Duration>,
    pub recurrence: Option<RecurrenceSpec>,

    pub priority: Option<Priority>,
    pub location: Option<String>,
    pub tags: Vec<String>,
    pub people: Vec<String>,

    pub content: Option<String>,
    pub warnings: Vec<String>,
    /// Byte-range → highlight kind for each recognized span of the raw input.
    /// Populated by the parser; can be used for syntax coloring.
    pub highlights: Vec<(std::ops::Range<usize>, HighlightKind)>,
}

impl ParseDraft {
    pub fn new(kind: EntityKind, raw: impl Into<String>) -> Self {
        Self {
            raw: raw.into(),
            kind,
            title: String::new(),
            when: None,
            duration: None,
            recurrence: None,
            priority: None,
            location: None,
            tags: Vec::new(),
            people: Vec::new(),
            content: None,
            warnings: Vec::new(),
            highlights: Vec::new(),
        }
    }
}
