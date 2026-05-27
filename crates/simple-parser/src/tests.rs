//! Comprehensive tests for the parser module.
//!
//! Structure:
//!   - `lexer`        — token recognition and span correctness
//!   - `helpers`      — shared test utilities
//!   - `title`        — title extraction (leading, trailing, mixed positions)
//!   - `at_clause`    — `@` datetime clause (all formats)
//!   - `tilde_clause` — `~` duration clause (all unit spellings)
//!   - `recurrence`   — `RecurrenceSpec` grammar via `parse_recurrence_str`
//!   - `weekday_set`  — `WeekdaySet` helpers
//!   - `parse_kind`   — `parse_action_input` vs `parse_event_input` carry correct kind
//!   - `content`      — content string assembly (metadata fields that are active)
//!   - `combinations` — multiple clauses together, various orderings
//!   - `errors`       — invalid / empty inputs
//!   - `build`        — `build_entity` for Action and Event

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[cfg(test)]
mod lexer {
    use crate::lexer::{Token, lex};

    #[test]
    fn single_word() {
        let tokens = lex("hello");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Word);
        assert_eq!(tokens[0].text, "hello");
    }

    #[test]
    fn sigils_recognised() {
        let tokens = lex("@ % ~ ! # &");
        let kinds: Vec<_> = tokens.iter().map(|t| t.token.clone()).collect();
        assert_eq!(
            kinds,
            vec![
                Token::At,
                Token::Percent,
                Token::Tilde,
                Token::Bang,
                Token::Hash,
                Token::Amp,
            ]
        );
    }

    #[test]
    fn number_token() {
        let tokens = lex("42");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Number);
        assert_eq!(tokens[0].text, "42");
    }

    #[test]
    fn quoted_token() {
        let tokens = lex(r#""hello world""#);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Quoted);
        assert_eq!(tokens[0].text, r#""hello world""#);
    }

    #[test]
    fn punct_tokens() {
        let tokens = lex(".,");
        assert_eq!(tokens.len(), 2);
        assert!(tokens.iter().all(|t| t.token == Token::Punct));
    }

    #[test]
    fn whitespace_is_skipped() {
        let tokens = lex("   foo   bar   ");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text, "foo");
        assert_eq!(tokens[1].text, "bar");
    }

    #[test]
    fn word_allows_hyphens() {
        // Hyphens are still part of Word; colons and slashes are no longer —
        // they are consumed by the dedicated Time24 / IsoDate tokens.
        let tokens = lex("foo-bar well-being");
        assert_eq!(tokens.len(), 2);
        assert!(tokens.iter().all(|t| t.token == Token::Word));
    }

    #[test]
    fn colon_in_time_produces_time24_token() {
        // "baz:qux" no longer lexes as a single Word because ':' is no longer
        // in the Word char class. It becomes Number + error-folded + Word or
        // similar — the key point is that colons now belong to Time24.
        let tokens = lex("14:30");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Time24);
        assert_eq!(tokens[0].text, "14:30");
    }

    #[test]
    fn iso_date_produces_single_token() {
        let tokens = lex("2025-06-15");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::IsoDate);
        assert_eq!(tokens[0].text, "2025-06-15");
    }

    #[test]
    fn rfc3339_produces_single_token() {
        let tokens = lex("2025-06-15T14:30:00Z");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Rfc3339);
    }

    #[test]
    fn time12_produces_single_token() {
        let tokens = lex("3pm");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Time12);
        assert_eq!(tokens[0].text, "3pm");
    }

    #[test]
    fn time12_with_minutes_produces_single_token() {
        let tokens = lex("10:30am");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Time12);
    }

    #[test]
    fn ordinal_day_produces_single_token() {
        for s in &["1st", "2nd", "3rd", "15th", "21st", "31st"] {
            let tokens = lex(s);
            assert_eq!(tokens.len(), 1, "{s}");
            assert_eq!(tokens[0].token, Token::OrdinalDay, "{s}");
        }
    }

    #[test]
    fn adjacent_number_and_unit_have_touching_spans() {
        // "1h" → Number("1") + Word("h"), span boundary must be shared.
        // "h" alone is not a recognised suffix token so it stays as Word.
        let tokens = lex("1h");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].token, Token::Number);
        assert_eq!(tokens[1].token, Token::Word);
        assert_eq!(tokens[0].span.end, tokens[1].span.start);
    }

    #[test]
    fn nine_am_is_single_time12_token() {
        // "9am" now lexes as a single Time12 token, not Number + Word.
        let tokens = lex("9am");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].token, Token::Time12);
        assert_eq!(tokens[0].text, "9am");
    }

    #[test]
    fn mixed_sentence() {
        let tokens = lex("Buy milk @tomorrow");
        // "Buy", "milk", "@", "tomorrow"
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[2].token, Token::At);
    }

    #[test]
    fn empty_input() {
        assert!(lex("").is_empty());
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[cfg(test)]
mod helpers {
    use crate::ast::WhenSpec;
    use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike, Utc};

    pub fn utc(year: i32, month: u32, day: u32, hour: u32, min: u32) -> DateTime<Utc> {
        let naive = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(hour, min, 0)
            .unwrap();
        Utc.from_utc_datetime(&naive)
    }

    /// Assert two datetimes match to the minute.
    pub fn assert_dt_eq_min(a: DateTime<Utc>, b: DateTime<Utc>) {
        assert_eq!(
            (a.year(), a.month(), a.day(), a.hour(), a.minute()),
            (b.year(), b.month(), b.day(), b.hour(), b.minute()),
            "datetimes differ: {a} vs {b}"
        );
    }

    /// Unwrap a `WhenSpec::DateTime` from an `Option<WhenSpec>`, panicking
    /// clearly if it is absent or is a `NaiveDate`.
    pub fn when_dt(when: Option<WhenSpec>) -> DateTime<Utc> {
        when.expect("expected Some(WhenSpec), got None")
            .unwrap_datetime()
    }

    /// Unwrap a `WhenSpec::NaiveDate` from an `Option<WhenSpec>`, panicking
    /// clearly if it is absent or is a `DateTime`.
    pub fn when_nd(when: Option<WhenSpec>) -> NaiveDate {
        when.expect("expected Some(WhenSpec), got None")
            .unwrap_naive_date()
    }
}

// ---------------------------------------------------------------------------
// Title extraction
// ---------------------------------------------------------------------------

#[cfg(test)]
mod title {
    use crate::parse::parse_action_input;

    #[test]
    fn plain_words() {
        let d = parse_action_input("Buy milk").unwrap();
        assert_eq!(d.title, "Buy milk");
    }

    #[test]
    fn title_before_at_clause() {
        let d = parse_action_input("Buy milk @tomorrow").unwrap();
        assert_eq!(d.title, "Buy milk");
    }

    #[test]
    fn title_after_at_clause() {
        let d = parse_action_input("@tomorrow Buy milk").unwrap();
        assert_eq!(d.title, "Buy milk");
    }

    #[test]
    fn title_before_tilde_clause() {
        let d = parse_action_input("Buy milk ~30m").unwrap();
        assert_eq!(d.title, "Buy milk");
    }

    #[test]
    fn title_after_tilde_clause() {
        let d = parse_action_input("~30m Buy milk").unwrap();
        assert_eq!(d.title, "Buy milk");
    }

    #[test]
    fn title_with_numbers() {
        let d = parse_action_input("Drink 8 glasses of water").unwrap();
        assert_eq!(d.title, "Drink 8 glasses of water");
    }

    #[test]
    fn title_strips_punctuation() {
        let d = parse_action_input("Buy milk, eggs.").unwrap();
        assert_eq!(d.title, "Buy milk eggs");
    }

    #[test]
    fn title_from_quoted_token() {
        let d = parse_action_input(r#""Buy milk""#).unwrap();
        assert_eq!(d.title, "Buy milk");
    }

    #[test]
    fn empty_input_errors() {
        assert!(parse_action_input("").is_err());
    }

    #[test]
    fn only_at_with_no_title_errors() {
        assert!(parse_action_input("@tomorrow").is_err());
    }

    #[test]
    fn only_punct_errors() {
        assert!(parse_action_input(".,").is_err());
    }

    #[test]
    fn only_whitespace_errors() {
        assert!(parse_action_input("   ").is_err());
    }
}

// ---------------------------------------------------------------------------
// @ datetime clause
// ---------------------------------------------------------------------------

#[cfg(test)]
mod at_clause {
    use super::helpers::{assert_dt_eq_min, utc, when_dt};
    use crate::parse::parse_action_input;
    use chrono::{Datelike, Local, Timelike, Utc};

    #[test]
    fn iso_date() {
        // @ sigil always produces WhenSpec::DateTime
        let d = parse_action_input("Meeting @2025-06-15").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.year(), when.month(), when.day()), (2025, 6, 15));
        assert_eq!((when.hour(), when.minute()), (9, 0));
    }

    #[test]
    fn iso_datetime() {
        let d = parse_action_input("Meeting @2025-06-15 14:30").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.year(), when.month(), when.day()), (2025, 6, 15));
        assert_eq!((when.hour(), when.minute()), (14, 30));
    }

    #[test]
    fn keyword_today_defaults_to_09h() {
        let d = parse_action_input("Standup @today").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!(when.date_naive(), Local::now().date_naive());
        assert_eq!((when.hour(), when.minute()), (9, 0));
    }

    #[test]
    fn keyword_tomorrow_defaults_to_09h() {
        let d = parse_action_input("Call @tomorrow").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        let tomorrow = (Local::now() + chrono::Duration::days(1)).date_naive();
        assert_eq!(when.date_naive(), tomorrow);
        assert_eq!((when.hour(), when.minute()), (9, 0));
    }

    #[test]
    fn keyword_now() {
        let before = Utc::now();
        let d = parse_action_input("Quick note @now").unwrap();
        let after = Utc::now();
        let when = when_dt(d.when);
        assert!(when >= before && when <= after);
    }

    #[test]
    fn today_with_24h_clock() {
        let d = parse_action_input("Standup @today 14:30").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!(when.date_naive(), Local::now().date_naive());
        assert_eq!((when.hour(), when.minute()), (14, 30));
    }

    #[test]
    fn today_with_am_time_adjacent() {
        let d = parse_action_input("Standup @today 9am").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.hour(), when.minute()), (9, 0));
    }

    #[test]
    fn today_with_pm_time_adjacent() {
        let d = parse_action_input("Standup @today 3pm").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.hour(), when.minute()), (15, 0));
    }

    #[test]
    fn tomorrow_with_am_time() {
        let d = parse_action_input("Call @tomorrow 10am").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        let tomorrow = (Local::now() + chrono::Duration::days(1)).date_naive();
        assert_eq!(when.date_naive(), tomorrow);
        assert_eq!((when.hour(), when.minute()), (10, 0));
    }

    #[test]
    fn tomorrow_with_pm_time() {
        let d = parse_action_input("Call @tomorrow 6pm").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.hour(), when.minute()), (18, 0));
    }

    #[test]
    fn noon_pm() {
        let d = parse_action_input("Lunch @today 12pm").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.hour(), when.minute()), (12, 0));
    }

    #[test]
    fn midnight_am() {
        let d = parse_action_input("Late thing @today 12am").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.hour(), when.minute()), (0, 0));
    }

    #[test]
    fn sigil_at_start_does_not_consume_title() {
        let d = parse_action_input("@tomorrow Buy milk").unwrap();
        assert_eq!(d.title, "Buy milk");
        assert!(d.when.is_some());
    }

    #[test]
    fn sigil_at_end_does_not_consume_title() {
        let d = parse_action_input("Buy milk @tomorrow").unwrap();
        assert_eq!(d.title, "Buy milk");
        assert!(d.when.is_some());
    }

    #[test]
    fn rfc3339_datetime() {
        let d = parse_action_input("Deploy @2025-01-15T08:00:00Z").unwrap();
        let expected = utc(2025, 1, 15, 8, 0);
        assert_dt_eq_min(when_dt(d.when), expected);
    }

    #[test]
    fn no_when_when_absent() {
        assert!(parse_action_input("Buy milk").unwrap().when.is_none());
    }
}

