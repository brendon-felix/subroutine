use chrono::{
    DateTime, Datelike, Days, Duration as ChronoDuration, Local, LocalResult, NaiveDate, Timelike,
};
use gpui::{App, Div, ParentElement, Pixels, Styled, div, prelude::FluentBuilder, px};
use gpui_component::{ActiveTheme, Colorize, h_flex};

pub(super) const DEFAULT_HOUR_HEIGHT: Pixels = px(280.);
pub enum TimeDivisionStyle {
    HourMinute,
    Hour,
    TimeOfDay,
    // MidnightDate,
    // Date,
    WeekDayDate,
    DateWeekStart,
    MonthName,
    // TimeOfYear,
    YearQuarter,
    Year,
}

impl TimeDivisionStyle {
    pub(super) fn label(&self, datetime: DateTime<Local>, minimal: bool, cx: &App) -> Div {
        match self {
            TimeDivisionStyle::HourMinute => match datetime.minute() {
                0 => hour_label(datetime, cx),
                _ => hour_minute_label(datetime, minimal, false, cx).text_xs(),
            },
            TimeDivisionStyle::Hour => hour_label(datetime, cx),
            TimeDivisionStyle::TimeOfDay => time_of_day_label(datetime, cx),
            // TimeDivisionStyle::MidnightDate => div(),
            // TimeDivisionStyle::Date => date_label(datetime, minimal, cx),
            TimeDivisionStyle::WeekDayDate => weekday_date_label(datetime, false, cx),
            TimeDivisionStyle::DateWeekStart => week_start_label(datetime, cx),
            TimeDivisionStyle::MonthName => month_label(datetime, cx),
            // TimeDivisionStyle::TimeOfYear => time_of_year_label(datetime, cx),
            TimeDivisionStyle::YearQuarter => year_quarter_label(datetime, cx),
            TimeDivisionStyle::Year => year_label(datetime, cx),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimeSubDivision {
    Minute,
    FiveMinutes,
    TenMinutes,
    QuarterHour,
    // HalfHour,
    Hour,
    EvenHour,
    // QuarterDay,
    HalfDay,
    Day,
    Week,
    Month,
    QuarterYear,
}

impl TimeSubDivision {
    pub(super) fn style(&self) -> TimeDivisionStyle {
        match self {
            TimeSubDivision::Minute => TimeDivisionStyle::HourMinute,
            TimeSubDivision::FiveMinutes => TimeDivisionStyle::HourMinute,
            TimeSubDivision::TenMinutes => TimeDivisionStyle::HourMinute,
            TimeSubDivision::QuarterHour => TimeDivisionStyle::HourMinute,
            // TimeSubDivision::HalfHour => TimeDivisionStyle::HourMinute,
            TimeSubDivision::Hour => TimeDivisionStyle::Hour,
            TimeSubDivision::EvenHour => TimeDivisionStyle::Hour,
            // TimeSubDivision::QuarterDay => TimeDivisionStyle::TimeOfDay,
            TimeSubDivision::HalfDay => TimeDivisionStyle::TimeOfDay,
            TimeSubDivision::Day => TimeDivisionStyle::WeekDayDate,
            TimeSubDivision::Week => TimeDivisionStyle::DateWeekStart,
            TimeSubDivision::Month => TimeDivisionStyle::MonthName,
            TimeSubDivision::QuarterYear => TimeDivisionStyle::YearQuarter,
        }
    }

    /// Returns the start of the next subdivision boundary after `start`.
    /// Sub-hour divisions use fixed durations (DST occurs on hour boundaries,
    /// so minutes are always exact). Day/Week/Month delegate to the calendar-aware
    /// `BaseTimeDivision` helpers so DST-affected days are correctly sized.
    pub(super) fn next_boundary(&self, start: DateTime<Local>) -> DateTime<Local> {
        match self {
            TimeSubDivision::Minute => start + ChronoDuration::minutes(1),
            TimeSubDivision::FiveMinutes => start + ChronoDuration::minutes(5),
            TimeSubDivision::TenMinutes => start + ChronoDuration::minutes(10),
            TimeSubDivision::QuarterHour => start + ChronoDuration::minutes(15),
            // TimeSubDivision::HalfHour => start + ChronoDuration::minutes(30),
            TimeSubDivision::Hour => start + ChronoDuration::hours(1),
            TimeSubDivision::EvenHour => start + ChronoDuration::hours(2),
            // TimeSubDivision::QuarterDay => start + ChronoDuration::hours(6),
            TimeSubDivision::HalfDay => start + ChronoDuration::hours(12),
            TimeSubDivision::Day => BaseTimeDivision::Day.next_boundary(start),
            TimeSubDivision::Week => {
                // Advance by exactly 7 calendar days from `start` (assumed to be Monday
                // midnight), yielding the next Monday midnight in local time.
                let naive = (start.date_naive() + Days::new(7))
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    // Extremely unlikely for a midnight, but fall back to fixed duration.
                    LocalResult::None => start + ChronoDuration::weeks(1),
                }
            }
            TimeSubDivision::Month => BaseTimeDivision::Month.next_boundary(start),
            TimeSubDivision::QuarterYear => {
                let month = start.month();
                let next_quarter_month = if month <= 3 {
                    4
                } else if month <= 6 {
                    7
                } else if month <= 9 {
                    10
                } else {
                    1
                };
                let (next_year, next_month) = if next_quarter_month == 1 {
                    (start.year() + 1, 1)
                } else {
                    (start.year(), next_quarter_month)
                };
                let naive = NaiveDate::from_ymd_opt(next_year, next_month, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => start + ChronoDuration::days(92), // Fallback: add ~3 months
                }
            }
        }
    }

    /// Returns the start of the subdivision period that contains `time`.
    pub(super) fn floor_boundary(&self, time: DateTime<Local>) -> DateTime<Local> {
        match self {
            TimeSubDivision::Minute => time
                .with_second(0)
                .and_then(|t| t.with_nanosecond(0))
                .unwrap(),
            TimeSubDivision::FiveMinutes => time
                .with_minute((time.minute() / 5) * 5)
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap(),
            TimeSubDivision::TenMinutes => time
                .with_minute((time.minute() / 10) * 10)
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap(),
            TimeSubDivision::QuarterHour => time
                .with_minute((time.minute() / 15) * 15)
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap(),
            // TimeSubDivision::HalfHour => time
            //     .with_minute((time.minute() / 30) * 30)
            //     .and_then(|t| t.with_second(0))
            //     .and_then(|t| t.with_nanosecond(0))
            //     .unwrap(),
            TimeSubDivision::Hour => BaseTimeDivision::Hour.floor_boundary(time),
            TimeSubDivision::EvenHour => {
                let hour = time.hour();
                let even_hour = if hour % 2 == 0 { hour } else { hour - 1 };
                time.with_hour(even_hour)
                    .and_then(|t| t.with_minute(0))
                    .and_then(|t| t.with_second(0))
                    .and_then(|t| t.with_nanosecond(0))
                    .unwrap()
            }
            // TimeSubDivision::QuarterDay => {
            //     let quarter = (time.hour() / 6) * 6;
            //     time.with_hour(quarter)
            //         .and_then(|t| t.with_minute(0))
            //         .and_then(|t| t.with_second(0))
            //         .and_then(|t| t.with_nanosecond(0))
            //         .unwrap()
            // }
            TimeSubDivision::HalfDay => {
                let half = if time.hour() < 12 { 0 } else { 12 };
                time.with_hour(half)
                    .and_then(|t| t.with_minute(0))
                    .and_then(|t| t.with_second(0))
                    .and_then(|t| t.with_nanosecond(0))
                    .unwrap()
            }
            TimeSubDivision::Day => BaseTimeDivision::Day.floor_boundary(time),
            TimeSubDivision::Week => {
                // Floor to the most recent Monday midnight (ISO week start).
                let days_since_monday = time.weekday().num_days_from_monday() as u64;
                let naive = (time.date_naive() - Days::new(days_since_monday))
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => time - ChronoDuration::days(days_since_monday as i64),
                }
            }
            TimeSubDivision::Month => BaseTimeDivision::Month.floor_boundary(time),
            TimeSubDivision::QuarterYear => {
                let month = time.month();
                let quarter_start_month = if month <= 3 {
                    1
                } else if month <= 6 {
                    4
                } else if month <= 9 {
                    7
                } else {
                    10
                };
                let naive = NaiveDate::from_ymd_opt(time.year(), quarter_start_month, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => {
                        // Fallback: subtract months until we find a valid date.
                        let mut candidate = time;
                        loop {
                            candidate = candidate - ChronoDuration::days(30);
                            let naive =
                                NaiveDate::from_ymd_opt(candidate.year(), quarter_start_month, 1)
                                    .unwrap()
                                    .and_hms_opt(0, 0, 0)
                                    .unwrap();
                            if let LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) =
                                naive.and_local_timezone(Local)
                            {
                                break dt;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Returns the ceil boundary — start of the next period if `time` is not
    /// already on a boundary, or `time` itself if it is.
    pub(super) fn ceil_boundary(&self, time: DateTime<Local>) -> DateTime<Local> {
        let floor = self.floor_boundary(time);
        if floor == time {
            time
        } else {
            self.next_boundary(floor)
        }
    }

    pub(super) fn closest_boundary(&self, time: DateTime<Local>) -> DateTime<Local> {
        let floor = self.floor_boundary(time);
        let ceil = self.ceil_boundary(time);
        if time - floor < ceil - time {
            floor
        } else {
            ceil
        }
    }

    /// Exact duration of the subdivision period that starts at `division_start`.
    /// The caller should pass a value already on a boundary (from `floor_boundary`
    /// or a previous `next_boundary` call).
    pub(super) fn exact_duration(&self, division_start: DateTime<Local>) -> ChronoDuration {
        self.next_boundary(division_start) - division_start
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BaseTimeDivision {
    FiveMinutes,
    Hour,
    Day,
    Month,
    Year,
}

impl BaseTimeDivision {
    /// Returns the start of the next division boundary after `start`.
    ///
    /// Sub-day divisions advance by a fixed duration (minutes are always
    /// exact and hours are always 3600 s of real time regardless of DST —
    /// spring-forward simply causes the sequence to skip the non-existent
    /// wall-clock hour, and fall-back produces two items with the same
    /// wall-clock label). Day/Month/Year use calendar arithmetic so their
    /// boundaries always land on midnight / first-of-month / Jan 1 in local
    /// time, correctly producing 23 h, 24 h, or 25 h days and exact
    /// month/year lengths.
    pub(super) fn next_boundary(&self, start: DateTime<Local>) -> DateTime<Local> {
        match self {
            BaseTimeDivision::FiveMinutes => start + ChronoDuration::minutes(5),
            BaseTimeDivision::Hour => start + ChronoDuration::hours(1),
            BaseTimeDivision::Day => {
                let naive = (start.date_naive() + Days::new(1))
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    // Midnight can't reasonably be a spring-forward gap, but handle
                    // it defensively by adding 24 h of real time.
                    LocalResult::None => start + ChronoDuration::hours(24),
                }
            }
            BaseTimeDivision::Month => {
                let (next_year, next_month) = if start.month() == 12 {
                    (start.year() + 1, 1)
                } else {
                    (start.year(), start.month() + 1)
                };
                let naive = NaiveDate::from_ymd_opt(next_year, next_month, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => start + ChronoDuration::days(28),
                }
            }
            BaseTimeDivision::Year => {
                let naive = NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => start + ChronoDuration::days(365),
                }
            }
        }
    }

    /// Returns the start of the division period that contains `time`.
    pub(super) fn floor_boundary(&self, time: DateTime<Local>) -> DateTime<Local> {
        match self {
            BaseTimeDivision::FiveMinutes => time
                .with_minute((time.minute() / 5) * 5)
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap(),
            BaseTimeDivision::Hour => time
                .with_minute(0)
                .and_then(|t| t.with_second(0))
                .and_then(|t| t.with_nanosecond(0))
                .unwrap(),
            BaseTimeDivision::Day => {
                let naive = time.date_naive().and_hms_opt(0, 0, 0).unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => {
                        // Subtract sub-day components as real time if midnight is
                        // in a spring-forward gap (extremely unlikely).
                        time - ChronoDuration::seconds(
                            (time.hour() * 3600 + time.minute() * 60 + time.second()) as i64,
                        )
                    }
                }
            }
            BaseTimeDivision::Month => {
                let naive = NaiveDate::from_ymd_opt(time.year(), time.month(), 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => time - ChronoDuration::days(time.day() as i64 - 1),
                }
            }
            BaseTimeDivision::Year => {
                let naive = NaiveDate::from_ymd_opt(time.year(), 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => time - ChronoDuration::days(time.ordinal() as i64 - 1),
                }
            }
        }
    }

    /// Returns the ceil boundary — start of the next period if `time` is not
    /// already on a boundary, or `time` itself if it is.
    pub(super) fn ceil_boundary(&self, time: DateTime<Local>) -> DateTime<Local> {
        let floor = self.floor_boundary(time);
        if floor == time {
            time
        } else {
            self.next_boundary(floor)
        }
    }

    /// Exact duration of the division that starts at `division_start`.
    /// The caller should pass a value already on a boundary (from `floor_boundary`
    /// or a previous `next_boundary` call).
    pub(super) fn exact_duration(&self, division_start: DateTime<Local>) -> ChronoDuration {
        self.next_boundary(division_start) - division_start
    }

    /// The `TimeDivisionStyle` used to render each item's primary label.
    pub(super) fn base_label_style(&self) -> TimeDivisionStyle {
        match self {
            BaseTimeDivision::FiveMinutes => TimeDivisionStyle::HourMinute,
            BaseTimeDivision::Hour => TimeDivisionStyle::Hour,
            BaseTimeDivision::Day => TimeDivisionStyle::WeekDayDate,
            BaseTimeDivision::Month => TimeDivisionStyle::MonthName,
            BaseTimeDivision::Year => TimeDivisionStyle::Year,
        }
    }

    /// Returns `true` when `time` is the first item of an outer division
    /// (e.g. midnight for Hour, the 1st of the month for Day).
    pub(super) fn is_outer_boundary(&self, time: DateTime<Local>) -> bool {
        match self {
            BaseTimeDivision::FiveMinutes => time.minute() == 0,
            BaseTimeDivision::Hour => time.hour() == 0,
            BaseTimeDivision::Day => time.day() == 1,
            BaseTimeDivision::Month => time.month() == 1,
            BaseTimeDivision::Year => false,
        }
    }

    /// The label string shown on the right side of an outer-boundary divider line.
    /// Returns `None` for Year (no outer) or if the time is not a boundary.
    pub(super) fn outer_label(&self, time: DateTime<Local>) -> Option<String> {
        match self {
            // Outer of FiveMinutes is Hour → show e.g. "2 PM"
            BaseTimeDivision::FiveMinutes => Some(time.format("%-I %p").to_string()),
            // Outer of Hour is Day → show e.g. "Mon 15"
            BaseTimeDivision::Hour => Some(time.format("%a %-d").to_string()),
            // Outer of Day is Month → show e.g. "March 2024"
            BaseTimeDivision::Day => Some(time.format("%B").to_string()),
            // Outer of Month is Year → show e.g. "2024"
            BaseTimeDivision::Month => Some(time.format("%Y").to_string()),
            BaseTimeDivision::Year => None,
        }
    }

    pub(super) fn outer_division(&self) -> Option<BaseTimeDivision> {
        match self {
            BaseTimeDivision::FiveMinutes => Some(BaseTimeDivision::Hour),
            BaseTimeDivision::Hour => Some(BaseTimeDivision::Day),
            BaseTimeDivision::Day => Some(BaseTimeDivision::Month),
            BaseTimeDivision::Month => Some(BaseTimeDivision::Year),
            BaseTimeDivision::Year => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimeZoomLevel {
    ZoomedIn,
    Normal,
    ZoomedOut,
    ZoomedOutFar,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TimeDivisionState {
    pub base_division: BaseTimeDivision,
    pub zoom_level: TimeZoomLevel,
}

impl TimeDivisionState {
    pub(super) fn current_subdivision(&self) -> Option<TimeSubDivision> {
        match self.zoom_level {
            TimeZoomLevel::ZoomedIn => match self.base_division {
                BaseTimeDivision::FiveMinutes => Some(TimeSubDivision::Minute),
                BaseTimeDivision::Hour => Some(TimeSubDivision::FiveMinutes),
                BaseTimeDivision::Day => Some(TimeSubDivision::Hour),
                BaseTimeDivision::Month => Some(TimeSubDivision::Day),
                BaseTimeDivision::Year => Some(TimeSubDivision::Month),
            },
            TimeZoomLevel::Normal => match self.base_division {
                BaseTimeDivision::FiveMinutes => Some(TimeSubDivision::Minute),
                BaseTimeDivision::Hour => Some(TimeSubDivision::TenMinutes),
                BaseTimeDivision::Day => Some(TimeSubDivision::EvenHour),
                BaseTimeDivision::Month => Some(TimeSubDivision::Week),
                BaseTimeDivision::Year => Some(TimeSubDivision::QuarterYear),
            },
            TimeZoomLevel::ZoomedOut => match self.base_division {
                BaseTimeDivision::FiveMinutes => None,
                BaseTimeDivision::Hour => Some(TimeSubDivision::QuarterHour),
                BaseTimeDivision::Day => Some(TimeSubDivision::HalfDay),
                BaseTimeDivision::Month => None,
                BaseTimeDivision::Year => None,
            },
            TimeZoomLevel::ZoomedOutFar => match self.base_division {
                BaseTimeDivision::FiveMinutes => None,
                BaseTimeDivision::Hour => None,
                BaseTimeDivision::Day => None,
                BaseTimeDivision::Month => None,
                BaseTimeDivision::Year => None,
            },
        }
    }
}

/// Floor a datetime to the nearest fixed-duration boundary within the current day.
/// Used for sub-hour tick positioning where divisions are always uniform.
/// For calendar-aware flooring (Day/Month/Year), use `BaseTimeDivision::floor_boundary`.
#[allow(dead_code)]
pub(super) fn floor_division(division: ChronoDuration, time: DateTime<Local>) -> DateTime<Local> {
    let secs = division.as_seconds_f32() as i64;
    let day_start = time
        .with_hour(0)
        .and_then(|t| t.with_minute(0))
        .and_then(|t| t.with_second(0))
        .and_then(|t| t.with_nanosecond(0))
        .unwrap();
    let elapsed = (time - day_start).as_seconds_f32().round() as i64;
    let floored_secs = (elapsed / secs) * secs;
    day_start + ChronoDuration::seconds(floored_secs)
}

#[allow(dead_code)]
pub(super) fn ceil_division(division: ChronoDuration, time: DateTime<Local>) -> DateTime<Local> {
    let floor = floor_division(division, time);
    if floor == time {
        time
    } else {
        floor + division
    }
}

fn hour_minute_label(datetime: DateTime<Local>, muted: bool, pm: bool, cx: &App) -> Div {
    let format = if pm {
        "%-I:%M %p" // "2:30 PM"
    } else {
        "%-I:%M" // "2:30"
    };
    let str = datetime.format(format).to_string();
    div()
        .child(str)
        .text_sm()
        .when(muted, |this| this.text_color(cx.theme().muted_foreground))
}

fn hour_label(datetime: DateTime<Local>, cx: &App) -> Div {
    let hour = datetime.hour();
    let primary = match hour {
        0 => "12".to_string(),
        12 => "Noon".to_string(),
        _ if hour < 13 => format!("{}", hour),
        _ => format!("{}", hour - 12),
    };

    let muted = cx.theme().muted_foreground;
    let primary_color = cx.theme().foreground.mix_oklab(muted, 0.5);

    let secondary = match hour {
        12 => None,
        _ if hour < 13 => Some("AM"),
        _ => Some("PM"),
    };

    h_flex()
        .gap_0p5()
        .items_end()
        .child(div().child(primary).text_sm().text_color(primary_color))
        .when_some(secondary, |this, str| {
            this.child(div().child(str).text_xs().text_color(muted))
        })
}

fn time_of_day_label(datetime: DateTime<Local>, cx: &App) -> Div {
    let hour = datetime.hour();
    let label = match hour {
        0 => "Midnight",
        12 => "Noon",
        _ if hour < 12 => "Morning",
        _ if hour < 18 => "Afternoon",
        _ => "Evening",
    };
    div()
        .child(label)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
}

// fn date_label(datetime: DateTime<Local>, minimal: bool, cx: &App) -> Div {
//     let format = match minimal {
//         true => "%e",     // "15"
//         false => "%b %e", // "Mar 15"
//     };
//     let str = datetime.format(format).to_string();
//     div()
//         .child(str)
//         .text_sm()
//         .text_color(cx.theme().muted_foreground)
// }

fn weekday_date_label(datetime: DateTime<Local>, minimal: bool, cx: &App) -> Div {
    let format = match minimal {
        true => "%a",     // "Fri"
        false => "%a %e", // "Fri 15"
    };
    let str = datetime.format(format).to_string();
    div()
        .child(str)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
}

fn week_start_label(datetime: DateTime<Local>, cx: &App) -> Div {
    let week = datetime.iso_week().week();
    let str = format!("W{}", week); // "2024 W11"
    div()
        .child(str)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
}

fn month_label(datetime: DateTime<Local>, cx: &App) -> Div {
    let str = datetime.format("%B").to_string(); // "March"
    div()
        .child(str)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
}

// fn time_of_year_label(datetime: DateTime<Local>, cx: &App) -> Div {
//     let month = datetime.month();
//     let label = match month {
//         12 | 1 | 2 => "Winter",
//         3 | 4 | 5 => "Spring",
//         6 | 7 | 8 => "Summer",
//         _ => "Fall",
//     };
//     div()
//         .child(label)
//         .text_sm()
//         .text_color(cx.theme().muted_foreground)
// }

fn year_quarter_label(datetime: DateTime<Local>, cx: &App) -> Div {
    let month = datetime.month();
    let quarter = match month {
        1..=3 => "Q1",
        4..=6 => "Q2",
        7..=9 => "Q3",
        _ => "Q4",
    };
    div()
        .child(quarter)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
}

fn year_label(datetime: DateTime<Local>, cx: &App) -> Div {
    let str = datetime.format("%Y").to_string(); // "2024"
    div()
        .child(str)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
}
