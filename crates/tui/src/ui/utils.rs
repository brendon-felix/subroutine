use chrono::Timelike;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Clear, Widget},
};

pub fn centered_area(area: Rect, height: u16, width: u16) -> Rect {
    Layout::new(
        Direction::Horizontal,
        [
            Constraint::Fill(1),
            Constraint::Length(width),
            Constraint::Fill(1),
        ],
    )
    .split(
        Layout::new(
            Direction::Vertical,
            [
                Constraint::Fill(1),
                Constraint::Length(height),
                Constraint::Fill(1),
            ],
        )
        .split(area)[1],
    )[1]
}

// pub fn centered_area_with_offset(area: Rect, height: u16, width: u16, offset: u16) -> Rect {
//     Layout::new(
//         Direction::Horizontal,
//         [
//             Constraint::Fill(1),
//             Constraint::Length(width),
//             Constraint::Fill(1),
//         ],
//     )
//     .split(
//         Layout::new(
//             Direction::Vertical,
//             [
//                 Constraint::Length(offset),
//                 Constraint::Length(height),
//                 Constraint::Fill(1),
//             ],
//         )
//         .split(area)[1],
//     )[1]
// }

pub fn paint_background(f: &mut Frame) {
    Clear.render(f.area(), f.buffer_mut());
    Block::default()
        .style(Style::default().bg(Color::Rgb(25, 25, 25)))
        .render(f.area(), f.buffer_mut());
}

/// Parse date in US format (MM/DD or MM/DD/YYYY) or ISO format (YYYY-MM-DD)
/// If year is not provided, uses current year or next year for valid future dates
pub fn parse_date_us_format(date_str: &str) -> Result<chrono::NaiveDate, String> {
    use chrono::{Datelike, Local, NaiveDate};

    let date_str = date_str.trim();

    // Try ISO format first (YYYY-MM-DD)
    if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        return Ok(date);
    }

    // Determine separator (/ or -)
    let separator = if date_str.contains('/') {
        '/'
    } else if date_str.contains('-') {
        '-'
    } else {
        return Err(format!(
            "Invalid date format. Use MM/DD, MM/DD/YYYY, MM-DD, MM-DD-YY, or YYYY-MM-DD"
        ));
    };

    let parts: Vec<&str> = date_str.split(separator).collect();

    let (month, day, year_opt) = match parts.len() {
        2 => {
            // MM/DD or MM-DD (no year provided)
            let month = parts[0]
                .parse::<u32>()
                .map_err(|_| format!("Invalid month: {}", parts[0]))?;
            let day = parts[1]
                .parse::<u32>()
                .map_err(|_| format!("Invalid day: {}", parts[1]))?;
            (month, day, None)
        }
        3 => {
            // MM/DD/YYYY or MM/DD/YY or MM-DD-YYYY or MM-DD-YY
            let month = parts[0]
                .parse::<u32>()
                .map_err(|_| format!("Invalid month: {}", parts[0]))?;
            let day = parts[1]
                .parse::<u32>()
                .map_err(|_| format!("Invalid day: {}", parts[1]))?;
            let year_part = parts[2]
                .parse::<i32>()
                .map_err(|_| format!("Invalid year: {}", parts[2]))?;

            // Handle 2-digit years
            let year = if year_part < 100 {
                // Assume 2000s for 2-digit years
                2000 + year_part
            } else {
                year_part
            };

            (month, day, Some(year))
        }
        _ => {
            return Err(format!(
                "Invalid date format. Use MM/DD, MM/DD/YYYY, MM-DD, MM-DD-YY, or YYYY-MM-DD"
            ));
        }
    };

    // Validate month and day ranges
    if month < 1 || month > 12 {
        return Err(format!("Month must be between 1 and 12"));
    }
    if day < 1 || day > 31 {
        return Err(format!("Day must be between 1 and 31"));
    }

    // If no year provided, determine current or next year for future date
    let year = if let Some(y) = year_opt {
        y
    } else {
        let today = Local::now().date_naive();
        let current_year = today.year();

        // Try current year first
        if let Some(date) = NaiveDate::from_ymd_opt(current_year, month, day) {
            if date >= today {
                current_year
            } else {
                // Date has passed this year, use next year
                current_year + 1
            }
        } else {
            // Invalid date for current year (e.g., Feb 30), try next year
            current_year + 1
        }
    };

    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("Invalid date: {}/{}/{}", month, day, year))
}