// ---------------------------------------------------------------------------
// ~ duration clause
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tilde_clause {
    use crate::parse::parse_action_input;
    use chrono::Duration;

    fn mins(n: i64) -> Duration {
        Duration::minutes(n)
    }
    fn hours(n: i64) -> Duration {
        Duration::hours(n)
    }

    #[test]
    fn suffix_m() {
        assert_eq!(
            parse_action_input("Task ~30m").unwrap().duration,
            Some(mins(30))
        );
    }

    #[test]
    fn suffix_min() {
        assert_eq!(
            parse_action_input("Task ~30min").unwrap().duration,
            Some(mins(30))
        );
    }

    #[test]
    fn suffix_mins() {
        assert_eq!(
            parse_action_input("Task ~30mins").unwrap().duration,
            Some(mins(30))
        );
    }

    #[test]
    fn suffix_minute() {
        assert_eq!(
            parse_action_input("Task ~30minute").unwrap().duration,
            Some(mins(30))
        );
    }

    #[test]
    fn suffix_minutes() {
        assert_eq!(
            parse_action_input("Task ~30minutes").unwrap().duration,
            Some(mins(30))
        );
    }

    #[test]
    fn suffix_h() {
        assert_eq!(
            parse_action_input("Task ~1h").unwrap().duration,
            Some(hours(1))
        );
    }

    #[test]
    fn suffix_hr() {
        assert_eq!(
            parse_action_input("Task ~2hr").unwrap().duration,
            Some(hours(2))
        );
    }

    #[test]
    fn suffix_hrs() {
        assert_eq!(
            parse_action_input("Task ~2hrs").unwrap().duration,
            Some(hours(2))
        );
    }

    #[test]
    fn suffix_hour() {
        assert_eq!(
            parse_action_input("Task ~1hour").unwrap().duration,
            Some(hours(1))
        );
    }

    #[test]
    fn suffix_hours() {
        assert_eq!(
            parse_action_input("Task ~3hours").unwrap().duration,
            Some(hours(3))
        );
    }

    #[test]
    fn sigil_before_title_does_not_consume_words() {
        let d = parse_action_input("~1h Buy milk").unwrap();
        assert_eq!(d.title, "Buy milk");
        assert_eq!(d.duration, Some(hours(1)));
    }

    #[test]
    fn sigil_after_title() {
        let d = parse_action_input("Buy milk ~1h").unwrap();
        assert_eq!(d.title, "Buy milk");
        assert_eq!(d.duration, Some(hours(1)));
    }

    #[test]
    fn large_duration() {
        assert_eq!(
            parse_action_input("Marathon ~240m").unwrap().duration,
            Some(mins(240))
        );
    }

    #[test]
    fn no_duration_when_absent() {
        assert!(
            parse_action_input("Simple task")
                .unwrap()
                .duration
                .is_none()
        );
    }
}

// ---------------------------------------------------------------------------
// % recurrence — grammar tested via parse_recurrence_str
// ---------------------------------------------------------------------------

#[cfg(test)]
mod recurrence {
    use crate::ast::RecurrenceSpec;
    use crate::parse::parse_recurrence_str;
    use chrono::Weekday::*;

    fn days(n: i64) -> RecurrenceSpec {
        RecurrenceSpec::EveryDays(n)
    }
    fn weeks(n: i64) -> RecurrenceSpec {
        RecurrenceSpec::EveryWeeks(n)
    }

    // -- Named shortcuts --

    #[test]
    fn daily() {
        assert_eq!(parse_recurrence_str("daily").unwrap(), days(1));
    }

    #[test]
    fn weekly() {
        assert_eq!(parse_recurrence_str("weekly").unwrap(), weeks(1));
    }

    #[test]
    fn weekdays_shorthand() {
        let r = parse_recurrence_str("weekdays").unwrap();
        let set = match r {
            RecurrenceSpec::OnWeekdays(s) => s,
            other => panic!("expected OnWeekdays, got {other:?}"),
        };
        assert!(set.contains(Mon));
        assert!(set.contains(Fri));
        assert!(!set.contains(Sat));
        assert!(!set.contains(Sun));
        assert_eq!(set.len(), 5);
    }

    #[test]
    fn weekends_shorthand() {
        let r = parse_recurrence_str("weekends").unwrap();
        let set = match r {
            RecurrenceSpec::OnWeekdays(s) => s,
            other => panic!("expected OnWeekdays, got {other:?}"),
        };
        assert!(set.contains(Sat));
        assert!(set.contains(Sun));
        assert!(!set.contains(Mon));
        assert_eq!(set.len(), 2);
    }

    // -- "every …" forms --

    #[test]
    fn every_day() {
        assert_eq!(parse_recurrence_str("every day").unwrap(), days(1));
    }

    #[test]
    fn every_week() {
        assert_eq!(parse_recurrence_str("every week").unwrap(), weeks(1));
    }

    #[test]
    fn every_weekday() {
        let r = parse_recurrence_str("every weekday").unwrap();
        assert!(matches!(r, RecurrenceSpec::OnWeekdays(_)));
    }

    #[test]
    fn every_weekdays() {
        let r = parse_recurrence_str("every weekdays").unwrap();
        assert!(matches!(r, RecurrenceSpec::OnWeekdays(_)));
    }

    #[test]
    fn every_weekend() {
        let r = parse_recurrence_str("every weekend").unwrap();
        assert!(matches!(r, RecurrenceSpec::OnWeekdays(_)));
    }

    #[test]
    fn every_weekends() {
        let r = parse_recurrence_str("every weekends").unwrap();
        assert!(matches!(r, RecurrenceSpec::OnWeekdays(_)));
    }

    #[test]
    fn every_n_days() {
        assert_eq!(parse_recurrence_str("every 3 days").unwrap(), days(3));
    }

    #[test]
    fn every_7_days() {
        assert_eq!(parse_recurrence_str("every 7 days").unwrap(), days(7));
    }

    #[test]
    fn every_n_weeks() {
        assert_eq!(parse_recurrence_str("every 2 weeks").unwrap(), weeks(2));
    }

    // -- Named single days --

    #[test]
    fn every_monday() {
        let r = parse_recurrence_str("every monday").unwrap();
        let set = match r {
            RecurrenceSpec::OnWeekdays(s) => s,
            other => panic!("expected OnWeekdays, got {other:?}"),
        };
        assert_eq!(set.len(), 1);
        assert!(set.contains(Mon));
    }

    #[test]
    fn every_monday_plural() {
        let r = parse_recurrence_str("every mondays").unwrap();
        let set = match r {
            RecurrenceSpec::OnWeekdays(s) => s,
            other => panic!("{other:?}"),
        };
        assert!(set.contains(Mon));
    }

