//! Clause-scanner parser for natural-language action/event input.
//!
//! Architecture:
//!   1. Lex input into `Vec<SpannedToken>`
//!   2. Walk token stream; at each position try every clause recogniser
//!   3. Longest-match wins; consumed positions are marked
//!   4. Remaining tokens form the title
//!
//! Each clause recogniser is a plain function:
//!   `fn try_<clause>(tokens, pos, ctx, now) -> Option<(Value, usize)>`
//! returning `Some((value, next_pos))` on success or `None` on no-match.

use chrono::{
    DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
    Weekday,
};
use thiserror::Error;

use crate::ast::{EntityKind, HighlightKind, ParseDraft, RecurrenceSpec, WeekdaySet, WhenSpec};
use crate::lexer::{SpannedToken, Token, lex};

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("missing title")]
    MissingTitle,
}

pub fn parse_action(input: &str) -> Result<ParseDraft, ParseError> {
    parse_impl(input, EntityKind::Action, Local::now())
}

pub fn parse_event(input: &str) -> Result<ParseDraft, ParseError> {
    parse_impl(input, EntityKind::Event, Local::now())
}

/// Parse a free-form routine step description.
pub fn parse_routine_step(input: &str) -> Result<ParseDraft, ParseError> {
    parse_impl(input, EntityKind::RoutineStep, Local::now())
}

// /// Parse a recurrence specifier string in isolation (used by tests and the
// /// `%` sigil handler).
// #[allow(dead_code)] // Used by tests and future external callers
// pub fn parse_recurrence_str(text: &str) -> Result<RecurrenceSpec, ParseError> {
//     try_recurrence_text(text).ok_or_else(|| ParseError::MissingTitle)
// }

// ---------------------------------------------------------------------------
// Core scanner
// ---------------------------------------------------------------------------

fn parse_impl(
    input: &str,
    kind: EntityKind,
    // ctx: &ParseContext<'_>,
    now: DateTime<Local>,
) -> Result<ParseDraft, ParseError> {
    let tokens = lex(input);
    let mut draft = ParseDraft::new(kind.clone(), input);
    let mut consumed = vec![false; tokens.len()];

    // One forward pass.  At each unconsumed position we try all clause
    // recognisers and take the longest match.
    // `now` is DateTime<Local>; passing it through means all relative date
    // calculations (today, tomorrow, in N days, …) use the user's local date
    // rather than the UTC date.  date_at() converts naive-local → UTC at the
    // point of storage, so ParseDraft.when is always UTC.
    let mut i = 0;
    while i < tokens.len() {
        if consumed[i] {
            i += 1;
            continue;
        }

        if let Some((len, kind, apply)) = longest_match(&tokens, i, now, &draft) {
            apply(&mut draft);
            let start = tokens[i].span.start;
            let end = tokens[i + len - 1].span.end;
            draft.highlights.push((start..end, kind));
            for j in i..i + len {
                consumed[j] = true;
            }
            i += len;
        } else {
            i += 1;
        }
    }

    assemble_title(&mut draft, &tokens, &consumed);

    for (idx, tok) in tokens.iter().enumerate() {
        if !consumed[idx] {
            match tok.token {
                Token::Word | Token::Number | Token::Quoted => {
                    draft
                        .highlights
                        .push((tok.span.clone(), HighlightKind::Title));
                }
                _ => {}
            }
        }
    }

    // Post-processing: reconcile recurrence with when.
    // Use the local date (from the original Local::now()) for "next matching
    // weekday" synthesis so it agrees with what the user sees on their clock.
    let now_local_date = now.date_naive();
    if let Some(RecurrenceSpec::OnWeekdays(set)) = draft.recurrence {
        match draft.when.clone() {
            None => {
                // Synthesise when = next matching weekday.
                // For actions: produce a NaiveDate (floating time).
                // For events: pin to default time.
                let mut d = now_local_date + Duration::days(1);
                for _ in 0..7 {
                    if set.contains(d.weekday()) {
                        draft.when = Some(match kind {
                            EntityKind::Action => WhenSpec::NaiveDate(d),
                            EntityKind::Event => WhenSpec::DateTime(date_at(d, default_time())),
                            EntityKind::RoutineStep => WhenSpec::NaiveDate(d),
                        });
                        break;
                    }
                    d = d + Duration::days(1);
                }
            }
            Some(when) if !set.contains(when.date().weekday()) => {
                // Advance the date part to the next matching weekday.
                let cur_date = when.date();
                let mut d = cur_date + Duration::days(1);
                for _ in 0..7 {
                    if set.contains(d.weekday()) {
                        draft.when = Some(match when {
                            WhenSpec::DateTime(dt) => {
                                // Preserve the parsed time-of-day.
                                let t = dt.with_timezone(&Local).time();
                                WhenSpec::DateTime(date_at(d, t))
                            }
                            WhenSpec::NaiveDate(_) => WhenSpec::NaiveDate(d),
                        });
                        break;
                    }
                    d = d + Duration::days(1);
                }
            }
            _ => {}
        }
    }

    if draft.title.trim().is_empty() {
        return Err(ParseError::MissingTitle);
    }

    Ok(draft)
}

// ---------------------------------------------------------------------------
// Longest-match dispatch
// ---------------------------------------------------------------------------