/// Parse time in US format (12-hour with AM/PM)
pub fn parse_time_us_format(time_str: &str) -> Result<chrono::NaiveTime, String> {
    use chrono::NaiveTime;

    let time_str = time_str.trim().to_lowercase();

    // Check if it contains AM or PM
    let (is_pm, time_without_suffix) = if time_str.ends_with("pm") {
        (true, time_str.trim_end_matches("pm").trim())
    } else if time_str.ends_with("am") {
        (false, time_str.trim_end_matches("am").trim())
    } else {
        // Try 24-hour format as fallback
        return NaiveTime::parse_from_str(&time_str, "%H:%M").map_err(|_| {
            format!("Invalid time format. Use formats like '5pm', '5:30 AM', or '17:00'")
        });
    };

    // Parse hour and optional minutes
    let parts: Vec<&str> = time_without_suffix.split(':').collect();

    let (hour_12, minute) = if parts.len() == 1 {
        // No colon, just hour (e.g., "5pm")
        let hour = parts[0]
            .parse::<u32>()
            .map_err(|_| format!("Invalid hour: {}", parts[0]))?;
        (hour, 0)
    } else if parts.len() == 2 {
        // Hour and minute (e.g., "5:30pm")
        let hour = parts[0]
            .parse::<u32>()
            .map_err(|_| format!("Invalid hour: {}", parts[0]))?;
        let minute = parts[1]
            .parse::<u32>()
            .map_err(|_| format!("Invalid minute: {}", parts[1]))?;
        (hour, minute)
    } else {
        return Err(format!(
            "Invalid time format. Use formats like '5pm' or '5:30 AM'"
        ));
    };

    // Validate ranges
    if hour_12 < 1 || hour_12 > 12 {
        return Err(format!("Hour must be between 1 and 12"));
    }
    if minute > 59 {
        return Err(format!("Minute must be between 0 and 59"));
    }

    // Convert to 24-hour format
    let hour_24 = if hour_12 == 12 {
        if is_pm { 12 } else { 0 }
    } else {
        if is_pm { hour_12 + 12 } else { hour_12 }
    };

    NaiveTime::from_hms_opt(hour_24, minute, 0)
        .ok_or_else(|| format!("Invalid time: {}:{:02}", hour_24, minute))
}

// /// Represents the target for a reschedule operation
// #[derive(Debug)]
// pub enum RescheduleTarget {
//     /// Relative to the task's original due datetime (e.g., "5min", "2 hours")
//     RelativeToDueDate(chrono::Duration),
//     /// Absolute time from now (e.g., "now", "now + 5min")
//     AbsoluteTime(chrono::DateTime<chrono::Local>),
// }

// /// Parse duration expression and return the reschedule target
// /// Supports formats like:
// /// - "now" - current time (absolute)
// /// - "5min", "5 minutes" - 5 minutes from task's due date (relative)
// /// - "10hr", "10 hours" - 10 hours from task's due date (relative)
// /// - "2day", "2 days" - 2 days from task's due date (relative)
// /// - "now + 5min", "now+5 minutes" - 5 minutes from now (absolute)
// pub fn parse_duration(duration_str: &str) -> Result<RescheduleTarget, String> {
//     use chrono::Local;

//     let duration_str = duration_str.trim().to_lowercase();

//     // Check if it starts with "now"
//     let is_absolute = duration_str.starts_with("now");

//     // Handle "now" case without additional duration
//     if duration_str == "now" {
//         return Ok(RescheduleTarget::AbsoluteTime(Local::now()));
//     }

//     // Get the duration part
//     let duration_part = if is_absolute {
//         let after_now = duration_str.strip_prefix("now").unwrap().trim();
//         // Remove optional "+" sign
//         after_now.strip_prefix("+").unwrap_or(after_now).trim()
//     } else {
//         &duration_str
//     };

//     // Parse the duration part (number + unit)
//     // Try to find where the number ends and the unit begins
//     let mut num_end = 0;
//     for (i, ch) in duration_part.chars().enumerate() {
//         if ch.is_ascii_digit() {
//             num_end = i + 1;
//         } else {
//             break;
//         }
//     }

//     if num_end == 0 {
//         return Err(format!(
//             "Invalid duration format. Expected number followed by unit (e.g., '5min', '2 hours')"
//         ));
//     }

//     let num_str = &duration_part[..num_end];
//     let unit_str = duration_part[num_end..].trim();

//     let value = num_str
//         .parse::<i64>()
//         .map_err(|_| format!("Invalid duration value: {}", num_str))?;