    #[test]
    fn every_friday() {
        let r = parse_recurrence_str("every friday").unwrap();
        let set = match r {
            RecurrenceSpec::OnWeekdays(s) => s,
            other => panic!("{other:?}"),
        };
        assert!(set.contains(Fri));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn every_sunday() {
        let r = parse_recurrence_str("every sunday").unwrap();
        let set = match r {
            RecurrenceSpec::OnWeekdays(s) => s,
            other => panic!("{other:?}"),
        };
        assert!(set.contains(Sun));
    }

    // -- Comma-separated day lists --

    #[test]
    fn day_list_two_days() {
        let r = parse_recurrence_str("mon,wed").unwrap();
        let set = match r {
            RecurrenceSpec::OnWeekdays(s) => s,
            other => panic!("{other:?}"),
        };
        assert!(set.contains(Mon));
        assert!(set.contains(Wed));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn day_list_with_spaces() {
        let r = parse_recurrence_str("mon, wed, fri").unwrap();
        let set = match r {
            RecurrenceSpec::OnWeekdays(s) => s,
            other => panic!("{other:?}"),
        };
        assert!(set.contains(Mon));
        assert!(set.contains(Wed));
        assert!(set.contains(Fri));
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn day_list_all_short_names() {
        // All seven short names are valid
        for (input, expected) in [
            ("mon,tue", vec![Mon, Tue]),
            ("wed,thu", vec![Wed, Thu]),
            ("fri,sat", vec![Fri, Sat]),
            ("sun,mon", vec![Sun, Mon]),
        ] {
            let r = parse_recurrence_str(input).unwrap();
            let set = match r {
                RecurrenceSpec::OnWeekdays(s) => s,
                other => panic!("{other:?}"),
            };
            for day in &expected {
                assert!(set.contains(*day), "{input}: missing {day:?}");
            }
            assert_eq!(set.len(), expected.len(), "{input}");
        }
    }

    #[test]
    fn invalid_recurrence_is_error() {
        assert!(parse_recurrence_str("sometimes").is_err());
        assert!(parse_recurrence_str("often").is_err());
        assert!(parse_recurrence_str("every other day").is_err());
        assert!(parse_recurrence_str("").is_err());
    }
}

// ---------------------------------------------------------------------------
// WeekdaySet helpers
// ---------------------------------------------------------------------------

#[cfg(test)]
mod weekday_set {
    use crate::ast::WeekdaySet;
    use chrono::Weekday::*;

    #[test]
    fn weekdays_contains_mon_to_fri() {
        let s = WeekdaySet::weekdays();
        for d in [Mon, Tue, Wed, Thu, Fri] {
            assert!(s.contains(d), "{d:?} should be in weekdays");
        }
        for d in [Sat, Sun] {
            assert!(!s.contains(d), "{d:?} should not be in weekdays");
        }
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn weekends_contains_sat_sun() {
        let s = WeekdaySet::weekends();
        assert!(s.contains(Sat));
        assert!(s.contains(Sun));
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn every_day_contains_all_seven() {
        let s = WeekdaySet::every_day();
        for d in [Mon, Tue, Wed, Thu, Fri, Sat, Sun] {
            assert!(s.contains(d));
        }
        assert_eq!(s.len(), 7);
    }

    #[test]
    fn is_empty_on_empty_set() {
        let s = WeekdaySet::new([]);
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn deduplication() {
        // BTreeSet deduplicates; inserting the same day twice should count once.
        let s = WeekdaySet::new([Mon, Mon, Tue]);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn equality() {
        assert_eq!(WeekdaySet::weekdays(), WeekdaySet::weekdays());
        assert_ne!(WeekdaySet::weekdays(), WeekdaySet::weekends());
    }
}

// ---------------------------------------------------------------------------
// parse_action_input vs parse_event_input — kind field
// ---------------------------------------------------------------------------

#[cfg(test)]
mod parse_kind {
    use crate::ast::EntityKind;
    use crate::parse::{parse_action_input, parse_event_input};

    #[test]
    fn action_input_sets_kind_action() {
        let d = parse_action_input("Buy milk").unwrap();
        assert_eq!(d.kind, EntityKind::Action);
    }

    #[test]
    fn event_input_sets_kind_event() {
        let d = parse_event_input("Team lunch @tomorrow").unwrap();
        assert_eq!(d.kind, EntityKind::Event);
    }

    #[test]
    fn action_input_with_when_still_kind_action() {
        // Kind is set by the entry-point, not inferred from the presence of @
        let d = parse_action_input("Follow up @tomorrow").unwrap();
        assert_eq!(d.kind, EntityKind::Action);
    }

    #[test]
    fn event_input_without_when_still_kind_event() {
        // parse_event_input doesn't require @ — kind is still Event
        let d = parse_event_input("Team lunch").unwrap();
        assert_eq!(d.kind, EntityKind::Event);
    }
}

// ---------------------------------------------------------------------------
// Content string assembly
// ---------------------------------------------------------------------------

#[cfg(test)]
mod content {
    use crate::parse::parse_action_input;

    #[test]
    fn no_metadata_means_no_content() {
        assert!(parse_action_input("Simple task").unwrap().content.is_none());
    }

    #[test]
    fn at_and_tilde_do_not_appear_in_content() {
        // when and duration live in their own fields, not in content
        let d = parse_action_input("Task @tomorrow ~1h").unwrap();
        let content = d.content.as_deref().unwrap_or("");
        assert!(!content.contains("tomorrow"));
        assert!(!content.contains("1h"));
    }
}

// ---------------------------------------------------------------------------
// Combinations
// ---------------------------------------------------------------------------

#[cfg(test)]
mod combinations {
    use super::helpers::when_dt;
    use crate::parse::parse_action_input;
    use chrono::{Datelike, Duration, Local, Timelike};

    #[test]
    fn at_and_tilde_title_first() {
        // @ sigil → DateTime
        let d = parse_action_input("Deep work @2025-06-15 ~2h").unwrap();
        assert_eq!(d.title, "Deep work");
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.year(), when.month(), when.day()), (2025, 6, 15));
        assert_eq!(d.duration, Some(Duration::hours(2)));
    }

    #[test]
    fn at_and_tilde_sigils_first() {
        let d = parse_action_input("@tomorrow ~1h Buy milk").unwrap();
        assert_eq!(d.title, "Buy milk");
        assert!(d.when.is_some());
        assert_eq!(d.duration, Some(Duration::hours(1)));
    }

    #[test]
    fn at_clause_middle_of_title() {
        let d = parse_action_input("Buy @tomorrow milk").unwrap();
        assert_eq!(d.title, "Buy milk");
        assert!(d.when.is_some());
    }

    #[test]
    fn tilde_clause_middle_of_title() {
        // "today" NL → WhenSpec::NaiveDate for actions
        let d = parse_action_input("Run ~30m today").unwrap();
        assert_eq!(d.title, "Run");
        assert_eq!(d.duration, Some(Duration::minutes(30)));
        assert!(d.when.is_some()); // "today" → NaiveDate(today)
    }

    #[test]
    fn iso_date_plus_duration() {
        // @ sigil → DateTime
        let d = parse_action_input("Workshop @2025-09-01 ~3hours").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.year(), when.month(), when.day()), (2025, 9, 1));
        assert_eq!(d.duration, Some(Duration::hours(3)));
    }

    #[test]
    fn full_iso_datetime_plus_duration() {
        let d = parse_action_input("Workshop @2025-09-01 14:30 ~2h").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.hour(), when.minute()), (14, 30));
        assert_eq!(d.duration, Some(Duration::hours(2)));
    }

    #[test]
    fn tomorrow_plus_time_plus_duration() {
        // @ sigil with time suffix → DateTime
        let d = parse_action_input("Meeting @tomorrow 9am ~1h").unwrap();
        assert_eq!(d.title, "Meeting");
        let when = when_dt(d.when).with_timezone(&Local);
        let tomorrow = (Local::now() + chrono::Duration::days(1)).date_naive();
        assert_eq!(when.date_naive(), tomorrow);
        assert_eq!((when.hour(), when.minute()), (9, 0));
        assert_eq!(d.duration, Some(Duration::hours(1)));
    }

    #[test]
    fn raw_field_preserved_exactly() {
        let input = "Buy milk @tomorrow ~15m";
        let d = parse_action_input(input).unwrap();
        assert_eq!(d.raw, input);
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[cfg(test)]
mod errors {
    use crate::parse::{parse_action_input, parse_event_input};

    #[test]
    fn empty_string_is_error() {
        assert!(parse_action_input("").is_err());
        assert!(parse_event_input("").is_err());
    }

    #[test]
    fn only_whitespace_is_error() {
        assert!(parse_action_input("   ").is_err());
    }

    #[test]
    fn bare_at_with_no_title_is_error() {
        assert!(parse_action_input("@tomorrow").is_err());
    }

    #[test]
    fn bad_datetime_sigil_falls_through_gracefully() {
        // With the new sigil handler: "@" followed by an unrecognised word
        // fails to parse as a time; the "@" and following words fall through
        // to the title rather than causing a hard error.
        // "not" is a plain word, so the whole thing becomes the title.
        let result = parse_action_input("Task @not-a-date");
        // We only require it either errors or produces a sensible title —
        // the important thing is that unrecognised sigil bodies don't crash.
        // Currently the parser is lenient: the sigil is dropped, the words
        // become title.
        if let Ok(d) = result {
            assert!(d.title.contains("Task"));
        }
        // An error is also acceptable — just not a panic.
    }

    #[test]
    fn bad_duration_sigil_falls_through_gracefully() {
        // "~fiveish" — "fiveish" is not a valid duration; the ~ is dropped
        // and "fiveish" becomes part of the title.
        let d = parse_action_input("Task ~fiveish").unwrap();
        assert_eq!(d.title, "Task fiveish");
        assert!(d.duration.is_none());
    }
}

// ---------------------------------------------------------------------------
// build_entity
// ---------------------------------------------------------------------------

#[cfg(test)]
mod build {
    use crate::build::{BuildTarget, BuiltEntity, build_entity};
    use crate::parse::{parse_action_input, parse_event_input};
    use chrono::{Datelike, Duration, Local, Timelike};
    use simple_core::{ActionState, RecurrenceRule};

    // -- Action --

    #[test]
    fn build_action_title_only() {
        let draft = parse_action_input("Buy milk").unwrap();
        let entity = build_entity(&draft, BuildTarget::Action).unwrap();
        let a = match entity {
            BuiltEntity::Action(a) => a,
            _ => panic!("expected Action"),
        };
        assert_eq!(a.title, "Buy milk");
        assert!(matches!(a.state, ActionState::Backlogged(None)));
        assert!(a.duration.is_none());
        assert!(a.recurrence.is_none());
        assert!(a.content.is_none());
    }

    #[test]
    fn build_action_with_target_and_duration() {
        let draft = parse_action_input("Deep work @2025-06-15 ~2h").unwrap();
        let entity = build_entity(&draft, BuildTarget::Action).unwrap();
        let a = match entity {
            BuiltEntity::Action(a) => a,
            _ => panic!("expected Action"),
        };
        assert_eq!(a.title, "Deep work");
        assert!(matches!(a.state, ActionState::Queued(_)));
        assert_eq!(a.duration, Some(Duration::hours(2)));
        let ActionState::Queued(target) = a.state else {
            unreachable!()
        };
        assert!(target.is_static);
    }

    #[test]
    fn build_action_with_every_n_days_recurrence() {
        let draft = parse_action_input("Morning run @today ~30m").unwrap();
        // Manually attach recurrence for this test since % is stubbed
        let mut draft = draft;
        draft.recurrence = Some(crate::ast::RecurrenceSpec::EveryDays(1));
        let entity = build_entity(&draft, BuildTarget::Action).unwrap();
        let a = match entity {
            BuiltEntity::Action(a) => a,
            _ => panic!("expected Action"),
        };
        assert_eq!(a.recurrence, Some(RecurrenceRule::days(1)));
    }

    #[test]
    fn build_action_with_every_n_weeks_recurrence() {
        let mut draft = parse_action_input("Review goals").unwrap();
        draft.recurrence = Some(crate::ast::RecurrenceSpec::EveryWeeks(1));
        let entity = build_entity(&draft, BuildTarget::Action).unwrap();
        let a = match entity {
            BuiltEntity::Action(a) => a,
            _ => panic!("expected Action"),
        };
        assert_eq!(a.recurrence, Some(RecurrenceRule::weeks(1)));
    }

    #[test]
    fn build_action_weekday_recurrence_maps_to_daily_tick() {
        // OnWeekdays recurrence → Duration::days(1) in the built entity;
        // the scheduler is responsible for skipping non-matching days.
        let mut draft = parse_action_input("Standup").unwrap();
        draft.recurrence = Some(crate::ast::RecurrenceSpec::weekdays());
        let entity = build_entity(&draft, BuildTarget::Action).unwrap();
        let a = match entity {
            BuiltEntity::Action(a) => a,
            _ => panic!("expected Action"),
        };
        assert_eq!(a.recurrence, Some(RecurrenceRule::days(1)));
    }

    // -- Event --

    #[test]
    fn build_event_requires_when() {
        let draft = parse_event_input("Team lunch").unwrap();
        assert!(build_entity(&draft, BuildTarget::Event).is_err());
    }

    #[test]
    fn build_event_with_datetime() {
        let draft = parse_event_input("Team lunch @2025-07-04 12:00").unwrap();
        let entity = build_entity(&draft, BuildTarget::Event).unwrap();
        let e = match entity {
            BuiltEntity::Event(e) => e,
            _ => panic!("expected Event"),
        };
        assert_eq!(e.title, "Team lunch");
        let local = e.time.with_timezone(&Local);
        assert_eq!((local.hour(), local.minute()), (12, 0));
        assert!(e.duration.is_none());
    }

    #[test]
    fn build_event_with_duration() {
        let draft = parse_event_input("Workshop @2025-09-01 ~3h").unwrap();
        let entity = build_entity(&draft, BuildTarget::Event).unwrap();
        let e = match entity {
            BuiltEntity::Event(e) => e,
            _ => panic!("expected Event"),
        };
        assert_eq!(e.duration, Some(Duration::hours(3)));
    }

    #[test]
    fn build_event_with_weekday_recurrence() {
        let mut draft = parse_event_input("Standup @today 9am").unwrap();
        draft.recurrence = Some(crate::ast::RecurrenceSpec::weekdays());
        let entity = build_entity(&draft, BuildTarget::Event).unwrap();
        let e = match entity {
            BuiltEntity::Event(e) => e,
            _ => panic!("expected Event"),
        };
        // OnWeekdays → 1-day tick
        assert_eq!(e.recurrence, Some(RecurrenceRule::days(1)));
    }

    #[test]
    fn build_event_time_is_correct() {
        // Use 24h clock — "YYYY-MM-DD HH:MM" is a supported datetime format.
        // am/pm suffixes are only supported after "today"/"tomorrow", not ISO dates.
        let draft = parse_event_input("Morning call @2025-03-10 08:00").unwrap();
        let entity = build_entity(&draft, BuildTarget::Event).unwrap();
        let e = match entity {
            BuiltEntity::Event(e) => e,
            _ => panic!("expected Event"),
        };
        let local = e.time.with_timezone(&Local);
        assert_eq!(local.hour(), 8);
        assert_eq!((local.year(), local.month(), local.day()), (2025, 3, 10));
    }

    // -- IDs --

    #[test]
    fn each_build_call_produces_unique_id() {
        let draft = parse_action_input("Buy milk").unwrap();
        let e1 = build_entity(&draft, BuildTarget::Action).unwrap();
        let e2 = build_entity(&draft, BuildTarget::Action).unwrap();
        let id1 = match &e1 {
            BuiltEntity::Action(a) => a.id,
            _ => panic!(),
        };
        let id2 = match &e2 {
            BuiltEntity::Action(a) => a.id,
            _ => panic!(),
        };
        assert_ne!(id1, id2);
    }
}

// ---------------------------------------------------------------------------
// Natural-language time expressions (Phase 1)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod nl_time {
    use super::helpers::{when_dt, when_nd};
    use crate::parse::parse_action_input;
    use chrono::{Datelike, Local, Timelike, Utc};

    /// Helper for tests that only care about hour+minute of a DateTime result.
    fn hour_min(input: &str) -> (u32, u32) {
        let d = parse_action_input(input).unwrap();
        let w = when_dt(d.when).with_timezone(&Local);
        (w.hour(), w.minute())
    }

    // --- Bare time tokens — always WhenSpec::DateTime ---

    #[test]
    fn bare_3pm_defaults_to_today() {
        let d = parse_action_input("Call someone 3pm").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!(when.date_naive(), Local::now().date_naive());
        assert_eq!((when.hour(), when.minute()), (15, 0));
        assert_eq!(d.title, "Call someone");
    }

    #[test]
    fn bare_1030am() {
        assert_eq!(hour_min("Meeting 10:30am"), (10, 30));
    }

    #[test]
    fn bare_24h_clock() {
        assert_eq!(hour_min("Standup 14:30"), (14, 30));
    }

    #[test]
    fn bare_noon_12pm() {
        assert_eq!(hour_min("Lunch 12pm"), (12, 0));
    }

    #[test]
    fn bare_midnight_12am() {
        assert_eq!(hour_min("Night shift 12am"), (0, 0));
    }

    // --- "at <time>" preposition form — always WhenSpec::DateTime ---

    #[test]
    fn at_3pm_preposition() {
        assert_eq!(hour_min("Call at 3pm"), (15, 0));
    }

    #[test]
    fn at_1430_preposition() {
        assert_eq!(hour_min("Meeting at 14:30"), (14, 30));
    }

    // --- today / tomorrow keywords (no time) → WhenSpec::NaiveDate for actions ---

    #[test]
    fn today_keyword_nl() {
        let d = parse_action_input("Do that thing today").unwrap();
        // No explicit time → NaiveDate
        assert_eq!(when_nd(d.when), Local::now().date_naive());
        assert_eq!(d.title, "Do that thing");
    }

    #[test]
    fn tomorrow_keyword_nl() {
        let d = parse_action_input("Go to the store tomorrow").unwrap();
        let expected = (Local::now() + chrono::Duration::days(1)).date_naive();
        // No explicit time → NaiveDate
        assert_eq!(when_nd(d.when), expected);
        assert_eq!(d.title, "Go to the store");
    }

    // --- today/tomorrow WITH explicit time → WhenSpec::DateTime ---

    #[test]
    fn tomorrow_at_10am() {
        let d = parse_action_input("Call tomorrow at 10am").unwrap();
        let expected_date = (Local::now() + chrono::Duration::days(1)).date_naive();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!(when.date_naive(), expected_date);
        assert_eq!((when.hour(), when.minute()), (10, 0));
        assert_eq!(d.title, "Call");
    }

    #[test]
    fn today_at_3pm() {
        let d = parse_action_input("Call today at 3pm").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!(when.date_naive(), Local::now().date_naive());
        assert_eq!(when.hour(), 15);
        assert_eq!(d.title, "Call");
    }

    // --- tonight — implied time (20:00) → always WhenSpec::DateTime ---

    #[test]
    fn tonight_keyword() {
        let d = parse_action_input("Watch a film tonight").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!(when.date_naive(), Local::now().date_naive());
        assert_eq!(when.hour(), 20);
        assert_eq!(d.title, "Watch a film");
    }

    // --- later / soon — implied times → always WhenSpec::DateTime ---

    #[test]
    fn later_keyword() {
        let d = parse_action_input("Ask that person something later").unwrap();
        assert_eq!(d.title, "Ask that person something");
        assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 14);
    }

    #[test]
    fn soon_keyword() {
        let d = parse_action_input("Review PR soon").unwrap();
        let tomorrow = (Local::now() + chrono::Duration::days(1)).date_naive();
        assert_eq!(when_dt(d.when).with_timezone(&Local).date_naive(), tomorrow);
    }

    // --- this morning/afternoon/evening — implied times → WhenSpec::DateTime ---

    #[test]
    fn this_morning() {
        let d = parse_action_input("Do that thing this morning").unwrap();
        assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 9);
        assert_eq!(d.title, "Do that thing");
    }