/// Try every clause recogniser at position `i` and return the one that
/// consumes the most tokens, together with a closure that applies the result
/// to a `ParseDraft`.
///
/// Returns `None` if no clause matches at this position.
fn longest_match<'t>(
    tokens: &'t [SpannedToken],
    i: usize,
    // ctx: &ParseContext<'_>,
    now: DateTime<Local>,
    draft: &ParseDraft,
) -> Option<(usize, HighlightKind, Box<dyn FnOnce(&mut ParseDraft) + 't>)> {
    // Collect all matches; pick the longest.
    let mut best: Option<(usize, HighlightKind, Box<dyn FnOnce(&mut ParseDraft)>)> = None;

    macro_rules! consider {
        ($len:expr, $kind:expr, $apply:expr) => {
            if $len > 0 {
                if best.as_ref().map_or(true, |(b, _, _)| $len > *b) {
                    best = Some(($len, $kind, Box::new($apply)));
                }
            }
        };
    }

    // // --- Sigil forms (unambiguous, always highest priority) ---
    // if matches!(tokens[i].token, Token::At) {
    //     if let Some((when, len)) = try_sigil_time(tokens, i + 1, now.with_timezone(&Utc)) {
    //         // The @ sigil always produces a full datetime.
    //         consider!(1 + len, HighlightKind::When, move |d: &mut ParseDraft| {
    //             d.when = Some(WhenSpec::DateTime(when))
    //         });
    //     }
    // }
    // if matches!(tokens[i].token, Token::Tilde) {
    //     if let Some((dur, len)) = try_sigil_duration(tokens, i + 1) {
    //         consider!(
    //             1 + len,
    //             HighlightKind::Duration,
    //             move |d: &mut ParseDraft| { d.duration = Some(dur) }
    //         );
    //     }
    // }
    // if matches!(tokens[i].token, Token::Percent) {
    //     if let Some((rec, len)) = try_sigil_recurrence(tokens, i + 1) {
    //         consider!(
    //             1 + len,
    //             HighlightKind::Recurrence,
    //             move |d: &mut ParseDraft| { d.recurrence = Some(rec) }
    //         );
    //     }
    // }
    // if matches!(tokens[i].token, Token::Bang) {
    //     if let Some((pri, len)) = try_sigil_priority(tokens, i + 1) {
    //         consider!(
    //             1 + len,
    //             HighlightKind::Priority,
    //             move |d: &mut ParseDraft| { d.priority = Some(pri) }
    //         );
    //     }
    // }
    // if matches!(tokens[i].token, Token::Hash) {
    //     if let Some((tag, len)) = try_sigil_tag(tokens, i + 1) {
    //         consider!(1 + len, HighlightKind::Tag, move |d: &mut ParseDraft| {
    //             d.tags.push(tag)
    //         });
    //     }
    // }
    // if matches!(tokens[i].token, Token::Amp) {
    //     if let Some((loc, len)) = try_sigil_location(tokens, i + 1) {
    //         consider!(
    //             1 + len,
    //             HighlightKind::Location,
    //             move |d: &mut ParseDraft| { d.location = Some(loc) }
    //         );
    //     }
    // }

    // --- Natural-language clauses ---

    // Only try NL time if we don't already have one.
    if draft.when.is_none() {
        if let Some((when, len)) = try_nl_when(
            tokens,
            i,
            draft.kind.clone(),
            now.with_timezone(&Utc),
            now.date_naive(),
        ) {
            consider!(len, HighlightKind::When, move |d: &mut ParseDraft| {
                d.when = Some(when)
            });
        }
    }

    if draft.recurrence.is_none() {
        if let Some((rec, len)) = try_nl_recurrence(tokens, i) {
            consider!(len, HighlightKind::Recurrence, move |d: &mut ParseDraft| {
                d.recurrence = Some(rec)
            });
        }
    }

    if draft.duration.is_none() {
        if let Some((dur, len)) = try_nl_duration(tokens, i) {
            consider!(len, HighlightKind::Duration, move |d: &mut ParseDraft| {
                d.duration = Some(dur)
            });
        }
    }

    // if draft.location.is_none() {
    //     if let Some((loc, len)) = try_nl_location(tokens, i, ctx) {
    //         consider!(len, HighlightKind::Location, move |d: &mut ParseDraft| {
    //             d.location = Some(loc)
    //         });
    //     }
    // }

    // if let Some((names, len)) = try_nl_people(tokens, i, ctx) {
    //     consider!(len, HighlightKind::People, move |d: &mut ParseDraft| {
    //         d.people.extend(names)
    //     });
    // }

    // if let Some((tag, len)) = try_nl_tag(tokens, i, ctx) {
    //     consider!(len, HighlightKind::Tag, move |d: &mut ParseDraft| {
    //         d.tags.push(tag)
    //     });
    // }

    // if draft.priority.is_none() {
    //     if let Some((pri, len)) = try_nl_priority(tokens, i) {
    //         consider!(len, HighlightKind::Priority, move |d: &mut ParseDraft| {
    //             d.priority = Some(pri)
    //         });
    //     }
    // }

    best
}

// ---------------------------------------------------------------------------
// Title assembly
// ---------------------------------------------------------------------------

fn assemble_title(draft: &mut ParseDraft, tokens: &[SpannedToken], consumed: &[bool]) {
    let mut words: Vec<String> = Vec::new();
    for (idx, tok) in tokens.iter().enumerate() {
        if consumed[idx] {
            continue;
        }
        match tok.token {
            Token::Word | Token::Number | Token::Quoted => {
                words.push(strip_quotes(&tok.text));
            }
            // Drop punct and bare sigils that had no clause match.
            _ => {}
        }
    }
    draft.title = words.join(" ").trim().to_string();
}

// // ---------------------------------------------------------------------------
// // Sigil clause handlers
// // ---------------------------------------------------------------------------
// // Each takes `tokens` starting AFTER the sigil and returns
// // `Some((value, tokens_consumed_after_sigil))` or `None`.

// fn try_sigil_time(
//     tokens: &[SpannedToken],
//     start: usize,
//     now: DateTime<Utc>,
// ) -> Option<(DateTime<Utc>, usize)> {
//     // `now` is UTC; date_at() converts naive-local → UTC correctly.
//     // Try up to 4 tokens (date + time can be 2; with keyword 3; padding for
//     // day-name + time combos).
//     for len in (1..=usize::min(4, tokens.len().saturating_sub(start))).rev() {
//         let text = join_adjacent(tokens, start, len);
//         if let Some(dt) = parse_datetime_expr(&text, now) {
//             return Some((dt, len));
//         }
//     }
//     None
// }

// fn try_sigil_duration(tokens: &[SpannedToken], start: usize) -> Option<(Duration, usize)> {
//     for len in (1..=usize::min(3, tokens.len().saturating_sub(start))).rev() {
//         let text = join_adjacent(tokens, start, len);
//         if let Some(dur) = parse_duration_expr(&text) {
//             return Some((dur, len));
//         }
//     }
//     None
// }

// fn try_sigil_recurrence(tokens: &[SpannedToken], start: usize) -> Option<(RecurrenceSpec, usize)> {
//     for len in (1..=usize::min(5, tokens.len().saturating_sub(start))).rev() {
//         let text = join_adjacent(tokens, start, len);
//         if let Some(rec) = try_recurrence_text(&text) {
//             return Some((rec, len));
//         }
//     }
//     None
// }

// fn try_sigil_priority(tokens: &[SpannedToken], start: usize) -> Option<(Priority, usize)> {
//     if start >= tokens.len() {
//         return None;
//     }
//     let text = tokens[start].text.to_ascii_lowercase();
//     match text.as_str() {
//         "high" | "urgent" | "critical" => Some((Priority::High, 1)),
//         "medium" | "normal" => Some((Priority::Medium, 1)),
//         "low" => Some((Priority::Low, 1)),
//         _ => None,
//     }
// }

// fn try_sigil_tag(tokens: &[SpannedToken], start: usize) -> Option<(String, usize)> {
//     if start >= tokens.len() {
//         return None;
//     }
//     match tokens[start].token {
//         Token::Word => Some((tokens[start].text.to_ascii_lowercase(), 1)),
//         Token::Quoted => Some((strip_quotes(&tokens[start].text), 1)),
//         _ => None,
//     }
// }

// fn try_sigil_location(tokens: &[SpannedToken], start: usize) -> Option<(String, usize)> {
//     if start >= tokens.len() {
//         return None;
//     }
//     match tokens[start].token {
//         Token::Word => {
//             // Collect consecutive words (e.g. `&the office` → "the office")
//             let mut end = start + 1;
//             while end < tokens.len() && matches!(tokens[end].token, Token::Word) {
//                 end += 1;
//             }
//             let text = tokens[start..end]
//                 .iter()
//                 .map(|t| t.text.as_str())
//                 .collect::<Vec<_>>()
//                 .join(" ");
//             Some((text.to_ascii_lowercase(), end - start))
//         }
//         Token::Quoted => Some((strip_quotes(&tokens[start].text), 1)),
//         _ => None,
//     }
// }

// ---------------------------------------------------------------------------
// Natural-language: Time
// ---------------------------------------------------------------------------