//     // Parse the unit
//     let duration = match unit_str {
//         "min" | "minute" | "minutes" => chrono::Duration::minutes(value),
//         "hr" | "hour" | "hours" | "h" => chrono::Duration::hours(value),
//         "day" | "days" | "d" => chrono::Duration::days(value),
//         _ => {
//             return Err(format!(
//                 "Invalid duration unit: '{}'. Use 'min', 'minutes', 'hr', 'hours', 'day', or 'days'",
//                 unit_str
//             ));
//         }
//     };

//     if is_absolute {
//         Ok(RescheduleTarget::AbsoluteTime(Local::now() + duration))
//     } else {
//         Ok(RescheduleTarget::RelativeToDueDate(duration))
//     }
// }

/// Parse a flexible datetime string that can handle:
/// - Keywords: "today", "tomorrow"
/// - Dates: "12/25", "12/25/2024", "2024-12-25"
/// - Times: "5pm", "5:30am", "17:30"
/// - Date + Time: "12/25 5pm", "tomorrow 9am"
/// - Durations: "in 10min", "in 2hr", "in 1 day"
///
/// For time-only input, automatically determines if it's today or tomorrow
/// based on whether the time has already passed today.
pub fn parse_datetime(input: &str) -> Result<chrono::NaiveDateTime, String> {
    use chrono::{Local, NaiveDateTime};

    let input = input.trim().to_lowercase();
    let now = Local::now();
    let today = now.date_naive();
    let current_time = now.time();

    // Handle keywords
    match input.as_str() {
        "today" => {
            return Ok(today.and_hms_opt(0, 0, 0).unwrap());
        }
        "tomorrow" => {
            let tomorrow = today + chrono::Duration::days(1);
            return Ok(tomorrow.and_hms_opt(0, 0, 0).unwrap());
        }
        _ => {}
    }

    // Handle duration formats (e.g., "in 10min", "in 2hr")
    if input.starts_with("in ") {
        let duration_part = input.strip_prefix("in ").unwrap().trim();

        // Parse the duration part (number + unit)
        let mut num_end = 0;
        for (i, ch) in duration_part.chars().enumerate() {
            if ch.is_ascii_digit() {
                num_end = i + 1;
            } else {
                break;
            }
        }

        if num_end == 0 {
            return Err(format!(
                "Invalid duration format. Expected 'in X unit' (e.g., 'in 10min', 'in 2 hours')"
            ));
        }

        let num_str = &duration_part[..num_end];
        let unit_str = duration_part[num_end..].trim();

        let value = num_str
            .parse::<i64>()
            .map_err(|_| format!("Invalid duration value: {}", num_str))?;

        // Parse the unit
        let duration = match unit_str {
            "min" | "minute" | "minutes" => chrono::Duration::minutes(value),
            "hr" | "hour" | "hours" | "h" => chrono::Duration::hours(value),
            "day" | "days" | "d" => chrono::Duration::days(value),
            "sec" | "second" | "seconds" | "s" => chrono::Duration::seconds(value),
            _ => {
                return Err(format!(
                    "Invalid duration unit: '{}'. Use 'min', 'minutes', 'hr', 'hours', 'day', 'days', 'sec', or 'seconds'",
                    unit_str
                ));
            }
        };

        let target_datetime = now + duration;
        return Ok(target_datetime.naive_local());
    }

    // Split input to check for date + time combinations
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.len() == 2 {
        // Try parsing as "date time" or "keyword time"
        let first_part = parts[0];
        let second_part = parts[1];

        // Check if first part is a keyword
        let date = match first_part {
            "today" => today,
            "tomorrow" => today + chrono::Duration::days(1),
            _ => {
                // Try parsing as date
                parse_date_us_format(first_part)?
            }
        };

        // Parse the time part
        let time = parse_time_us_format(second_part)?;
        return Ok(date.and_time(time));
    }

    if parts.len() == 1 {
        let single_part = parts[0];

        // Try parsing as time only
        if let Ok(time) = parse_time_us_format(single_part) {
            // Determine if this time is today or tomorrow
            let target_date = if time > current_time {
                // Time hasn't passed yet today
                today
            } else {
                // Time has already passed today, use tomorrow
                today + chrono::Duration::days(1)
            };
            return Ok(target_date.and_time(time));
        }

        // Try parsing as date only
        if let Ok(date) = parse_date_us_format(single_part) {
            // Default to start of day (midnight)
            return Ok(date.and_hms_opt(0, 0, 0).unwrap());
        }
    }

    // If we get here, try parsing the entire input as a combined datetime string
    // This handles cases where there might be different formatting

    // Try common datetime formats
    let datetime_formats = [
        "%m/%d/%Y %I:%M %p",
        "%m/%d/%Y %H:%M",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d %I:%M %p",
        "%m/%d %I:%M %p",
        "%m/%d %H:%M",
    ];

    for format in &datetime_formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(&input, format) {
            return Ok(dt);
        }
    }

    Err(format!(
        "Unable to parse '{}'. Supported formats include: keywords (today, tomorrow), dates (12/25, 12/25/2024), times (5pm, 17:30), combinations (today 5pm), or durations (in 10min)",
        input
    ))
}