    #[test]
    fn this_afternoon() {
        let d = parse_action_input("Do that thing this afternoon").unwrap();
        assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 14);
        assert_eq!(d.title, "Do that thing");
    }

    #[test]
    fn this_evening() {
        let d = parse_action_input("Do that thing this evening").unwrap();
        assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 20);
        assert_eq!(d.title, "Do that thing");
    }

    // --- bare weekday names (no time) → WhenSpec::NaiveDate for actions ---

    #[test]
    fn bare_monday_is_next_monday() {
        let d = parse_action_input("Do that thing monday").unwrap();
        let date = when_nd(d.when);
        assert_eq!(date.weekday(), chrono::Weekday::Mon);
        assert!(date > Local::now().date_naive());
    }

    #[test]
    fn bare_friday_is_next_friday() {
        let d = parse_action_input("Do that thing friday").unwrap();
        assert_eq!(when_nd(d.when).weekday(), chrono::Weekday::Fri);
    }

    #[test]
    fn next_monday_explicit() {
        let d = parse_action_input("Meeting next monday").unwrap();
        let date = when_nd(d.when);
        assert_eq!(date.weekday(), chrono::Weekday::Mon);
        assert!(date > Local::now().date_naive());
    }

    // --- weekday WITH explicit time → WhenSpec::DateTime ---

    #[test]
    fn next_monday_at_6pm() {
        let d = parse_action_input("Read that book next monday at 6pm").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!(when.weekday(), chrono::Weekday::Mon);
        assert_eq!((when.hour(), when.minute()), (18, 0));
    }

    #[test]
    fn friday_3pm_order_day_first() {
        let d = parse_action_input("Meeting friday 3pm").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!(when.weekday(), chrono::Weekday::Fri);
        assert_eq!(when.hour(), 15);
    }

    // --- "on <weekday>" (no time) → WhenSpec::NaiveDate for actions ---

    #[test]
    fn on_tuesday_preposition() {
        let d = parse_action_input("Standup on tuesday").unwrap();
        assert_eq!(when_nd(d.when).weekday(), chrono::Weekday::Tue);
    }

    // --- relative: in N hours/minutes → WhenSpec::DateTime ---

    #[test]
    fn in_2_hours() {
        let before = Utc::now();
        let d = parse_action_input("Check in in 2 hours").unwrap();
        let after = Utc::now();
        let when = when_dt(d.when);
        assert!(when >= before + chrono::Duration::hours(2));
        assert!(when <= after + chrono::Duration::hours(2) + chrono::Duration::seconds(5));
    }

    #[test]
    fn in_30_minutes() {
        let before = Utc::now();
        let d = parse_action_input("Quick check in 30 minutes").unwrap();
        let when = when_dt(d.when);
        assert!(when >= before + chrono::Duration::minutes(30));
    }

    // --- in N days → WhenSpec::NaiveDate for actions ---

    #[test]
    fn in_3_days() {
        let d = parse_action_input("Follow up in 3 days").unwrap();
        let expected = (Local::now() + chrono::Duration::days(3)).date_naive();
        assert_eq!(when_nd(d.when), expected);
    }

    // --- ISO date token (no time) → WhenSpec::NaiveDate for actions ---

    #[test]
    fn iso_date_nl() {
        let d = parse_action_input("Meeting 2025-06-15").unwrap();
        let date = when_nd(d.when);
        assert_eq!((date.year(), date.month(), date.day()), (2025, 6, 15));
    }

    // --- ISO date WITH time → WhenSpec::DateTime ---

    #[test]
    fn iso_date_with_time24() {
        let d = parse_action_input("Meeting 2025-06-15 14:30").unwrap();
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.year(), when.month(), when.day()), (2025, 6, 15));
        assert_eq!((when.hour(), when.minute()), (14, 30));
    }

    // --- Month + day (no time) → WhenSpec::NaiveDate for actions ---

    #[test]
    fn june_15_month_day() {
        let d = parse_action_input("Birthday june 15").unwrap();
        let date = when_nd(d.when);
        assert_eq!(date.month(), 6);
        assert_eq!(date.day(), 15);
        assert_eq!(d.title, "Birthday");
    }

    #[test]
    fn jan_1st_ordinal() {
        let d = parse_action_input("New year party january 1st").unwrap();
        let date = when_nd(d.when);
        assert_eq!(date.month(), 1);
        assert_eq!(date.day(), 1);
    }

    // --- time at end of sentence (natural order) ---

    #[test]
    fn time_at_end_of_sentence() {
        // "3pm tomorrow" — time+date combo → DateTime
        let d = parse_action_input("Go to the store 3pm tomorrow").unwrap();
        assert_eq!(d.title, "Go to the store");
        assert!(d.when.is_some());
    }

    #[test]
    fn time_before_title() {
        let d = parse_action_input("3pm Go to the store").unwrap();
        assert_eq!(d.title, "Go to the store");
        assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 15);
    }
}

