use chrono::{DateTime, Months, Utc};
use serde::{Deserialize, Serialize};

/// The granularity of a [`RecurrenceRule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecurrenceUnit {
    Minutes,
    Hours,
    Days,
    Weeks,
    Months,
    Years,
}

impl RecurrenceUnit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minutes => "minutes",
            Self::Hours => "hours",
            Self::Days => "days",
            Self::Weeks => "weeks",
            Self::Months => "months",
            Self::Years => "years",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "minutes" => Some(Self::Minutes),
            "hours" => Some(Self::Hours),
            "days" => Some(Self::Days),
            "weeks" => Some(Self::Weeks),
            "months" => Some(Self::Months),
            "years" => Some(Self::Years),
            _ => None,
        }
    }

    /// Human-readable singular label (e.g. for UI display).
    pub fn label(self) -> &'static str {
        match self {
            Self::Minutes => "minute",
            Self::Hours => "hour",
            Self::Days => "day",
            Self::Weeks => "week",
            Self::Months => "month",
            Self::Years => "year",
        }
    }
}

/// A recurrence rule expressed as a count + calendar unit.
///
/// This preserves the semantic intent of "every 2 weeks" or "every 3 months"
/// rather than collapsing it into an ambiguous seconds count. Month and year
/// arithmetic is calendar-correct via [`chrono::Months`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurrenceRule {
    pub count: u32,
    pub unit: RecurrenceUnit,
}

impl RecurrenceRule {
    // ── Constructors ────────────────────────────────────────────────────────

    pub fn minutes(count: u32) -> Self {
        Self {
            count,
            unit: RecurrenceUnit::Minutes,
        }
    }

    pub fn hours(count: u32) -> Self {
        Self {
            count,
            unit: RecurrenceUnit::Hours,
        }
    }

    pub fn days(count: u32) -> Self {
        Self {
            count,
            unit: RecurrenceUnit::Days,
        }
    }

    pub fn weeks(count: u32) -> Self {
        Self {
            count,
            unit: RecurrenceUnit::Weeks,
        }
    }

    pub fn months(count: u32) -> Self {
        Self {
            count,
            unit: RecurrenceUnit::Months,
        }
    }

    pub fn years(count: u32) -> Self {
        Self {
            count,
            unit: RecurrenceUnit::Years,
        }
    }

    // ── Scheduling ──────────────────────────────────────────────────────────

    /// Return the next occurrence after `from` according to this rule.
    ///
    /// - Minutes / hours / days / weeks use exact [`chrono::Duration`] math.
    /// - Months and years use [`chrono::Months`] so "every month" on the 31st
    ///   correctly clamps to the last day of shorter months.
    /// - Falls back to `from` unchanged only if the date would overflow the
    ///   representable range (extremely unlikely in practice).
    pub fn next_after(self, from: DateTime<Utc>) -> DateTime<Utc> {
        let n = self.count as i64;
        match self.unit {
            RecurrenceUnit::Minutes => from + chrono::Duration::minutes(n),
            RecurrenceUnit::Hours => from + chrono::Duration::hours(n),
            RecurrenceUnit::Days => from + chrono::Duration::days(n),
            RecurrenceUnit::Weeks => from + chrono::Duration::weeks(n),
            RecurrenceUnit::Months => from
                .checked_add_months(Months::new(self.count))
                .unwrap_or(from),
            RecurrenceUnit::Years => from
                .checked_add_months(Months::new(self.count.saturating_mul(12)))
                .unwrap_or(from),
        }
    }

    // ── Display helpers ─────────────────────────────────────────────────────

    /// A compact human-readable description, e.g. "every 2 weeks".
    pub fn describe(self) -> String {
        if self.count == 1 {
            format!("every {}", self.unit.label())
        } else {
            format!("every {} {}s", self.count, self.unit.label())
        }
    }
}

impl std::fmt::Display for RecurrenceRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.describe())
    }
}