/// Try to match a time/date expression starting at `tokens[i]`.
///
/// Returns `(datetime, tokens_consumed)` on success.
///
/// Resolution priority (longest-match is tried first):
///   - `<day/keyword> at <time>` — e.g. "tomorrow at 3pm"
///   - `<day/keyword> <time>`    — e.g. "tomorrow 3pm"
///   - `at <time>`               — e.g. "at 3pm"
///   - `on <day>`                — e.g. "on tuesday"
///   - `in N unit`               — relative: "in 2 hours"
///   - bare time token           — e.g. "3pm", "14:30"
///   - bare date token           — e.g. "2025-06-15"
///   - bare keyword              — "today", "tomorrow", "tonight", etc.
///
/// For actions, date-only expressions (no explicit time) produce a
/// `WhenSpec::NaiveDate`; the scheduling time is left floating.
/// For events (and any expression that includes an explicit time), a full
/// `WhenSpec::DateTime` is always returned.
fn try_nl_when(
    tokens: &[SpannedToken],
    i: usize,
    kind: EntityKind,
    now: DateTime<Utc>,
    today: chrono::NaiveDate,
) -> Option<(WhenSpec, usize)> {
    // Helper: wrap a NaiveDate result according to entity kind.
    // Actions get a floating NaiveDate; events get pinned to default_time().
    let date_spec = |d: NaiveDate| -> WhenSpec {
        match kind {
            EntityKind::Action => WhenSpec::NaiveDate(d),
            EntityKind::Event => WhenSpec::DateTime(date_at(d, default_time())),
            EntityKind::RoutineStep => WhenSpec::NaiveDate(d),
        }
    };

    // `now` is UTC (used only for relative arithmetic like "in 2 hours").
    // `today` is the user's local date (used for "today", "tomorrow", weekday
    // anchoring, etc.) so that e.g. "today" means the user's calendar day.
    let n = tokens.len();
    if i >= n {
        return None;
    }

    let lower = tokens[i].text.to_ascii_lowercase();

    // ---- "<time> <day/keyword>" — e.g. "3pm tomorrow", "6pm next thu" ----
    // Try this FIRST so it wins over bare time token when a date follows.
    // A time+date combo always carries an explicit time → DateTime.
    if matches!(tokens[i].token, Token::Time12 | Token::Time24) {
        if let Some((time, _time_len)) = try_time_token(tokens, i, now) {
            // Look ahead for a date anchor
            if let Some(date) = try_date_anchor(tokens, i + 1, now, today) {
                return Some((WhenSpec::DateTime(date_at(date.0, time)), 1 + date.1));
            }
        }
    }

    // ---- "in N hours/minutes/days" ----
    if lower == "in" && i + 2 < n {
        if let Token::Number = tokens[i + 1].token {
            let unit = tokens[i + 2].text.to_ascii_lowercase();
            let amount: i64 = tokens[i + 1].text.parse().ok()?;
            let spec = match unit.as_str() {
                // Relative time offsets always produce a full DateTime.
                "hour" | "hours" | "hr" | "hrs" => {
                    Some(WhenSpec::DateTime(now + Duration::hours(amount)))
                }
                "minute" | "minutes" | "min" | "mins" => {
                    Some(WhenSpec::DateTime(now + Duration::minutes(amount)))
                }
                // Day/week offsets are date-only for actions.
                "day" | "days" => {
                    let date = today + chrono::Days::new(amount as u64);
                    Some(date_spec(date))
                }
                "week" | "weeks" => {
                    let date = today + chrono::Days::new(amount as u64 * 7);
                    Some(date_spec(date))
                }
                _ => None,
            };
            if let Some(spec) = spec {
                return Some((spec, 3));
            }
        }
    }

    // ---- "this morning/afternoon/evening" ----
    // These include a time-of-day component → always DateTime.
    if lower == "this" && i + 1 < n {
        let next = tokens[i + 1].text.to_ascii_lowercase();
        let time_opt = match next.as_str() {
            "morning" => Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
            "afternoon" => Some(NaiveTime::from_hms_opt(14, 0, 0).unwrap()),
            "evening" => Some(NaiveTime::from_hms_opt(20, 0, 0).unwrap()),
            "night" => Some(NaiveTime::from_hms_opt(21, 0, 0).unwrap()),
            _ => None,
        };
        if let Some(t) = time_opt {
            return Some((WhenSpec::DateTime(date_at(today, t)), 2));
        }
        // "this <weekday>" and "this week" — date-only for actions.
        if let Some(day) = parse_weekday_name(&next) {
            let date = this_or_next_weekday(today, day, false);
            return Some((date_spec(date), 2));
        }
        if next == "week" {
            let date = today + chrono::Days::new(7);
            return Some((date_spec(date), 2));
        }
    }

    // ---- "next <weekday/week>" ----
    if lower == "next" && i + 1 < n {
        let next = tokens[i + 1].text.to_ascii_lowercase();
        if let Some(day) = parse_weekday_name(&next) {
            let date = next_weekday_strict(today, day);
            // Try to grab an optional time after: "next monday at 3pm" / "next monday 3pm"
            if let Some((time, extra)) = try_time_suffix(tokens, i + 2, now) {
                return Some((WhenSpec::DateTime(date_at(date, time)), 2 + extra));
            }
            return Some((date_spec(date), 2));
        }
        if next == "week" {
            let date = today + chrono::Days::new(7);
            return Some((date_spec(date), 2));
        }
    }

    // ---- bare weekday name ----
    if let Some(day) = parse_weekday_name(&lower) {
        let date = next_weekday_strict(today, day);
        if let Some((time, extra)) = try_time_suffix(tokens, i + 1, now) {
            return Some((WhenSpec::DateTime(date_at(date, time)), 1 + extra));
        }
        return Some((date_spec(date), 1));
    }

    // ---- "tonight" — always has implied time → DateTime ----
    if lower == "tonight" {
        let t = NaiveTime::from_hms_opt(20, 0, 0).unwrap();
        return Some((WhenSpec::DateTime(date_at(today, t)), 1));
    }

    // ---- "later" / "soon" — implied times → DateTime ----
    if lower == "later" {
        let t = NaiveTime::from_hms_opt(14, 0, 0).unwrap();
        return Some((WhenSpec::DateTime(date_at(today, t)), 1));
    }
    if lower == "soon" {
        let t = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
        let date = today + chrono::Days::new(1);
        return Some((WhenSpec::DateTime(date_at(date, t)), 1));
    }

    // ---- "today" with optional time suffix ----
    if lower == "today" {
        if let Some((time, extra)) = try_time_suffix(tokens, i + 1, now) {
            return Some((WhenSpec::DateTime(date_at(today, time)), 1 + extra));
        }
        // No explicit time → date-only for actions.
        return Some((date_spec(today), 1));
    }

    // ---- "tomorrow" / "tom" with optional time suffix ----
    if lower == "tomorrow" || lower == "tom" {
        let date = today + chrono::Days::new(1);
        if let Some((time, extra)) = try_time_suffix(tokens, i + 1, now) {
            return Some((WhenSpec::DateTime(date_at(date, time)), 1 + extra));
        }
        // No explicit time → date-only for actions.
        return Some((date_spec(date), 1));
    }

    // ---- "at <time>" or "at noon/midnight/morning/…" (preposition form) ----
    // "at" always implies a time → DateTime.
    if lower == "at" && i + 1 < n {
        // Named times — all of them are valid after "at"
        let next_lower = tokens[i + 1].text.to_ascii_lowercase();
        if let Some(t) = parse_named_time(&next_lower) {
            return Some((WhenSpec::DateTime(date_at(today, t)), 2));
        }
        // Clock token
        if let Some((time, extra)) = try_time_token(tokens, i + 1, now) {
            return Some((WhenSpec::DateTime(date_at(today, time)), 1 + extra));
        }
    }

    // ---- bare named time: only noon/midnight are unambiguous as bare words ----
    // morning/afternoon/evening/night are too likely to be title words, so they
    // only fire when preceded by "at" or "this" (handled above).
    // Named times always carry an explicit time-of-day → DateTime.
    if let Some(t) = parse_bare_named_time(&lower) {
        return Some((WhenSpec::DateTime(date_at(today, t)), 1));
    }

    // ---- "on <weekday>" ----
    if lower == "on" && i + 1 < n {
        let next = tokens[i + 1].text.to_ascii_lowercase();
        if let Some(day) = parse_weekday_name(&next) {
            let date = next_weekday_strict(today, day);
            if let Some((time, extra)) = try_time_suffix(tokens, i + 2, now) {
                return Some((WhenSpec::DateTime(date_at(date, time)), 2 + extra));
            }
            return Some((date_spec(date), 2));
        }
    }

    // ---- Month + day: "june 15", "june 15th" ----
    if let Some(month) = parse_month_name(&lower) {
        if i + 1 < n {
            let next_lower = tokens[i + 1].text.to_ascii_lowercase();
            let day_opt = if matches!(tokens[i + 1].token, Token::OrdinalDay) {
                parse_ordinal_number(&tokens[i + 1].text)
            } else if matches!(tokens[i + 1].token, Token::Number) {
                tokens[i + 1].text.parse::<u32>().ok()
            } else if matches!(tokens[i + 1].token, Token::Word) {
                // "15th" may lex as Word if ordinal suffix folds in
                parse_ordinal_number(&next_lower).or_else(|| next_lower.parse::<u32>().ok())
            } else {
                None
            };
            if let Some(day) = day_opt {
                let year = next_month_day_year(today, month, day);
                if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                    if let Some((time, extra)) = try_time_suffix(tokens, i + 2, now) {
                        return Some((WhenSpec::DateTime(date_at(date, time)), 2 + extra));
                    }
                    return Some((date_spec(date), 2));
                }
            }
        }
    }

    // ---- "the <ordinal>" — e.g. "the 15th" ----
    if lower == "the" && i + 1 < n {
        let next = &tokens[i + 1];
        let day_opt = if matches!(next.token, Token::OrdinalDay) {
            parse_ordinal_number(&next.text)
        } else {
            None
        };
        if let Some(day) = day_opt {
            if let Some(date) = next_month_with_day(today, day) {
                if let Some((time, extra)) = try_time_suffix(tokens, i + 2, now) {
                    return Some((WhenSpec::DateTime(date_at(date, time)), 2 + extra));
                }
                return Some((date_spec(date), 2));
            }
        }
    }

    // ---- Rfc3339 token — always a full DateTime ----
    if matches!(tokens[i].token, Token::Rfc3339) {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&tokens[i].text) {
            return Some((WhenSpec::DateTime(dt.with_timezone(&Utc)), 1));
        }
    }

    // ---- IsoDate token ----
    if matches!(tokens[i].token, Token::IsoDate) {
        if let Ok(date) = NaiveDate::parse_from_str(&tokens[i].text, "%Y-%m-%d") {
            // Try to grab time immediately after; if present → DateTime.
            if let Some((time, extra)) = try_time_token(tokens, i + 1, now) {
                return Some((WhenSpec::DateTime(date_at(date, time)), 1 + extra));
            }
            return Some((date_spec(date), 1));
        }
    }

    // ---- Bare time token (Time12 or Time24) — always DateTime ----
    if let Some((time, len)) = try_time_token(tokens, i, now) {
        return Some((WhenSpec::DateTime(date_at(today, time)), len));
    }

    None
}