// ---------------------------------------------------------------------------
// Natural-language recurrence (Phase 2)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod nl_recurrence {
    use crate::ast::RecurrenceSpec;
    use crate::parse::parse_action_input;
    use chrono::Weekday::*;

    fn rec(input: &str) -> RecurrenceSpec {
        parse_action_input(input).unwrap().recurrence.unwrap()
    }

    #[test]
    fn daily_keyword_nl() {
        assert_eq!(rec("Morning run daily"), RecurrenceSpec::EveryDays(1));
    }

    #[test]
    fn weekly_keyword_nl() {
        assert_eq!(rec("Review goals weekly"), RecurrenceSpec::EveryWeeks(1));
    }

    #[test]
    fn weekdays_keyword_nl() {
        let r = rec("Standup weekdays");
        assert!(matches!(r, RecurrenceSpec::OnWeekdays(_)));
        if let RecurrenceSpec::OnWeekdays(s) = r {
            assert!(s.contains(Mon));
            assert!(s.contains(Fri));
            assert!(!s.contains(Sat));
        }
    }

    #[test]
    fn weekends_keyword_nl() {
        let r = rec("Long run weekends");
        assert!(matches!(r, RecurrenceSpec::OnWeekdays(_)));
        if let RecurrenceSpec::OnWeekdays(s) = r {
            assert!(s.contains(Sat));
            assert!(s.contains(Sun));
            assert!(!s.contains(Mon));
        }
    }

    #[test]
    fn every_day_nl() {
        assert_eq!(rec("Walk every day"), RecurrenceSpec::EveryDays(1));
    }

    #[test]
    fn every_week_nl() {
        assert_eq!(rec("Review every week"), RecurrenceSpec::EveryWeeks(1));
    }

    #[test]
    fn every_3_days_nl() {
        assert_eq!(
            rec("Water plants every 3 days"),
            RecurrenceSpec::EveryDays(3)
        );
    }

    #[test]
    fn every_2_weeks_nl() {
        assert_eq!(rec("Dentist every 2 weeks"), RecurrenceSpec::EveryWeeks(2));
    }

    #[test]
    fn every_monday_nl() {
        let r = rec("Read that book every monday");
        if let RecurrenceSpec::OnWeekdays(s) = r {
            assert_eq!(s.len(), 1);
            assert!(s.contains(Mon));
        } else {
            panic!("expected OnWeekdays");
        }
    }

    #[test]
    fn every_thursday_nl() {
        let r = rec("Team sync every thursday");
        if let RecurrenceSpec::OnWeekdays(s) = r {
            assert!(s.contains(Thu));
        } else {
            panic!("expected OnWeekdays");
        }
    }

    #[test]
    fn every_weekday_nl() {
        let r = rec("Standup every weekday");
        if let RecurrenceSpec::OnWeekdays(s) = r {
            assert_eq!(s.len(), 5);
        } else {
            panic!("expected OnWeekdays");
        }
    }

    #[test]
    fn every_weekend_nl() {
        let r = rec("Long run every weekend");
        if let RecurrenceSpec::OnWeekdays(s) = r {
            assert_eq!(s.len(), 2);
        } else {
            panic!("expected OnWeekdays");
        }
    }

    #[test]
    fn recurrence_title_not_contaminated() {
        let d = parse_action_input("Morning run every day").unwrap();
        assert_eq!(d.title, "Morning run");
        assert!(d.recurrence.is_some());
    }

    #[test]
    fn recurrence_at_start() {
        let d = parse_action_input("every monday Read that book").unwrap();
        assert_eq!(d.title, "Read that book");
        assert!(d.recurrence.is_some());
    }
}

// ---------------------------------------------------------------------------
// Natural-language duration (Phase 4)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod nl_duration {
    use crate::parse::parse_action_input;
    use chrono::Duration;

    fn dur(input: &str) -> Duration {
        parse_action_input(input).unwrap().duration.unwrap()
    }

    // --- "for <duration>" intro word ---

    #[test]
    fn for_30_minutes() {
        assert_eq!(dur("Call for 30 minutes"), Duration::minutes(30));
    }

    #[test]
    fn for_1_hour() {
        assert_eq!(dur("Meeting for 1 hour"), Duration::hours(1));
    }

    #[test]
    fn for_2_hours() {
        assert_eq!(dur("Workshop for 2 hours"), Duration::hours(2));
    }

    #[test]
    fn for_45min() {
        assert_eq!(dur("Task for 45min"), Duration::minutes(45));
    }

    #[test]
    fn for_1h() {
        assert_eq!(dur("Task for 1h"), Duration::hours(1));
    }

    // --- "lasting" intro word ---

    #[test]
    fn lasting_2_hours() {
        assert_eq!(dur("Workshop lasting 2 hours"), Duration::hours(2));
    }

    // --- combined forms ---

    #[test]
    fn combined_1h30m() {
        assert_eq!(dur("Task for 1h30m"), Duration::minutes(90));
    }

    #[test]
    fn combined_1h_space_30min() {
        assert_eq!(dur("Task for 1h 30min"), Duration::minutes(90));
    }

    #[test]
    fn colon_form_1_30() {
        assert_eq!(dur("Task for 1:30"), Duration::minutes(90));
    }

    #[test]
    fn colon_form_0_45() {
        assert_eq!(dur("Task for 0:45"), Duration::minutes(45));
    }

    // --- title not contaminated ---

    #[test]
    fn for_clause_removed_from_title() {
        let d = parse_action_input("Deep work for 2 hours").unwrap();
        assert_eq!(d.title, "Deep work");
        assert_eq!(d.duration, Some(Duration::hours(2)));
    }

    #[test]
    fn for_clause_at_start() {
        let d = parse_action_input("for 1h Deep work").unwrap();
        assert_eq!(d.title, "Deep work");
        assert_eq!(d.duration, Some(Duration::hours(1)));
    }

    // --- tilde sigil still works ---

    #[test]
    fn tilde_sigil_still_works() {
        assert_eq!(dur("Task ~30m"), Duration::minutes(30));
        assert_eq!(dur("Task ~1h"), Duration::hours(1));
    }
}

// ---------------------------------------------------------------------------
// Natural-language location (Phase 3)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod nl_location {
    use crate::parse::{ParseContext, parse_action_input, parse_action_input_ctx};

    fn ctx_with_locs<'a>(locs: &'a [&'a str]) -> ParseContext<'a> {
        ParseContext {
            locations: locs,
            ..Default::default()
        }
    }

    #[test]
    fn at_known_location() {
        let ctx = ctx_with_locs(&["home", "work"]);
        let d = parse_action_input_ctx("Do that activity at home today", &ctx).unwrap();
        assert_eq!(d.title, "Do that activity");
        assert_eq!(d.location.as_deref(), Some("home"));
        assert!(d.when.is_some());
    }

    #[test]
    fn at_work_location() {
        let ctx = ctx_with_locs(&["home", "work"]);
        let d = parse_action_input_ctx("Team meeting at work every tuesday", &ctx).unwrap();
        assert_eq!(d.location.as_deref(), Some("work"));
    }

    #[test]
    fn in_known_location() {
        let ctx = ctx_with_locs(&["the office", "home"]);
        let d = parse_action_input_ctx("Meeting in the office today", &ctx).unwrap();
        assert_eq!(d.location.as_deref(), Some("the office"));
    }

    #[test]
    fn at_time_not_stolen_as_location() {
        // "at 3pm" must be parsed as a time, not a location
        let ctx = ctx_with_locs(&["3pm"]); // even if "3pm" is in the location list
        let d = parse_action_input_ctx("Call at 3pm", &ctx).unwrap();
        // The time clause should win over location for time tokens
        assert!(d.when.is_some());
    }

    #[test]
    fn no_context_no_location() {
        // Without a context, bare location words stay in the title
        let d = parse_action_input("Work out at home").unwrap();
        // "at" does not match a known location → stays in title
        assert_eq!(d.location, None);
    }

    #[test]
    fn amp_sigil_location() {
        let d = parse_action_input("Task &home").unwrap();
        assert_eq!(d.location.as_deref(), Some("home"));
        assert_eq!(d.title, "Task");
    }

    #[test]
    fn amp_sigil_quoted_location() {
        let d = parse_action_input(r#"Task &"coffee shop""#).unwrap();
        assert_eq!(d.location.as_deref(), Some("coffee shop"));
    }
}

// ---------------------------------------------------------------------------
// Natural-language people / attendees (Phase 5)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod nl_people {
    use crate::parse::{ParseContext, parse_action_input_ctx};

    fn ctx_with_people<'a>(people: &'a [&'a str]) -> ParseContext<'a> {
        ParseContext {
            people,
            ..Default::default()
        }
    }

    #[test]
    fn with_single_person() {
        let ctx = ctx_with_people(&["Isabel"]);
        let d = parse_action_input_ctx("Date with Isabel at 8pm", &ctx).unwrap();
        assert_eq!(d.title, "Date");
        assert_eq!(d.people, vec!["Isabel"]);
        assert!(d.when.is_some());
    }

    #[test]
    fn with_capitalised_name_no_context() {
        // Capitalised names are inferred as people even without context
        let d = parse_action_input_ctx("Lunch with Bob", &ParseContext::default()).unwrap();
        assert!(d.people.contains(&"Bob".to_string()));
    }

    #[test]
    fn with_and_list() {
        let ctx = ctx_with_people(&["Bob", "Alice"]);
        let d = parse_action_input_ctx("Meeting with Bob and Alice", &ctx).unwrap();
        assert!(d.people.contains(&"Bob".to_string()));
        assert!(d.people.contains(&"Alice".to_string()));
    }

    #[test]
    fn people_not_in_title() {
        let ctx = ctx_with_people(&["Isabel"]);
        let d = parse_action_input_ctx("Date with Isabel", &ctx).unwrap();
        assert_eq!(d.title, "Date");
    }
}

// ---------------------------------------------------------------------------
// Natural-language tags (Phase 6)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod nl_tags {
    use crate::parse::{ParseContext, parse_action_input_ctx};

    #[test]
    fn hash_sigil_tag() {
        let d = parse_action_input_ctx("Task #work", &ParseContext::default()).unwrap();
        assert!(d.tags.contains(&"work".to_string()));
        assert_eq!(d.title, "Task");
    }

    #[test]
    fn hash_sigil_quoted_tag() {
        let d = parse_action_input_ctx(r#"Task #"deep work""#, &ParseContext::default()).unwrap();
        assert!(d.tags.contains(&"deep work".to_string()));
    }

    #[test]
    fn bare_word_tag_from_context() {
        let ctx = ParseContext {
            tags: &["work", "personal"],
            ..Default::default()
        };
        let d = parse_action_input_ctx("Do work stuff work", &ctx).unwrap();
        assert!(d.tags.contains(&"work".to_string()));
    }

    #[test]
    fn multiple_hash_tags() {
        let d = parse_action_input_ctx("Task #work #focus", &ParseContext::default()).unwrap();
        assert!(d.tags.contains(&"work".to_string()));
        assert!(d.tags.contains(&"focus".to_string()));
    }
}

// ---------------------------------------------------------------------------
// Natural-language priority (Phase 6)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod nl_priority {
    use crate::ast::Priority;
    use crate::parse::parse_action_input;

    fn pri(input: &str) -> Priority {
        parse_action_input(input).unwrap().priority.unwrap()
    }

    #[test]
    fn urgent_keyword() {
        assert_eq!(pri("Buy milk urgent"), Priority::High);
    }

    #[test]
    fn asap_keyword() {
        assert_eq!(pri("File taxes asap"), Priority::High);
    }

    #[test]
    fn high_priority_phrase() {
        assert_eq!(pri("Fix bug high priority"), Priority::High);
    }

    #[test]
    fn low_priority_phrase() {
        assert_eq!(pri("Clean desk low priority"), Priority::Low);
    }

    #[test]
    fn medium_priority_phrase() {
        assert_eq!(pri("Review PR medium priority"), Priority::Medium);
    }

    #[test]
    fn whenever_keyword() {
        assert_eq!(pri("Organise photos whenever"), Priority::Low);
    }

    #[test]
    fn bang_sigil_high() {
        let d = parse_action_input("Fix bug !high").unwrap();
        assert_eq!(d.priority, Some(Priority::High));
        assert_eq!(d.title, "Fix bug");
    }

    #[test]
    fn bang_sigil_low() {
        let d = parse_action_input("Organise photos !low").unwrap();
        assert_eq!(d.priority, Some(Priority::Low));
    }

    #[test]
    fn priority_not_in_title() {
        let d = parse_action_input("Fix bug urgent").unwrap();
        assert!(!d.title.contains("urgent"));
    }
}

