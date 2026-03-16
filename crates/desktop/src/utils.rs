use anyhow::{Result, bail};
use chrono::{DateTime, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

/// Parses a human-readable duration string like "24h", "7d", "90m", "3600s".
pub fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    if let Some(value) = s.strip_suffix('d') {
        let days: i64 = value
            .parse()
            .map_err(|_| anyhow::anyhow!("Expected a number before 'd'"))?;
        return Ok(Duration::days(days));
    }
    if let Some(value) = s.strip_suffix('h') {
        let hours: i64 = value
            .parse()
            .map_err(|_| anyhow::anyhow!("Expected a number before 'h'"))?;
        return Ok(Duration::hours(hours));
    }
    if let Some(value) = s.strip_suffix('m') {
        let minutes: i64 = value
            .parse()
            .map_err(|_| anyhow::anyhow!("Expected a number before 'm'"))?;
        return Ok(Duration::minutes(minutes));
    }
    if let Some(value) = s.strip_suffix('s') {
        let seconds: i64 = value
            .parse()
            .map_err(|_| anyhow::anyhow!("Expected a number before 's'"))?;
        return Ok(Duration::seconds(seconds));
    }
    bail!(
        "Unrecognized duration format '{}'. Use a number followed by d, h, m, or s (e.g. 24h, 7d, 30m)",
        s
    )
}

/// Formats a chrono Duration into a compact human-readable string (e.g. "1h", "30m").
pub fn format_duration(d: Duration) -> String {
    let total_seconds = d.num_seconds();
    if total_seconds == 0 {
        return "0m".to_string();
    }
    if total_seconds % 86400 == 0 {
        format!("{}d", total_seconds / 86400)
    } else if total_seconds % 3600 == 0 {
        format!("{}h", total_seconds / 3600)
    } else if total_seconds % 60 == 0 {
        format!("{}m", total_seconds / 60)
    } else {
        format!("{}s", total_seconds)
    }
}

/// Parses a flexible datetime string into a `DateTime<Utc>`, interpreting bare
/// dates/times as **local time** and converting to UTC.
///
/// Accepted formats: RFC-3339, `2026-03-01 18:00`, `2026-03-01`, `18:00`, `6pm`, `6:30pm`.
pub fn parse_datetime_local(s: &str) -> Result<DateTime<Utc>> {
    let s = s.trim();

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }

    for fmt in &[
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Local
                .from_local_datetime(&ndt)
                .single()
                .map(|dt| dt.with_timezone(&Utc))
                .ok_or_else(|| anyhow::anyhow!("Ambiguous local datetime '{}'", s));
        }
    }

    if let Ok(nd) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let ndt = nd
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow::anyhow!("Invalid date midnight"))?;
        return Local
            .from_local_datetime(&ndt)
            .single()
            .map(|dt| dt.with_timezone(&Utc))
            .ok_or_else(|| anyhow::anyhow!("Ambiguous local date '{}'", s));
    }

    if let Ok(time) = parse_time_of_day(s) {
        let today = Local::now().date_naive();
        let ndt = today.and_time(time);
        return Local
            .from_local_datetime(&ndt)
            .single()
            .map(|dt| dt.with_timezone(&Utc))
            .ok_or_else(|| anyhow::anyhow!("Ambiguous local time '{}'", s));
    }

    bail!(
        "Unrecognized datetime '{}'. Accepted formats: \
        '2026-03-01T18:00Z', '2026-03-01 18:00', '2026-03-01', '18:00', '6pm', '6:30am'",
        s
    )
}

/// Formats a UTC datetime into a local-time string for display in the UI.
pub fn format_datetime_local(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

fn parse_time_of_day(s: &str) -> Result<NaiveTime> {
    let s = s.trim();

    for fmt in &["%H:%M:%S", "%H:%M"] {
        if let Ok(t) = NaiveTime::parse_from_str(s, fmt) {
            return Ok(t);
        }
    }

    let lower = s.to_lowercase();
    let lower = lower.trim();
    if lower.ends_with("am") || lower.ends_with("pm") {
        let is_pm = lower.ends_with("pm");
        let time_part = if is_pm {
            lower.trim_end_matches("pm")
        } else {
            lower.trim_end_matches("am")
        }
        .trim();
        let (hour, minute) = if let Some((h, m)) = time_part.split_once(':') {
            let hour: u32 = h
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid hour in '{}'", s))?;
            let minute: u32 = m
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid minute in '{}'", s))?;
            (hour, minute)
        } else {
            let hour: u32 = time_part
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid hour in '{}'", s))?;
            (hour, 0)
        };
        let hour_24 = match (is_pm, hour) {
            (false, 12) => 0,
            (false, h) => h,
            (true, 12) => 12,
            (true, h) => h + 12,
        };
        if let Some(t) = NaiveTime::from_hms_opt(hour_24, minute, 0) {
            return Ok(t);
        }
    }

    if let Ok(hour) = s.parse::<u32>() {
        if let Some(t) = NaiveTime::from_hms_opt(hour, 0, 0) {
            return Ok(t);
        }
    }

    bail!(
        "Unrecognized time '{}'. Accepted formats: '18:00', '6pm', '6:30am', '14'",
        s
    )
}