/// Try to read a time value (NaiveTime) from `tokens[start]`.
/// Returns `(time, tokens_consumed)`.
fn try_time_token(
    tokens: &[SpannedToken],
    start: usize,
    _now: DateTime<Utc>,
) -> Option<(NaiveTime, usize)> {
    if start >= tokens.len() {
        return None;
    }
    match tokens[start].token {
        Token::Time12 => {
            let t = parse_time12(&tokens[start].text)?;
            Some((t, 1))
        }
        Token::Time24 => {
            let t = NaiveTime::parse_from_str(&tokens[start].text, "%H:%M").ok()?;
            Some((t, 1))
        }
        _ => None,
    }
}

/// Try to read an optional time suffix: `at <time>` or bare `<time>`.
/// Used after a date expression like "tomorrow" or "next monday".
fn try_time_suffix(
    tokens: &[SpannedToken],
    start: usize,
    now: DateTime<Utc>,
) -> Option<(NaiveTime, usize)> {
    if start >= tokens.len() {
        return None;
    }
    let lower = tokens[start].text.to_ascii_lowercase();

    if lower == "at" && start + 1 < tokens.len() {
        if let Some((t, extra)) = try_time_token(tokens, start + 1, now) {
            return Some((t, 1 + extra)); // "at" + time token(s)
        }
        return None;
    }
    try_time_token(tokens, start, now)
}