// ---------------------------------------------------------------------------
// Worked examples from DESIGN.md
// ---------------------------------------------------------------------------

#[cfg(test)]
mod worked_examples {
    use super::helpers::{when_dt, when_nd};
    use crate::ast::RecurrenceSpec;
    use crate::parse::{
        ParseContext, parse_action_input, parse_action_input_ctx, parse_event_input_ctx,
    };
    use chrono::{Datelike, Local, Timelike, Weekday};

    /// "Go to the store 3pm tomorrow" — time+date → DateTime
    #[test]
    fn go_to_the_store() {
        let d = parse_action_input("Go to the store 3pm tomorrow").unwrap();
        assert_eq!(d.title, "Go to the store");
        let when = when_dt(d.when).with_timezone(&Local);
        let tomorrow = (Local::now() + chrono::Duration::days(1)).date_naive();
        assert_eq!(when.date_naive(), tomorrow);
        assert_eq!(when.hour(), 15);
    }

    /// "Ask that person something later" — implied time → DateTime
    #[test]
    fn ask_that_person_later() {
        let d = parse_action_input("Ask that person something later").unwrap();
        assert_eq!(d.title, "Ask that person something");
        assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 14);
    }

    /// "Do that thing this afternoon" — implied time → DateTime
    #[test]
    fn do_that_thing_this_afternoon() {
        let d = parse_action_input("Do that thing this afternoon").unwrap();
        assert_eq!(d.title, "Do that thing");
        assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 14);
    }

    /// "Read that book every monday at 6pm for 45min" — weekday + explicit time → DateTime
    #[test]
    fn read_book_every_monday() {
        let d = parse_action_input("Read that book every monday at 6pm for 45min").unwrap();
        assert_eq!(d.title, "Read that book");
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!(when.weekday(), Weekday::Mon);
        assert_eq!(when.hour(), 18);
        assert_eq!(d.duration, Some(chrono::Duration::minutes(45)));
        if let Some(RecurrenceSpec::OnWeekdays(s)) = d.recurrence {
            assert!(s.contains(Weekday::Mon));
            assert_eq!(s.len(), 1);
        } else {
            panic!("expected OnWeekdays(Mon), got {:?}", d.recurrence);
        }
    }

    /// "Do that activity at home today" — date only → NaiveDate
    #[test]
    fn activity_at_home_today() {
        let ctx = ParseContext {
            locations: &["home", "work"],
            ..Default::default()
        };
        let d = parse_action_input_ctx("Do that activity at home today", &ctx).unwrap();
        assert_eq!(d.title, "Do that activity");
        assert_eq!(d.location.as_deref(), Some("home"));
        assert_eq!(when_nd(d.when), Local::now().date_naive());
    }

    /// "Concert 6pm next thu for 2 hours" — time+weekday → DateTime
    #[test]
    fn concert_next_thu() {
        let d = parse_action_input("Concert 6pm next thu for 2 hours").unwrap();
        assert_eq!(d.title, "Concert");
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!(when.weekday(), Weekday::Thu);
        assert_eq!(when.hour(), 18);
        assert_eq!(d.duration, Some(chrono::Duration::hours(2)));
    }

    /// "Date with Isabel at 8pm" — explicit time → DateTime
    #[test]
    fn date_with_isabel() {
        let ctx = ParseContext {
            people: &["Isabel"],
            ..Default::default()
        };
        let d = parse_action_input_ctx("Date with Isabel at 8pm", &ctx).unwrap();
        assert_eq!(d.title, "Date");
        assert_eq!(d.people, vec!["Isabel"]);
        assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 20);
    }

    /// "Team meeting at work every tuesday for 1h" — weekday (no time) → NaiveDate
    #[test]
    fn team_meeting_at_work_every_tuesday() {
        let ctx = ParseContext {
            locations: &["work", "home"],
            ..Default::default()
        };
        let d = parse_action_input_ctx("Team meeting at work every tuesday for 1h", &ctx).unwrap();
        assert_eq!(d.title, "Team meeting");
        assert_eq!(d.location.as_deref(), Some("work"));
        assert_eq!(d.duration, Some(chrono::Duration::hours(1)));
        if let Some(RecurrenceSpec::OnWeekdays(s)) = d.recurrence {
            assert!(s.contains(Weekday::Tue));
        } else {
            panic!("expected OnWeekdays, got {:?}", d.recurrence);
        }
        // when should be next Tuesday as a NaiveDate (no explicit time)
        assert_eq!(when_nd(d.when).weekday(), Weekday::Tue);
    }

    /// "Do that activity every thu, fri at noon for 2 hours"
    #[test]
    fn activity_every_thu_fri() {
        let d = parse_action_input("Do that activity every thu, fri at noon for 2 hours").unwrap();
        assert_eq!(d.title, "Do that activity");
        assert_eq!(d.duration, Some(chrono::Duration::hours(2)));
        if let Some(RecurrenceSpec::OnWeekdays(s)) = d.recurrence {
            assert!(s.contains(Weekday::Thu));
            assert!(s.contains(Weekday::Fri));
            assert_eq!(s.len(), 2);
        } else {
            panic!("expected OnWeekdays, got {:?}", d.recurrence);
        }
    }

    /// "event" parse with time using parse_event_input_ctx — @ sigil → DateTime
    #[test]
    fn event_team_meeting_sigil() {
        let d = parse_event_input_ctx("Team meeting @2025-06-15 10:00", &ParseContext::default())
            .unwrap();
        assert_eq!(d.title, "Team meeting");
        let when = when_dt(d.when).with_timezone(&Local);
        assert_eq!((when.hour(), when.minute()), (10, 0));
        assert_eq!((when.year(), when.month(), when.day()), (2025, 6, 15));
    }
}

// ---------------------------------------------------------------------------
// ParseContext — context-aware parsing
// ---------------------------------------------------------------------------

#[cfg(test)]
mod parse_context {
    use crate::parse::{ParseContext, parse_action_input_ctx};

    #[test]
    fn empty_context_is_default() {
        let d = parse_action_input_ctx("Buy milk", &ParseContext::default()).unwrap();
        assert_eq!(d.title, "Buy milk");
    }

    #[test]
    fn location_and_people_and_tags() {
        let ctx = ParseContext {
            locations: &["home"],
            people: &["Alice"],
            tags: &["personal"],
        };
        let d = parse_action_input_ctx("Catch up with Alice at home", &ctx).unwrap();
        assert_eq!(d.title, "Catch up");
        assert!(d.people.contains(&"Alice".to_string()));
        assert_eq!(d.location.as_deref(), Some("home"));
    }

    #[test]
    fn unknown_words_stay_in_title() {
        let ctx = ParseContext {
            locations: &["office"],
            ..Default::default()
        };
        let d = parse_action_input_ctx("Buy groceries at the store", &ctx).unwrap();
        // "the store" is not in the known locations list → stays in title
        assert!(d.title.contains("groceries"));
        assert_eq!(d.location, None);
    }
}

// ---------------------------------------------------------------------------
// % recurrence sigil (now wired up)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod percent_sigil {
    use crate::ast::RecurrenceSpec;
    use crate::parse::parse_action_input;
    use chrono::Weekday::*;

    #[test]
    fn percent_daily() {
        let d = parse_action_input("Morning run %daily").unwrap();
        assert_eq!(d.recurrence, Some(RecurrenceSpec::EveryDays(1)));
        assert_eq!(d.title, "Morning run");
    }

    #[test]
    fn percent_weekly() {
        let d = parse_action_input("Review goals %weekly").unwrap();
        assert_eq!(d.recurrence, Some(RecurrenceSpec::EveryWeeks(1)));
    }

    #[test]
    fn percent_every_monday() {
        let d = parse_action_input("Standup %every monday").unwrap();
        if let Some(RecurrenceSpec::OnWeekdays(s)) = d.recurrence {
            assert!(s.contains(Mon));
        } else {
            panic!("expected OnWeekdays");
        }
    }

    #[test]
    fn percent_every_3_days() {
        let d = parse_action_input("Water plants %every 3 days").unwrap();
        assert_eq!(d.recurrence, Some(RecurrenceSpec::EveryDays(3)));
    }
}