/// Format a DateTime<Utc> back to a string that can be parsed by parse_datetime
/// Uses "Today" and "Tomorrow" keywords when applicable, otherwise formats as date and time
/// If is_all_day is true, omits the time portion
pub fn format_datetime(dt: chrono::DateTime<chrono::Utc>, is_all_day: bool) -> String {
    use chrono::{Datelike, Local, NaiveTime, Weekday};

    let now = Local::now();
    let today = now.date_naive();
    let tomorrow = today + chrono::Duration::days(1);
    let local_dt = dt.with_timezone(&Local).naive_local();
    let dt_date = local_dt.date();
    let dt_time = local_dt.time();
    let midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap();

    // Calculate days difference
    let days_diff = (dt_date - today).num_days();

    // Check if it's today or tomorrow
    if dt_date == today {
        if is_all_day || dt_time == midnight {
            // All day or exact start of today
            "Today".to_string()
        } else {
            // Today with specific time
            format!("Today {}", format_time(dt_time))
        }
    } else if dt_date == tomorrow {
        if is_all_day || dt_time == midnight {
            // All day or exact start of tomorrow
            "Tomorrow".to_string()
        } else {
            // Tomorrow with specific time
            format!("Tomorrow {}", format_time(dt_time))
        }
    } else if days_diff > 1 && days_diff <= 7 {
        // Within next 7 days (excluding today/tomorrow): show day name
        let day_name = match dt_date.weekday() {
            Weekday::Mon => "Monday",
            Weekday::Tue => "Tuesday",
            Weekday::Wed => "Wednesday",
            Weekday::Thu => "Thursday",
            Weekday::Fri => "Friday",
            Weekday::Sat => "Saturday",
            Weekday::Sun => "Sunday",
        };
        if is_all_day || dt_time == midnight {
            day_name.to_string()
        } else {
            format!("{} {}", day_name, format_time(dt_time))
        }
    } else if days_diff > 7 && days_diff <= 14 {
        // Between 7 and 14 days: show "Next Mon", "Next Tue", etc.
        let day_abbrev = match dt_date.weekday() {
            Weekday::Mon => "Mon",
            Weekday::Tue => "Tue",
            Weekday::Wed => "Wed",
            Weekday::Thu => "Thu",
            Weekday::Fri => "Fri",
            Weekday::Sat => "Sat",
            Weekday::Sun => "Sun",
        };
        if is_all_day || dt_time == midnight {
            format!("Next {}", day_abbrev)
        } else {
            format!("Next {} {}", day_abbrev, format_time(dt_time))
        }
    } else {
        // Other date (past or > 14 days in future)
        if is_all_day || dt_time == midnight {
            // Date only (all day or midnight)
            format!("{}/{}/{}", dt_date.month(), dt_date.day(), dt_date.year())
        } else {
            // Date with time
            format!(
                "{}/{}/{} {}",
                dt_date.month(),
                dt_date.day(),
                dt_date.year(),
                format_time(dt_time)
            )
        }
    }
}

/// Helper function to format time in a way that can be parsed by parse_time_us_format
fn format_time(time: chrono::NaiveTime) -> String {
    let hour = time.hour();
    let minute = time.minute();

    if hour == 0 {
        // Midnight
        if minute == 0 {
            "12am".to_string()
        } else {
            format!("12:{:02}am", minute)
        }
    } else if hour < 12 {
        // AM hours
        if minute == 0 {
            format!("{}am", hour)
        } else {
            format!("{}:{:02}am", hour, minute)
        }
    } else if hour == 12 {
        // Noon
        if minute == 0 {
            "12pm".to_string()
        } else {
            format!("12:{:02}pm", minute)
        }
    } else {
        // PM hours
        let hour_12 = hour - 12;
        if minute == 0 {
            format!("{}pm", hour_12)
        } else {
            format!("{}:{:02}pm", hour_12, minute)
        }
    }
}