/// Try to parse a date anchor (not a full datetime, just a date) starting at
/// `tokens[start]`.  Returns `(NaiveDate, tokens_consumed)`.
///
/// Recognised anchors: `today`, `tomorrow`, `tonight`, weekday names,
/// `next <weekday>`, ISO-date tokens.
fn try_date_anchor(
    tokens: &[SpannedToken],
    start: usize,
    _now: DateTime<Utc>,
    today: chrono::NaiveDate,
) -> Option<(NaiveDate, usize)> {
    if start >= tokens.len() {
        return None;
    }
    let lower = tokens[start].text.to_ascii_lowercase();

    match lower.as_str() {
        "today" | "tonight" => return Some((today, 1)),
        "tomorrow" | "tom" => return Some((today + Duration::days(1), 1)),
        _ => {}
    }

    if let Some(day) = parse_weekday_name(&lower) {
        return Some((next_weekday_strict(today, day), 1));
    }

    if lower == "next" && start + 1 < tokens.len() {
        let next = tokens[start + 1].text.to_ascii_lowercase();
        if let Some(day) = parse_weekday_name(&next) {
            return Some((next_weekday_strict(today, day), 2));
        }
        if next == "week" {
            return Some((today + Duration::days(7), 2));
        }
    }

    if matches!(tokens[start].token, Token::IsoDate) {
        if let Ok(d) = NaiveDate::parse_from_str(&tokens[start].text, "%Y-%m-%d") {
            return Some((d, 1));
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Natural-language: Recurrence
// ---------------------------------------------------------------------------

fn try_nl_recurrence(tokens: &[SpannedToken], i: usize) -> Option<(RecurrenceSpec, usize)> {
    let n = tokens.len();
    if i >= n {
        return None;
    }
    let lower = tokens[i].text.to_ascii_lowercase();

    // Single-word keywords.
    match lower.as_str() {
        "daily" => return Some((RecurrenceSpec::EveryDays(1), 1)),
        "weekly" => return Some((RecurrenceSpec::EveryWeeks(1), 1)),
        "monthly" => return Some((RecurrenceSpec::EveryMonths(1), 1)),
        "yearly" | "annually" => return Some((RecurrenceSpec::EveryYears(1), 1)),
        "weekdays" => return Some((RecurrenceSpec::weekdays(), 1)),
        "weekends" => return Some((RecurrenceSpec::weekends(), 1)),
        _ => {}
    }

    // "every ..."
    if lower == "every" && i + 1 < n {
        return try_every_clause(tokens, i);
    }

    // "<Nth> of every month" / "<Nth> of the month"
    // Tokens: OrdinalDay  "of"  "every"/"the"  "month"
    if matches!(tokens[i].token, Token::OrdinalDay) {
        if let Some(day) = parse_ordinal_number(&tokens[i].text) {
            if (1..=31).contains(&day) {
                // Try "of every month" (3 tokens) first, then "of the month"
                let suffixes: &[(&str, &str, usize)] = &[
                    ("of", "every", 3), // "<Nth> of every month" → 4 tokens total
                    ("of", "the", 3),   // "<Nth> of the month"   → 4 tokens total
                ];
                for &(word1, word2, extra) in suffixes {
                    if i + extra < n
                        && tokens[i + 1].text.to_ascii_lowercase() == word1
                        && tokens[i + 2].text.to_ascii_lowercase() == word2
                        && tokens[i + 3].text.to_ascii_lowercase() == "month"
                    {
                        return Some((RecurrenceSpec::OnMonthDay(day), 1 + extra));
                    }
                }
            }
        }
    }

    // Comma-separated short day list: "mon,wed,fri"
    // Only fires if the token text itself contains a comma.
    if lower.contains(',') {
        if let Some(rec) = try_recurrence_text(&lower) {
            return Some((rec, 1));
        }
    }

    None
}

fn try_every_clause(tokens: &[SpannedToken], i: usize) -> Option<(RecurrenceSpec, usize)> {
    // Only consume tokens that could plausibly be part of a recurrence spec.
    // Stop as soon as we hit a hard clause-boundary word ("at", "for", "in",
    // "with", "lasting", "takes") or a non-Word/Number/Number/OrdinalDay/Punct token.
    const STOP_WORDS: &[&str] = &["at", "for", "in", "with", "lasting", "takes", "by"];

    let mut end = i + 1; // start after "every"
    while end < tokens.len() {
        match tokens[end].token {
            Token::Word | Token::Number | Token::OrdinalDay => {
                let lower = tokens[end].text.to_ascii_lowercase();
                if STOP_WORDS.contains(&lower.as_str()) {
                    break;
                }
                end += 1;
            }
            Token::Punct => {
                // Allow commas (day-list separator), stop on anything else
                if tokens[end].text == "," {
                    end += 1;
                } else {
                    break;
                }
            }
            _ => break,
        }
        // Safety cap: never look more than 8 tokens ahead
        if end > i + 8 {
            break;
        }
    }

    // Try longest spans first, down to "every <something>" (minimum 2 tokens)
    for span_end in (i + 2..=end).rev() {
        let text = tokens[i..span_end]
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let text_lower = text.to_ascii_lowercase();
        if let Some(rec) = try_recurrence_text(&text_lower) {
            return Some((rec, span_end - i));
        }
    }
    None
}

/// Parse a recurrence specifier from a plain string (no sigil, no tokens).
///
/// This is the canonical recogniser used by both the sigil handler and the
/// natural-language scanner.
pub fn try_recurrence_text(text: &str) -> Option<RecurrenceSpec> {
    let t = text.trim();
    match t {
        "daily" => return Some(RecurrenceSpec::EveryDays(1)),
        "weekly" => return Some(RecurrenceSpec::EveryWeeks(1)),
        "monthly" | "every month" => return Some(RecurrenceSpec::EveryMonths(1)),
        "yearly" | "annually" | "every year" => return Some(RecurrenceSpec::EveryYears(1)),
        "quarterly" => return Some(RecurrenceSpec::EveryMonths(3)),
        "biweekly" | "fortnightly" => return Some(RecurrenceSpec::EveryWeeks(2)),
        "weekdays" | "every weekday" | "every weekdays" => return Some(RecurrenceSpec::weekdays()),
        "weekends" | "every weekend" | "every weekends" => return Some(RecurrenceSpec::weekends()),
        "every day" => return Some(RecurrenceSpec::EveryDays(1)),
        "every week" => return Some(RecurrenceSpec::EveryWeeks(1)),
        _ => {}
    }

    if let Some(rest) = t.strip_prefix("every ") {
        // "every N days"
        if let Some(inner) = rest.strip_suffix(" days") {
            if let Ok(n) = inner.trim().parse::<i64>() {
                return Some(RecurrenceSpec::EveryDays(n));
            }
        }
        // "every N weeks"
        if let Some(inner) = rest.strip_suffix(" weeks") {
            if let Ok(n) = inner.trim().parse::<i64>() {
                return Some(RecurrenceSpec::EveryWeeks(n));
            }
        }
        // "every N months"
        if let Some(inner) = rest.strip_suffix(" months") {
            if let Ok(n) = inner.trim().parse::<i64>() {
                return Some(RecurrenceSpec::EveryMonths(n));
            }
        }
        // "every N years"
        if let Some(inner) = rest.strip_suffix(" years") {
            if let Ok(n) = inner.trim().parse::<i64>() {
                return Some(RecurrenceSpec::EveryYears(n));
            }
        }

        // "every 15th", "every 1st", "every 30th" — bare ordinal
        if let Some(day) = parse_ordinal_number(rest) {
            if (1..=31).contains(&day) {
                return Some(RecurrenceSpec::OnMonthDay(day));
            }
        }

        // "every <N>th of the month", "every <N>th of every month"
        // Accept: "15th of the month", "1st of every month", "30th of the month"
        if let Some(ordinal_part) = rest
            .strip_suffix(" of the month")
            .or_else(|| rest.strip_suffix(" of every month"))
        {
            if let Some(day) = parse_ordinal_number(ordinal_part.trim()) {
                if (1..=31).contains(&day) {
                    return Some(RecurrenceSpec::OnMonthDay(day));
                }
            }
        }

        // "every monday", "every thu, fri", "every mon, wed, fri"
        let days = parse_day_list(rest);
        if !days.is_empty() {
            return Some(RecurrenceSpec::OnWeekdays(WeekdaySet::new(days)));
        }
    }

    // "<N>st/nd/rd/th of every month" — e.g. "1st of every month"
    if let Some(ordinal_part) = t.strip_suffix(" of every month") {
        if let Some(day) = parse_ordinal_number(ordinal_part.trim()) {
            if (1..=31).contains(&day) {
                return Some(RecurrenceSpec::OnMonthDay(day));
            }
        }
    }
    // "<N>st/nd/rd/th of the month"
    if let Some(ordinal_part) = t.strip_suffix(" of the month") {
        if let Some(day) = parse_ordinal_number(ordinal_part.trim()) {
            if (1..=31).contains(&day) {
                return Some(RecurrenceSpec::OnMonthDay(day));
            }
        }
    }

    // Bare comma-separated list: "mon,wed,fri"
    if t.contains(',') {
        let days = parse_day_list(t);
        if days.len() >= 2 {
            return Some(RecurrenceSpec::OnWeekdays(WeekdaySet::new(days)));
        }
    }

    None
}

/// Parse a comma-separated or space-and-comma-separated list of day names.
/// Accepts both long and short forms, with and without trailing plurals.
///
/// Returns an empty `Vec` if any non-empty, non-connector token is NOT a
/// valid day name — this prevents partial matches from consuming unrelated
/// words like "for" or "at".
fn parse_day_list(text: &str) -> Vec<Weekday> {
    let mut days = Vec::new();
    for part in text.split(|c: char| c == ',' || c == ' ') {
        let s = part.trim();
        if s.is_empty() {
            continue;
        }
        match parse_weekday_name(s) {
            Some(day) => days.push(day),
            // Any unrecognised token invalidates the whole list
            None => return Vec::new(),
        }
    }
    days
}

// ---------------------------------------------------------------------------
// Natural-language: Duration
// ---------------------------------------------------------------------------

fn try_nl_duration(tokens: &[SpannedToken], i: usize) -> Option<(Duration, usize)> {
    let n = tokens.len();
    if i >= n {
        return None;
    }
    let lower = tokens[i].text.to_ascii_lowercase();

    // "for <duration>", "lasting <duration>", "takes <duration>"
    let intro_consumed = if matches!(lower.as_str(), "for" | "lasting" | "takes") {
        1usize
    } else {
        0
    };
    let dur_start = i + intro_consumed;

    if dur_start >= n {
        return None;
    }

    // Try up to 4 tokens for the duration expression (e.g. "1 hour 30 mins")
    for len in (1..=usize::min(4, n - dur_start)).rev() {
        let text = join_adjacent(tokens, dur_start, len);
        if let Some(dur) = parse_duration_expr(&text) {
            return Some((dur, intro_consumed + len));
        }
    }

    // If we had an intro word but no duration followed, don't consume anything
    None
}

/// Parse a duration string like `30m`, `1h30m`, `1h 30m`, `1:30`, `2 hours`.
pub fn parse_duration_expr(text: &str) -> Option<Duration> {
    let t = text.trim();

    // // "H:MM" colon form → hours + minutes
    // if let Some(dur) = parse_colon_duration(t) {
    //     return Some(dur);
    // }

    // Combined "1h30m" or "1h 30m" form
    if let Some(dur) = parse_combined_duration(t) {
        return Some(dur);
    }

    // Simple "Nunit" forms
    parse_simple_duration(t)
}

// fn parse_colon_duration(text: &str) -> Option<Duration> {
//     // "1:30" → 1h30m, "0:45" → 45m
//     let parts: Vec<&str> = text.splitn(2, ':').collect();
//     if parts.len() != 2 {
//         return None;
//     }
//     let h: i64 = parts[0].parse().ok()?;
//     let m: i64 = parts[1].parse().ok()?;
//     if m >= 60 {
//         return None;
//     }
//     Some(Duration::hours(h) + Duration::minutes(m))
// }

fn parse_combined_duration(text: &str) -> Option<Duration> {
    // Matches patterns like "1h30m", "1h 30min", "1hr30mins", "2h 15m"
    // Split on 'h'/'hr'/'hrs'/'hour'/'hours'
    let lower = text.to_ascii_lowercase();

    // Find the hours part
    let h_split = lower
        .find("hours")
        .map(|p| (p, 5))
        .or_else(|| lower.find("hour").map(|p| (p, 4)))
        .or_else(|| lower.find("hrs").map(|p| (p, 3)))
        .or_else(|| lower.find("hr").map(|p| (p, 2)))
        .or_else(|| lower.find('h').map(|p| (p, 1)));

    let (h_end, h_suffix_len) = h_split?;
    let h_str = lower[..h_end].trim();
    let h: i64 = h_str.parse().ok()?;
    let after_h = lower[h_end + h_suffix_len..].trim();
    if after_h.is_empty() {
        return None; // pure hours — handled by simple parser
    }
    // Parse the minutes tail
    let m = parse_simple_duration(after_h)?;
    Some(Duration::hours(h) + m)
}

fn parse_simple_duration(text: &str) -> Option<Duration> {
    let lower = text.trim().to_ascii_lowercase();

    // Try longest suffix first to avoid "m" matching "mins"
    let (n_str, unit): (&str, &str) = if let Some(s) = lower.strip_suffix("minutes") {
        (s, "minutes")
    } else if let Some(s) = lower.strip_suffix("minute") {
        (s, "minute")
    } else if let Some(s) = lower.strip_suffix("mins") {
        (s, "mins")
    } else if let Some(s) = lower.strip_suffix("min") {
        (s, "min")
    } else if let Some(s) = lower.strip_suffix("hours") {
        (s, "hours")
    } else if let Some(s) = lower.strip_suffix("hour") {
        (s, "hour")
    } else if let Some(s) = lower.strip_suffix("hrs") {
        (s, "hrs")
    } else if let Some(s) = lower.strip_suffix("hr") {
        (s, "hr")
    } else if let Some(s) = lower.strip_suffix('h') {
        (s, "h")
    } else if let Some(s) = lower.strip_suffix('m') {
        (s, "m")
    } else {
        return None;
    };
    let _ = unit;
    let n: i64 = n_str.trim().parse().ok()?;
    match unit {
        "h" | "hr" | "hrs" | "hour" | "hours" => Some(Duration::hours(n)),
        "m" | "min" | "mins" | "minute" | "minutes" => Some(Duration::minutes(n)),
        _ => None,
    }
}

// // ---------------------------------------------------------------------------
// // Natural-language: Location
// // ---------------------------------------------------------------------------

// fn try_nl_location<'a>(
//     tokens: &[SpannedToken],
//     i: usize,
//     ctx: &ParseContext<'a>,
// ) -> Option<(String, usize)> {
//     let n = tokens.len();
//     if i >= n {
//         return None;
//     }
//     let lower = tokens[i].text.to_ascii_lowercase();

//     // "at <location>" or "in <location>" — only if NOT a time expression
//     if (lower == "at" || lower == "in") && i + 1 < n {
//         // Don't steal "at <time>" from the time clause — check the next token
//         if matches!(tokens[i + 1].token, Token::Time12 | Token::Time24) {
//             return None;
//         }

//         // Try to match "at <multi-word-location>"
//         let loc = try_match_location(tokens, i + 1, ctx)?;
//         return Some((loc.0, 1 + loc.1));
//     }

//     // Bare known location name
//     if !ctx.locations.is_empty() {
//         let loc = try_match_location(tokens, i, ctx)?;
//         return Some(loc);
//     }

//     None
// }

// /// Try to match a location from `tokens[start]` against the context list.
// /// Returns `(location_string, tokens_consumed)`.
// fn try_match_location(
//     tokens: &[SpannedToken],
//     start: usize,
//     ctx: &ParseContext<'_>,
// ) -> Option<(String, usize)> {
//     if start >= tokens.len() {
//         return None;
//     }
//     // Try matching longest spans first (multi-word locations)
//     for len in (1..=usize::min(4, tokens.len() - start)).rev() {
//         let candidate = tokens[start..start + len]
//             .iter()
//             .map(|t| t.text.as_str())
//             .collect::<Vec<_>>()
//             .join(" ")
//             .to_ascii_lowercase();
//         if ctx
//             .locations
//             .iter()
//             .any(|loc| loc.to_ascii_lowercase() == candidate)
//         {
//             return Some((candidate, len));
//         }
//     }
//     // Also accept a quoted string as a location
//     if matches!(tokens[start].token, Token::Quoted) {
//         return Some((strip_quotes(&tokens[start].text), 1));
//     }
//     None
// }

// // ---------------------------------------------------------------------------
// // Natural-language: People
// // ---------------------------------------------------------------------------

// fn try_nl_people<'a>(
//     tokens: &[SpannedToken],
//     i: usize,
//     ctx: &ParseContext<'a>,
// ) -> Option<(Vec<String>, usize)> {
//     let n = tokens.len();
//     if i >= n {
//         return None;
//     }
//     let lower = tokens[i].text.to_ascii_lowercase();

//     if lower != "with" || i + 1 >= n {
//         return None;
//     }

//     // Collect names: "with Isabel", "with Bob and Alice"
//     // Stop at hard clause-boundary words so "with Isabel at 8pm" doesn't
//     // consume "at" and "8pm" as person names.
//     const STOP_WORDS: &[&str] = &[
//         "at", "for", "by", "on", "in", "every", "daily", "weekly", "weekdays", "weekends",
//         "lasting", "takes", "urgent", "asap", "high", "low", "medium", "priority", "whenever",
//     ];

//     let mut names: Vec<String> = Vec::new();
//     let mut pos = i + 1;

//     loop {
//         if pos >= n {
//             break;
//         }
//         // Stop at non-word tokens (sigils, time tokens, etc.)
//         if !matches!(tokens[pos].token, Token::Word | Token::Quoted) {
//             break;
//         }
//         let word = tokens[pos].text.to_ascii_lowercase();
//         // Stop at clause-introducing words
//         if STOP_WORDS.contains(&word.as_str()) {
//             break;
//         }
//         // "and" is a connector between names — skip it but don't add it
//         if word == "and" {
//             pos += 1;
//             continue;
//         }
//         // Accept: known person, capitalised word (likely a proper name),
//         // or any word when a people list is provided (context-guided mode).
//         let is_known = ctx.people.iter().any(|p| p.to_ascii_lowercase() == word);
//         let is_capitalised = tokens[pos]
//             .text
//             .chars()
//             .next()
//             .map_or(false, |c| c.is_uppercase());
//         if is_known || is_capitalised {
//             names.push(tokens[pos].text.clone());
//             pos += 1;
//         } else {
//             break;
//         }
//     }

//     if names.is_empty() {
//         return None;
//     }
//     Some((names, pos - i))
// }

// // ---------------------------------------------------------------------------
// // Natural-language: Tags
// // ---------------------------------------------------------------------------

// fn try_nl_tag<'a>(
//     tokens: &[SpannedToken],
//     i: usize,
//     ctx: &ParseContext<'a>,
// ) -> Option<(String, usize)> {
//     if ctx.tags.is_empty() {
//         return None;
//     }
//     let lower = tokens[i].text.to_ascii_lowercase();
//     if ctx.tags.iter().any(|t| t.to_ascii_lowercase() == lower) {
//         return Some((lower, 1));
//     }
//     None
// }

// // ---------------------------------------------------------------------------
// // Natural-language: Priority
// // ---------------------------------------------------------------------------

// fn try_nl_priority(tokens: &[SpannedToken], i: usize) -> Option<(Priority, usize)> {
//     let n = tokens.len();
//     if i >= n {
//         return None;
//     }
//     let lower = tokens[i].text.to_ascii_lowercase();

//     match lower.as_str() {
//         "urgent" | "asap" | "critical" => return Some((Priority::High, 1)),
//         "whenever" => return Some((Priority::Low, 1)),
//         _ => {}
//     }

//     // "high priority", "low priority", "medium priority", "normal priority"
//     if i + 1 < n {
//         let next = tokens[i + 1].text.to_ascii_lowercase();
//         if next == "priority" {
//             match lower.as_str() {
//                 "high" => return Some((Priority::High, 2)),
//                 "medium" | "normal" => return Some((Priority::Medium, 2)),
//                 "low" => return Some((Priority::Low, 2)),
//                 _ => {}
//             }
//         }
//     }

//     // "not urgent" → Low
//     if lower == "not" && i + 1 < n {
//         let next = tokens[i + 1].text.to_ascii_lowercase();
//         if next == "urgent" {
//             return Some((Priority::Low, 2));
//         }
//     }

//     None
// }

// ---------------------------------------------------------------------------
// Datetime helpers
// ---------------------------------------------------------------------------

// fn parse_datetime_expr(text: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
//     let today = Local::now().date_naive();
//     let lower = text.trim().to_ascii_lowercase();

//     match lower.as_str() {
//         "now" => return Some(now),
//         "today" => return Some(date_at(today, default_time())),
//         "tomorrow" | "tom" => {
//             let d = today + chrono::Days::new(1);
//             return Some(date_at(d, default_time()));
//         }
//         "tonight" => {
//             let t = NaiveTime::from_hms_opt(20, 0, 0).unwrap();
//             return Some(date_at(today, t));
//         }
//         "later" => {
//             let t = NaiveTime::from_hms_opt(14, 0, 0).unwrap();
//             return Some(date_at(today, t));
//         }
//         _ => {}
//     }

//     // Try RFC-3339
//     if let Ok(dt) = DateTime::parse_from_rfc3339(text.trim()) {
//         return Some(dt.with_timezone(&Utc));
//     }

//     // ISO date + space + time: treat as local time (e.g. "@2025-06-15 14:30"
//     // means 2:30pm in the user's local timezone).
//     if let Ok(ndt) = NaiveDateTime::parse_from_str(text.trim(), "%Y-%m-%d %H:%M") {
//         return Some(date_at(ndt.date(), ndt.time()));
//     }

//     // ISO date alone
//     if let Ok(d) = NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d") {
//         return Some(date_at(d, default_time()));
//     }

//     // "today 14:30", "today 3pm", "tomorrow 9am" etc.
//     // "today", "tomorrow" / "tom" etc.
//     let parts: Vec<&str> = text.trim().splitn(2, ' ').collect();
//     if parts.len() == 2 {
//         let base_date = match parts[0].to_ascii_lowercase().as_str() {
//             "today" => Some(today),
//             "tomorrow" | "tom" => Some(today + chrono::Days::new(1)),
//             _ => None,
//         };
//         if let Some(date) = base_date {
//             if let Some(t) = parse_time_str(parts[1]) {
//                 return Some(date_at(date, t));
//             }
//         }
//     }

//     // A weekday name
//     if let Some(day) = parse_weekday_name(&lower) {
//         let date = next_weekday_strict(today, day);
//         return Some(date_at(date, default_time()));
//     }

//     None
// }

// /// Parse a time string in any supported format.
// fn parse_time_str(text: &str) -> Option<NaiveTime> {
//     let lower = text.trim().to_ascii_lowercase();
//     // "14:30"
//     if let Ok(t) = NaiveTime::parse_from_str(&lower, "%H:%M") {
//         return Some(t);
//     }
//     parse_time12(&lower)
// }

/// Recognise named times when preceded by "at" or "this":
/// noon, midnight, morning, afternoon, evening, night, midday.
fn parse_named_time(text: &str) -> Option<NaiveTime> {
    match text.trim() {
        "noon" | "midday" => NaiveTime::from_hms_opt(12, 0, 0),
        "midnight" => NaiveTime::from_hms_opt(0, 0, 0),
        "morning" => NaiveTime::from_hms_opt(9, 0, 0),
        "afternoon" => NaiveTime::from_hms_opt(14, 0, 0),
        "evening" => NaiveTime::from_hms_opt(20, 0, 0),
        "night" => NaiveTime::from_hms_opt(21, 0, 0),
        _ => None,
    }
}

/// Recognise unambiguous bare named times (no preposition required).
/// Only `noon` and `midnight` are safe to match without context — all others
/// (`morning`, `afternoon`, `evening`, `night`) are too likely to be title
/// words and only fire after "at" or "this".
fn parse_bare_named_time(text: &str) -> Option<NaiveTime> {
    match text.trim() {
        "noon" | "midday" => NaiveTime::from_hms_opt(12, 0, 0),
        "midnight" => NaiveTime::from_hms_opt(0, 0, 0),
        _ => None,
    }
}

/// Parse a 12-hour clock string like `3pm`, `10:30am`, `12pm`, `3p`, `11a`, `10:30p`.
///
/// Suffix priority: longer suffixes (`am`/`pm`) are tried before the bare
/// single-letter forms (`a`/`p`) so `"10am"` is never misread as `"10a" + "m"`.
fn parse_time12(text: &str) -> Option<NaiveTime> {
    let lower = text.trim().to_ascii_lowercase();

    // Two-letter suffixes first (longest match wins).
    if let Some(hm) = lower.strip_suffix("am") {
        let (h, m) = parse_hm(hm)?;
        let hour = if h == 12 { 0 } else { h };
        return NaiveTime::from_hms_opt(hour, m, 0);
    }
    if let Some(hm) = lower.strip_suffix("pm") {
        let (h, m) = parse_hm(hm)?;
        let hour = if h == 12 { 12 } else { h + 12 };
        if hour >= 24 {
            return None;
        }
        return NaiveTime::from_hms_opt(hour, m, 0);
    }

    // Single-letter suffixes: `3a` → 3 am, `3p` → 3 pm.
    if let Some(hm) = lower.strip_suffix('a') {
        let (h, m) = parse_hm(hm)?;
        let hour = if h == 12 { 0 } else { h };
        return NaiveTime::from_hms_opt(hour, m, 0);
    }
    if let Some(hm) = lower.strip_suffix('p') {
        let (h, m) = parse_hm(hm)?;
        let hour = if h == 12 { 12 } else { h + 12 };
        if hour >= 24 {
            return None;
        }
        return NaiveTime::from_hms_opt(hour, m, 0);
    }

    None
}

/// Parse "H" or "H:MM" into (hour, minute).
fn parse_hm(text: &str) -> Option<(u32, u32)> {
    if let Some((h, m)) = text.split_once(':') {
        Some((h.parse().ok()?, m.parse().ok()?))
    } else {
        Some((text.parse().ok()?, 0))
    }
}

fn default_time() -> NaiveTime {
    NaiveTime::from_hms_opt(9, 0, 0).unwrap()
}

/// Combine a local `NaiveDate` and local `NaiveTime` into a `DateTime<Utc>`.
///
/// The naive date+time is interpreted as wall-clock time in the system's local
/// timezone, then converted to UTC.  This is the single place where
/// user-supplied times (e.g. "3pm", "14:30", "tomorrow 9am") become UTC
/// instants — getting this right means the stored `when` field is always the
/// correct UTC moment regardless of the user's timezone.
///
/// For ambiguous local times (DST fall-back fold), `from_local_datetime`
/// returns `LocalResult::Ambiguous`; we take the earlier of the two
/// interpretations (pre-fold) via `.earliest()`.  For invalid local times
/// (DST spring-forward gap) we fall back to interpreting the naive datetime
/// as UTC, which is a rare and graceful degradation.
fn date_at(date: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
    let ndt = NaiveDateTime::new(date, time);
    match Local.from_local_datetime(&ndt) {
        chrono::LocalResult::Single(dt) => dt.with_timezone(&Utc),
        chrono::LocalResult::Ambiguous(earliest, _latest) => earliest.with_timezone(&Utc),
        chrono::LocalResult::None => {
            // Spring-forward gap: the local time doesn't exist.
            // Fall back to treating it as UTC (rare, graceful degradation).
            Utc.from_utc_datetime(&ndt)
        }
    }
}

// ---------------------------------------------------------------------------
// Weekday utilities
// ---------------------------------------------------------------------------

/// Parse long or short weekday names, with optional plural `s`.
pub fn parse_weekday_name(s: &str) -> Option<Weekday> {
    match s.trim_end_matches('s').to_ascii_lowercase().as_str() {
        "mon" | "monday" => Some(Weekday::Mon),
        "tue" | "tuesday" => Some(Weekday::Tue),
        "wed" | "wednesday" => Some(Weekday::Wed),
        "thu" | "thursday" => Some(Weekday::Thu),
        "fri" | "friday" => Some(Weekday::Fri),
        "sat" | "saturday" => Some(Weekday::Sat),
        "sun" | "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

/// Return the date of the next occurrence of `day` strictly after `from`.
/// E.g. if today is Monday and `day` is Monday, returns next Monday (7 days).
fn next_weekday_strict(from: NaiveDate, day: Weekday) -> NaiveDate {
    let mut d = from + Duration::days(1);
    while d.weekday() != day {
        d = d + Duration::days(1);
    }
    d
}

/// "this <weekday>": if `same_ok` is true return today if today matches;
/// otherwise behave like `next_weekday_strict`.
fn this_or_next_weekday(from: NaiveDate, day: Weekday, same_ok: bool) -> NaiveDate {
    if same_ok && from.weekday() == day {
        return from;
    }
    next_weekday_strict(from, day)
}

// ---------------------------------------------------------------------------
// Month / ordinal utilities
// ---------------------------------------------------------------------------

fn parse_month_name(s: &str) -> Option<u32> {
    match s.to_ascii_lowercase().as_str() {
        "january" | "jan" => Some(1),
        "february" | "feb" => Some(2),
        "march" | "mar" => Some(3),
        "april" | "apr" => Some(4),
        "may" => Some(5),
        "june" | "jun" => Some(6),
        "july" | "jul" => Some(7),
        "august" | "aug" => Some(8),
        "september" | "sep" | "sept" => Some(9),
        "october" | "oct" => Some(10),
        "november" | "nov" => Some(11),
        "december" | "dec" => Some(12),
        _ => None,
    }
}

/// Strip ordinal suffix and parse to a day number: "15th" → 15.
fn parse_ordinal_number(text: &str) -> Option<u32> {
    let lower = text.to_ascii_lowercase();
    let digits = lower
        .trim_end_matches("th")
        .trim_end_matches("st")
        .trim_end_matches("nd")
        .trim_end_matches("rd");
    digits.parse().ok()
}

/// Given a month+day, return the year such that the date is in the future.
fn next_month_day_year(from: NaiveDate, month: u32, day: u32) -> i32 {
    let this_year = from.year();
    if let Some(d) = NaiveDate::from_ymd_opt(this_year, month, day) {
        if d >= from {
            return this_year;
        }
    }
    this_year + 1
}

/// Given a day-of-month, return the nearest future date with that day.
fn next_month_with_day(from: NaiveDate, day: u32) -> Option<NaiveDate> {
    // Try current month
    if let Some(d) = NaiveDate::from_ymd_opt(from.year(), from.month(), day) {
        if d >= from {
            return Some(d);
        }
    }
    // Try next month
    let (next_year, next_month) = if from.month() == 12 {
        (from.year() + 1, 1)
    } else {
        (from.year(), from.month() + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, day)
}

// ---------------------------------------------------------------------------
// Token joining helpers
// ---------------------------------------------------------------------------

/// Join `len` tokens starting at `start`, using source adjacency to decide
/// whether to insert a space between them.  Adjacent tokens (no gap in the
/// original source) are concatenated directly; non-adjacent tokens get a
/// single space.
fn join_adjacent(tokens: &[SpannedToken], start: usize, len: usize) -> String {
    let slice = &tokens[start..start + len];
    let mut out = String::new();
    for (idx, tok) in slice.iter().enumerate() {
        if idx == 0 {
            out.push_str(&tok.text);
        } else {
            let prev = &slice[idx - 1];
            if prev.span.end == tok.span.start {
                out.push_str(&tok.text);
            } else {
                out.push(' ');
                out.push_str(&tok.text);
            }
        }
    }
    out
}

fn strip_quotes(text: &str) -> String {
    if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}