// ---------------------------------------------------------------------------
// Edge cases and regression tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod edge_cases {
    use super::helpers::{when_dt, when_nd};
    use crate::parse::parse_action_input;
    use chrono::{Duration, Local, Timelike};

    #[test]
    fn all_specifiers_together_sigil_style() {
        let d = parse_action_input("Deep work @2025-06-15 ~2h %weekly").unwrap();
        assert_eq!(d.title, "Deep work");
        assert!(d.when.is_some());
        assert_eq!(d.duration, Some(Duration::hours(2)));
        assert!(d.recurrence.is_some());
    }

    #[test]
    fn title_with_digits_not_misread_as_time() {
        let d = parse_action_input("Drink 8 glasses of water").unwrap();
        assert_eq!(d.title, "Drink 8 glasses of water");
        assert!(d.when.is_none());
    }

    #[test]
    fn number_not_a_duration_without_unit() {
        let d = parse_action_input("Take 8 deep breaths").unwrap();
        assert_eq!(d.title, "Take 8 deep breaths");
        assert!(d.duration.is_none());
    }

    #[test]
    fn time_and_duration_together_nl() {
        // bare time token → always DateTime
        let d = parse_action_input("Call 3pm for 30 minutes").unwrap();
        assert_eq!(d.title, "Call");
        assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 15);
        assert_eq!(d.duration, Some(Duration::minutes(30)));
    }

    #[test]
    fn recurrence_and_time_and_duration() {
        // "at 7am" → always DateTime
        let d = parse_action_input("Morning run every day at 7am for 30min").unwrap();
        assert_eq!(d.title, "Morning run");
        assert!(d.recurrence.is_some());
        assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 7);
        assert_eq!(d.duration, Some(Duration::minutes(30)));
    }

    #[test]
    fn iso_date_in_nl_position() {
        // ISO date without time → NaiveDate for actions
        let d = parse_action_input("Workshop 2025-09-01 for 3 hours").unwrap();
        assert_eq!(d.title, "Workshop");
        assert!(d.when.is_some());
        // confirm it's a NaiveDate, not a DateTime
        let _ = when_nd(d.when);
        assert_eq!(d.duration, Some(Duration::hours(3)));
    }

    #[test]
    fn quoted_title_with_clause() {
        let d = parse_action_input(r#""Buy milk" tomorrow"#).unwrap();
        assert_eq!(d.title, "Buy milk");
        assert!(d.when.is_some());
    }

    #[test]
    fn duration_without_for_sigil_form_still_works() {
        // The ~sigil form still works even with NL parsing active
        let d = parse_action_input("Task ~2h").unwrap();
        assert_eq!(d.duration, Some(Duration::hours(2)));
    }

    // ---------------------------------------------------------------------------
    // Short am/pm suffixes: 3p, 11a, 10:30p, etc.
    // ---------------------------------------------------------------------------

    #[cfg(test)]
    mod short_time_suffix {
        use super::super::helpers::when_dt;
        use crate::lexer::{Token, lex};
        use crate::parse::parse_action_input;
        use chrono::{Datelike, Local, Timelike};

        // --- Lexer: short suffixes produce Time12 tokens ---

        #[test]
        fn bare_3p_lexes_as_time12() {
            let tokens = lex("3p");
            assert_eq!(tokens.len(), 1);
            assert_eq!(tokens[0].token, Token::Time12);
            assert_eq!(tokens[0].text, "3p");
        }

        #[test]
        fn bare_11a_lexes_as_time12() {
            let tokens = lex("11a");
            assert_eq!(tokens.len(), 1);
            assert_eq!(tokens[0].token, Token::Time12);
            assert_eq!(tokens[0].text, "11a");
        }

        #[test]
        fn colon_form_10_30p_lexes_as_time12() {
            let tokens = lex("10:30p");
            assert_eq!(tokens.len(), 1);
            assert_eq!(tokens[0].token, Token::Time12);
            assert_eq!(tokens[0].text, "10:30p");
        }

        #[test]
        fn colon_form_9_15a_lexes_as_time12() {
            let tokens = lex("9:15a");
            assert_eq!(tokens.len(), 1);
            assert_eq!(tokens[0].token, Token::Time12);
            assert_eq!(tokens[0].text, "9:15a");
        }

        // Existing two-letter suffixes must still lex correctly.
        #[test]
        fn full_am_suffix_still_lexes_as_time12() {
            let tokens = lex("9am");
            assert_eq!(tokens.len(), 1);
            assert_eq!(tokens[0].token, Token::Time12);
        }

        #[test]
        fn full_pm_suffix_still_lexes_as_time12() {
            let tokens = lex("3pm");
            assert_eq!(tokens.len(), 1);
            assert_eq!(tokens[0].token, Token::Time12);
        }

        // --- Parser: bare short-suffix times are parsed correctly ---

        #[test]
        fn bare_3p_parses_to_15h() {
            // bare time token → always DateTime
            let d = parse_action_input("Call 3p").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!((when.hour(), when.minute()), (15, 0));
        }

        #[test]
        fn bare_11a_parses_to_11h() {
            let d = parse_action_input("Meeting 11a").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!((when.hour(), when.minute()), (11, 0));
        }

        #[test]
        fn bare_12p_parses_to_noon() {
            let d = parse_action_input("Lunch 12p").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!((when.hour(), when.minute()), (12, 0));
        }

        #[test]
        fn bare_12a_parses_to_midnight() {
            let d = parse_action_input("Task 12a").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!((when.hour(), when.minute()), (0, 0));
        }

        #[test]
        fn colon_10_30p_parses_to_22h30() {
            let d = parse_action_input("Call 10:30p").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!((when.hour(), when.minute()), (22, 30));
        }

        #[test]
        fn colon_9_15a_parses_to_9h15() {
            let d = parse_action_input("Standup 9:15a").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!((when.hour(), when.minute()), (9, 15));
        }

        // --- "at" preposition form → always DateTime ---

        #[test]
        fn at_3p_preposition() {
            let d = parse_action_input("Call at 3p").unwrap();
            assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 15);
        }

        #[test]
        fn at_11a_preposition() {
            let d = parse_action_input("Meeting at 11a").unwrap();
            assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 11);
        }

        // --- Combined with date keywords — time present → DateTime ---

        #[test]
        fn tomorrow_3p() {
            let d = parse_action_input("Call tomorrow 3p").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!(when.hour(), 15);
            let tomorrow = (chrono::Local::now() + chrono::Duration::days(1)).date_naive();
            assert_eq!(when.date_naive(), tomorrow);
        }

        #[test]
        fn today_at_9a() {
            let d = parse_action_input("Standup today at 9a").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!(when.hour(), 9);
            assert_eq!(when.date_naive(), chrono::Local::now().date_naive());
        }

        #[test]
        fn next_monday_6p() {
            let d = parse_action_input("Review next monday 6p").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!(when.hour(), 18);
            assert_eq!(when.weekday(), chrono::Weekday::Mon);
        }

        // --- Title is not contaminated ---

        #[test]
        fn title_correct_with_short_suffix() {
            let d = parse_action_input("Call Alice 3p").unwrap();
            assert_eq!(d.title, "Call Alice");
            assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 15);
        }

        // --- Short suffix in sigil @ form → DateTime ---

        #[test]
        fn sigil_at_with_short_suffix() {
            let d = parse_action_input("Call @3p").unwrap();
            assert_eq!(when_dt(d.when).with_timezone(&Local).hour(), 15);
            assert_eq!(d.title, "Call");
        }
    }

    // ---------------------------------------------------------------------------
    // "tom" abbreviation for tomorrow
    // ---------------------------------------------------------------------------

    #[cfg(test)]
    mod tom_abbreviation {
        use super::super::helpers::{when_dt, when_nd};
        use crate::parse::parse_action_input;
        use chrono::{Local, Timelike};

        fn tomorrow_date() -> chrono::NaiveDate {
            (chrono::Local::now() + chrono::Duration::days(1)).date_naive()
        }

        // --- Bare "tom" (no time) → WhenSpec::NaiveDate ---

        #[test]
        fn tom_alone_sets_tomorrow() {
            let d = parse_action_input("Call tom").unwrap();
            assert_eq!(when_nd(d.when), tomorrow_date());
        }

        #[test]
        fn tom_uses_default_time() {
            // "tom" alone no longer pins a time — it gives a floating NaiveDate.
            // The action will be promoted to the queue at schedule time.
            let d = parse_action_input("Call tom").unwrap();
            assert_eq!(when_nd(d.when), tomorrow_date());
        }

        // --- "tom" WITH time suffixes → WhenSpec::DateTime ---

        #[test]
        fn tom_at_3pm() {
            let d = parse_action_input("Call tom at 3pm").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!(when.date_naive(), tomorrow_date());
            assert_eq!(when.hour(), 15);
        }

        #[test]
        fn tom_bare_time_3pm() {
            let d = parse_action_input("Call tom 3pm").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!(when.date_naive(), tomorrow_date());
            assert_eq!(when.hour(), 15);
        }

        #[test]
        fn tom_with_short_suffix_3p() {
            let d = parse_action_input("Meeting tom 3p").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!(when.date_naive(), tomorrow_date());
            assert_eq!(when.hour(), 15);
        }

        #[test]
        fn tom_at_short_suffix_9a() {
            let d = parse_action_input("Standup tom at 9a").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!(when.date_naive(), tomorrow_date());
            assert_eq!(when.hour(), 9);
        }

        #[test]
        fn tom_with_24h_time() {
            let d = parse_action_input("Call tom 14:30").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!(when.date_naive(), tomorrow_date());
            assert_eq!((when.hour(), when.minute()), (14, 30));
        }

        // --- "tom" combined with duration (no time) → NaiveDate ---

        #[test]
        fn tom_with_duration() {
            let d = parse_action_input("Workshop tom for 2h").unwrap();
            assert_eq!(when_nd(d.when), tomorrow_date());
            assert_eq!(d.duration, Some(chrono::Duration::hours(2)));
        }

        // --- Title is not contaminated ---

        #[test]
        fn title_correct_with_tom() {
            let d = parse_action_input("Call Alice tom").unwrap();
            assert_eq!(d.title, "Call Alice");
            assert_eq!(when_nd(d.when), tomorrow_date());
        }

        #[test]
        fn tom_at_start_title_correct() {
            let d = parse_action_input("tom Call Alice").unwrap();
            assert_eq!(d.title, "Call Alice");
            assert_eq!(when_nd(d.when), tomorrow_date());
        }

        // --- time-first order: time+tom → DateTime ---

        #[test]
        fn time_first_then_tom() {
            let d = parse_action_input("Call 3pm tom").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!(when.date_naive(), tomorrow_date());
            assert_eq!(when.hour(), 15);
        }

        #[test]
        fn short_time_first_then_tom() {
            let d = parse_action_input("Call 3p tom").unwrap();
            let when = when_dt(d.when).with_timezone(&Local);
            assert_eq!(when.date_naive(), tomorrow_date());
            assert_eq!(when.hour(), 15);
        }
    }
}

// ---------------------------------------------------------------------------
// Monthly / yearly / OnMonthDay recurrence
// ---------------------------------------------------------------------------

#[cfg(test)]
mod nl_monthly_recurrence {
    use crate::ast::RecurrenceSpec;
    use crate::parse::{parse_action_input, parse_recurrence_str, try_recurrence_text};

    fn rec(input: &str) -> RecurrenceSpec {
        parse_action_input(input).unwrap().recurrence.unwrap()
    }

    // --- "monthly" keyword ---

    #[test]
    fn monthly_keyword_nl() {
        assert_eq!(rec("Pay rent monthly"), RecurrenceSpec::EveryMonths(1));
    }

    #[test]
    fn monthly_via_try_recurrence_text() {
        assert_eq!(
            try_recurrence_text("monthly"),
            Some(RecurrenceSpec::EveryMonths(1))
        );
    }

    #[test]
    fn every_month_phrase() {
        assert_eq!(
            try_recurrence_text("every month"),
            Some(RecurrenceSpec::EveryMonths(1))
        );
    }

    #[test]
    fn every_n_months() {
        assert_eq!(
            rec("Dentist every 3 months"),
            RecurrenceSpec::EveryMonths(3)
        );
    }

    #[test]
    fn every_2_months() {
        assert_eq!(
            try_recurrence_text("every 2 months"),
            Some(RecurrenceSpec::EveryMonths(2))
        );
    }

    #[test]
    fn quarterly_keyword() {
        assert_eq!(
            try_recurrence_text("quarterly"),
            Some(RecurrenceSpec::EveryMonths(3))
        );
    }

    // --- "yearly" / "annually" keywords ---

    #[test]
    fn yearly_keyword_nl() {
        assert_eq!(rec("File taxes yearly"), RecurrenceSpec::EveryYears(1));
    }

    #[test]
    fn annually_keyword_nl() {
        assert_eq!(
            rec("Renew passport annually"),
            RecurrenceSpec::EveryYears(1)
        );
    }

    #[test]
    fn every_year_phrase() {
        assert_eq!(
            try_recurrence_text("every year"),
            Some(RecurrenceSpec::EveryYears(1))
        );
    }

    #[test]
    fn every_2_years() {
        assert_eq!(
            try_recurrence_text("every 2 years"),
            Some(RecurrenceSpec::EveryYears(2))
        );
    }

    // --- "every <Nth>" — bare ordinal ---

    #[test]
    fn every_1st() {
        assert_eq!(
            try_recurrence_text("every 1st"),
            Some(RecurrenceSpec::OnMonthDay(1))
        );
    }

    #[test]
    fn every_15th() {
        assert_eq!(
            try_recurrence_text("every 15th"),
            Some(RecurrenceSpec::OnMonthDay(15))
        );
    }

    #[test]
    fn every_30th() {
        assert_eq!(
            try_recurrence_text("every 30th"),
            Some(RecurrenceSpec::OnMonthDay(30))
        );
    }

    #[test]
    fn every_2nd() {
        assert_eq!(
            try_recurrence_text("every 2nd"),
            Some(RecurrenceSpec::OnMonthDay(2))
        );
    }

    #[test]
    fn every_3rd() {
        assert_eq!(
            try_recurrence_text("every 3rd"),
            Some(RecurrenceSpec::OnMonthDay(3))
        );
    }

    // --- "every <Nth> of the month" ---

    #[test]
    fn every_15th_of_the_month() {
        assert_eq!(
            try_recurrence_text("every 15th of the month"),
            Some(RecurrenceSpec::OnMonthDay(15))
        );
    }

    #[test]
    fn every_1st_of_the_month() {
        assert_eq!(
            try_recurrence_text("every 1st of the month"),
            Some(RecurrenceSpec::OnMonthDay(1))
        );
    }

    #[test]
    fn every_30th_of_every_month() {
        assert_eq!(
            try_recurrence_text("every 30th of every month"),
            Some(RecurrenceSpec::OnMonthDay(30))
        );
    }

    // --- "<Nth> of every month" (no leading "every") ---

    #[test]
    fn first_of_every_month() {
        assert_eq!(
            try_recurrence_text("1st of every month"),
            Some(RecurrenceSpec::OnMonthDay(1))
        );
    }

    #[test]
    fn fifteenth_of_every_month() {
        assert_eq!(
            try_recurrence_text("15th of every month"),
            Some(RecurrenceSpec::OnMonthDay(15))
        );
    }

    #[test]
    fn ordinal_of_the_month() {
        assert_eq!(
            try_recurrence_text("30th of the month"),
            Some(RecurrenceSpec::OnMonthDay(30))
        );
    }

    // --- natural-language full-sentence parsing ---

    #[test]
    fn pay_rent_every_1st() {
        let d = parse_action_input("Pay rent every 1st").unwrap();
        assert_eq!(d.title, "Pay rent");
        assert_eq!(d.recurrence, Some(RecurrenceSpec::OnMonthDay(1)));
    }

    #[test]
    fn invoice_every_30th() {
        let d = parse_action_input("Send invoice every 30th").unwrap();
        assert_eq!(d.title, "Send invoice");
        assert_eq!(d.recurrence, Some(RecurrenceSpec::OnMonthDay(30)));
    }

    #[test]
    fn review_every_15th_of_month() {
        let d = parse_action_input("Review budget every 15th of the month").unwrap();
        assert_eq!(d.title, "Review budget");
        assert_eq!(d.recurrence, Some(RecurrenceSpec::OnMonthDay(15)));
    }

    #[test]
    fn recurrence_title_not_contaminated_monthly() {
        let d = parse_action_input("Pay rent monthly").unwrap();
        assert_eq!(d.title, "Pay rent");
        assert!(d.recurrence.is_some());
    }

    #[test]
    fn recurrence_title_not_contaminated_yearly() {
        let d = parse_action_input("File taxes yearly").unwrap();
        assert_eq!(d.title, "File taxes");
        assert!(d.recurrence.is_some());
    }

    // --- parse_recurrence_str public API ---

    #[test]
    fn parse_recurrence_str_monthly() {
        assert_eq!(
            parse_recurrence_str("monthly").unwrap(),
            RecurrenceSpec::EveryMonths(1)
        );
    }

    #[test]
    fn parse_recurrence_str_yearly() {
        assert_eq!(
            parse_recurrence_str("yearly").unwrap(),
            RecurrenceSpec::EveryYears(1)
        );
    }

    #[test]
    fn parse_recurrence_str_on_month_day() {
        assert_eq!(
            parse_recurrence_str("every 15th").unwrap(),
            RecurrenceSpec::OnMonthDay(15)
        );
    }
}

// ---------------------------------------------------------------------------
// format_recurrence display formatting (mirrors desktop logic, tested here
// so the parser crate is self-contained and changes are caught early)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod format_recurrence {
    use crate::ast::{RecurrenceSpec, WeekdaySet};
    use chrono::Weekday::*;

    /// Replicate the formatting logic from desktop's `format_recurrence` so
    /// we can test it without depending on gpui.
    fn fmt(spec: RecurrenceSpec) -> String {
        match spec {
            RecurrenceSpec::EveryDays(1) => "daily".into(),
            RecurrenceSpec::EveryDays(7) => "weekly".into(),
            RecurrenceSpec::EveryDays(n) => format!("every {n} days"),
            RecurrenceSpec::EveryWeeks(1) => "weekly".into(),
            RecurrenceSpec::EveryWeeks(n) => format!("every {n} weeks"),
            RecurrenceSpec::EveryMonths(1) => "monthly".into(),
            RecurrenceSpec::EveryMonths(3) => "quarterly".into(),
            RecurrenceSpec::EveryMonths(n) => format!("every {n} months"),
            RecurrenceSpec::EveryYears(1) => "yearly".into(),
            RecurrenceSpec::EveryYears(n) => format!("every {n} years"),
            RecurrenceSpec::OnMonthDay(day) => format!("the {}", ordinal(day)),
            RecurrenceSpec::OnWeekdays(set) => fmt_weekday_set(&set),
        }
    }

    fn ordinal(n: u32) -> String {
        let suffix = match n % 100 {
            11 | 12 | 13 => "th",
            _ => match n % 10 {
                1 => "st",
                2 => "nd",
                3 => "rd",
                _ => "th",
            },
        };
        format!("{n}{suffix}")
    }

    fn fmt_weekday_set(set: &WeekdaySet) -> String {
        if *set == WeekdaySet::every_day() {
            return "daily".into();
        }
        if *set == WeekdaySet::weekdays() {
            return "weekdays".into();
        }
        if *set == WeekdaySet::weekends() {
            return "weekends".into();
        }
        let names: Vec<&str> = [Mon, Tue, Wed, Thu, Fri, Sat, Sun]
            .iter()
            .filter(|&&d| set.contains(d))
            .map(|d| match d {
                Mon => "Mon",
                Tue => "Tue",
                Wed => "Wed",
                Thu => "Thu",
                Fri => "Fri",
                Sat => "Sat",
                Sun => "Sun",
            })
            .collect();
        names.join(", ")
    }

    // --- EveryDays ---

    #[test]
    fn every_days_1_is_daily() {
        assert_eq!(fmt(RecurrenceSpec::EveryDays(1)), "daily");
    }

    #[test]
    fn every_days_7_is_weekly() {
        assert_eq!(fmt(RecurrenceSpec::EveryDays(7)), "weekly");
    }

    #[test]
    fn every_days_n() {
        assert_eq!(fmt(RecurrenceSpec::EveryDays(3)), "every 3 days");
    }

    // --- EveryWeeks ---

    #[test]
    fn every_weeks_1_is_weekly() {
        assert_eq!(fmt(RecurrenceSpec::EveryWeeks(1)), "weekly");
    }

    #[test]
    fn every_weeks_2() {
        assert_eq!(fmt(RecurrenceSpec::EveryWeeks(2)), "every 2 weeks");
    }

    // --- EveryMonths ---

    #[test]
    fn every_months_1_is_monthly() {
        assert_eq!(fmt(RecurrenceSpec::EveryMonths(1)), "monthly");
    }

    #[test]
    fn every_months_3_is_quarterly() {
        assert_eq!(fmt(RecurrenceSpec::EveryMonths(3)), "quarterly");
    }

    #[test]
    fn every_months_6() {
        assert_eq!(fmt(RecurrenceSpec::EveryMonths(6)), "every 6 months");
    }

    // --- EveryYears ---

    #[test]
    fn every_years_1_is_yearly() {
        assert_eq!(fmt(RecurrenceSpec::EveryYears(1)), "yearly");
    }

    #[test]
    fn every_years_2() {
        assert_eq!(fmt(RecurrenceSpec::EveryYears(2)), "every 2 years");
    }

    // --- OnMonthDay ordinal formatting ---

    #[test]
    fn on_month_day_1st() {
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(1)), "the 1st");
    }

    #[test]
    fn on_month_day_2nd() {
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(2)), "the 2nd");
    }

    #[test]
    fn on_month_day_3rd() {
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(3)), "the 3rd");
    }

    #[test]
    fn on_month_day_4th() {
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(4)), "the 4th");
    }

    #[test]
    fn on_month_day_11th() {
        // 11 is irregular — must be "11th" not "11st"
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(11)), "the 11th");
    }

    #[test]
    fn on_month_day_12th() {
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(12)), "the 12th");
    }

    #[test]
    fn on_month_day_13th() {
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(13)), "the 13th");
    }

    #[test]
    fn on_month_day_21st() {
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(21)), "the 21st");
    }

    #[test]
    fn on_month_day_22nd() {
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(22)), "the 22nd");
    }

    #[test]
    fn on_month_day_30th() {
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(30)), "the 30th");
    }

    #[test]
    fn on_month_day_31st() {
        assert_eq!(fmt(RecurrenceSpec::OnMonthDay(31)), "the 31st");
    }

    // --- OnWeekdays named sets ---

    #[test]
    fn on_weekdays_all_is_daily() {
        assert_eq!(
            fmt(RecurrenceSpec::OnWeekdays(WeekdaySet::every_day())),
            "daily"
        );
    }

    #[test]
    fn on_weekdays_mon_fri_is_weekdays() {
        assert_eq!(
            fmt(RecurrenceSpec::OnWeekdays(WeekdaySet::weekdays())),
            "weekdays"
        );
    }

    #[test]
    fn on_weekdays_sat_sun_is_weekends() {
        assert_eq!(
            fmt(RecurrenceSpec::OnWeekdays(WeekdaySet::weekends())),
            "weekends"
        );
    }

    #[test]
    fn on_weekdays_mon_wed_fri() {
        let set = WeekdaySet::new([Mon, Wed, Fri]);
        assert_eq!(fmt(RecurrenceSpec::OnWeekdays(set)), "Mon, Wed, Fri");
    }

    #[test]
    fn on_weekdays_tue_thu() {
        let set = WeekdaySet::new([Tue, Thu]);
        assert_eq!(fmt(RecurrenceSpec::OnWeekdays(set)), "Tue, Thu");
    }

    #[test]
    fn on_weekdays_single_friday() {
        let set = WeekdaySet::new([Fri]);
        assert_eq!(fmt(RecurrenceSpec::OnWeekdays(set)), "Fri");
    }
}
