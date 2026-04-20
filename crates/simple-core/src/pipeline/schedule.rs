use chrono::{DateTime, Duration, Utc};

use crate::Action;

use super::{CONSECUTIVE_GAP_THRESHOLD, DEFAULT_ACTION_DURATION, QueueItem};

pub(super) fn action_effective_duration(action: &Action) -> Duration {
    action.duration.unwrap_or(DEFAULT_ACTION_DURATION)
}

pub(super) fn action_end_time(action: &Action) -> Option<DateTime<Utc>> {
    action.target.map(|t| t + action_effective_duration(action))
}

pub(super) fn is_missed(action: &Action, now: DateTime<Utc>) -> bool {
    match action_end_time(action) {
        Some(end) => end <= now,
        None => false,
    }
}

pub(super) fn event_intervals(queue: &[QueueItem]) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut intervals: Vec<(DateTime<Utc>, DateTime<Utc>)> = queue
        .iter()
        .filter_map(|item| {
            if let QueueItem::Event(event) = item {
                Some((event.time, event.end_time()))
            } else {
                None
            }
        })
        .collect();
    intervals.sort_by_key(|(start, _)| *start);
    intervals
}

pub(super) fn all_anchor_intervals(queue: &[QueueItem]) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut intervals: Vec<(DateTime<Utc>, DateTime<Utc>)> = queue
        .iter()
        .filter_map(|item| match item {
            QueueItem::Event(e) => Some((e.time, e.end_time())),
            QueueItem::Action(a) if a.target_static => {
                let t = a.target?;
                Some((t, t + action_effective_duration(a)))
            }
            _ => None,
        })
        .collect();
    intervals.sort_by_key(|(start, _)| *start);
    intervals
}

fn round_up_to_5_minutes(dt: DateTime<Utc>) -> DateTime<Utc> {
    let total_seconds = dt.timestamp();
    let step = 5 * 60;
    let remainder = total_seconds % step;
    if remainder == 0 {
        dt
    } else {
        dt + Duration::seconds(step - remainder)
    }
}

pub(super) fn find_free_slot(
    earliest_start: DateTime<Utc>,
    total_duration: Duration,
    occupied: &[(DateTime<Utc>, DateTime<Utc>)],
) -> DateTime<Utc> {
    // Do NOT round up earliest_start here — rounding up front can skip a valid
    // gap between earliest_start and the next 5-minute boundary.
    let mut candidate = earliest_start;

    let mut i = 0;
    while i < occupied.len() {
        let (interval_start, interval_end) = occupied[i];

        if interval_end <= candidate {
            i += 1;
            continue;
        }

        let candidate_end = candidate + total_duration;
        if interval_start >= candidate_end {
            break;
        }

        candidate = round_up_to_5_minutes(interval_end);
    }

    round_up_to_5_minutes(candidate)
}

pub(super) fn group_consecutive(actions: Vec<Action>) -> Vec<Vec<Action>> {
    group_by_gap(actions, CONSECUTIVE_GAP_THRESHOLD)
}

pub(super) fn group_by_gap(actions: Vec<Action>, gap_threshold: Duration) -> Vec<Vec<Action>> {
    let mut groups: Vec<Vec<Action>> = Vec::new();

    for action in actions {
        let has_target = action.target.is_some();

        if has_target {
            let attach = groups.last_mut().and_then(|group| {
                let last = group.last()?;
                let last_end = action_end_time(last)?;
                let gap = action.target? - last_end;
                if gap <= gap_threshold { Some(()) } else { None }
            });

            if attach.is_some() {
                groups.last_mut().unwrap().push(action);
            } else {
                groups.push(vec![action]);
            }
        } else {
            groups.push(vec![action]);
        }
    }

    groups
}

pub(super) fn compact_cluster(cluster: Vec<Action>, cluster_start: DateTime<Utc>) -> Vec<Action> {
    let mut result = Vec::with_capacity(cluster.len());
    let mut cursor = cluster_start;

    for mut action in cluster {
        action.target = Some(cursor);
        cursor = cursor + action_effective_duration(&action);
        result.push(action);
    }

    result
}

pub(super) fn compacted_cluster_duration(cluster: &[Action]) -> Duration {
    if cluster.is_empty() {
        return Duration::zero();
    }
    cluster
        .iter()
        .map(action_effective_duration)
        .fold(Duration::zero(), |acc, d| acc + d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, Event};
    use chrono::TimeZone;
    use uuid::Uuid;

    use crate::pipeline::{Pipeline, QueueItem};

    fn hm(hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 1, 1, hour, minute, 0).unwrap()
    }

    fn hms(hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2024, 1, 1, hour, minute, second)
            .unwrap()
    }

    fn make_action(target_hm: (u32, u32), duration_mins: i64) -> Action {
        Action {
            id: Uuid::now_v7(),
            lineage_id: Uuid::now_v7(),
            origin_routine_id: None,
            title: format!("action@{}:{}", target_hm.0, target_hm.1),
            content: None,
            target: Some(hm(target_hm.0, target_hm.1)),
            target_static: false,
            naive_date: None,
            duration: Some(Duration::minutes(duration_mins)),
            recurrence: None,
            ephemeral: true,
            completed_at: None,
        }
    }

    fn make_event(time_hm: (u32, u32), duration_mins: i64) -> Event {
        Event {
            id: Uuid::now_v7(),
            lineage_id: Uuid::now_v7(),
            title: format!("event@{}:{}", time_hm.0, time_hm.1),
            content: None,
            time: hm(time_hm.0, time_hm.1),
            duration: Some(Duration::minutes(duration_mins)),
            recurrence: None,
            ephemeral: true,
        }
    }

    #[test]
    fn find_free_slot_empty() {
        let result = find_free_slot(hms(10, 3, 0), Duration::minutes(5), &[]);
        assert_eq!(result, hm(10, 5));
    }

    #[test]
    fn find_free_slot_already_on_boundary() {
        let result = find_free_slot(hm(10, 0), Duration::minutes(5), &[]);
        assert_eq!(result, hm(10, 0));
    }

    /// Key regression: action ends at 23:45, event starts at 23:50, new action
    /// needs 5 min — should land at 23:45, not after the event.
    #[test]
    fn find_free_slot_gap_fits_exactly() {
        let occupied = vec![(hm(23, 40), hm(23, 45)), (hm(23, 50), hm(0, 20))];
        let result = find_free_slot(hm(22, 0), Duration::minutes(5), &occupied);
        assert_eq!(
            result,
            hm(22, 0),
            "should use first gap before existing action"
        );

        let result = find_free_slot(hm(23, 45), Duration::minutes(5), &occupied);
        assert_eq!(result, hm(23, 45), "should fit in the exact 5-min gap");

        let result = find_free_slot(hms(23, 44, 30), Duration::minutes(5), &occupied);
        assert_eq!(result, hm(23, 45), "should round up into the exact gap");
    }

    #[test]
    fn find_free_slot_gap_too_small() {
        let event_start = Utc.with_ymd_and_hms(2024, 1, 1, 23, 48, 0).unwrap();
        let event_end = Utc.with_ymd_and_hms(2024, 1, 2, 0, 18, 0).unwrap();
        let occupied = vec![(hm(23, 40), hm(23, 45)), (event_start, event_end)];
        let result = find_free_slot(hm(23, 45), Duration::minutes(5), &occupied);
        let expected = Utc.with_ymd_and_hms(2024, 1, 2, 0, 20, 0).unwrap();
        assert_eq!(result, expected);
    }

    /// Regression for the off-by-one that used strict > instead of >=.
    #[test]
    fn find_free_slot_candidate_end_touches_interval_start() {
        let occupied = vec![(hm(5, 40), hm(5, 45)), (hm(5, 50), hm(7, 50))];
        let result = find_free_slot(hm(5, 45), Duration::minutes(5), &occupied);
        assert_eq!(
            result,
            hm(5, 45),
            "gap of exactly 5 min before event must be accepted"
        );
    }

    /// Regression for the bug where duplicate intervals pushed the slot past the event.
    #[test]
    fn find_free_slot_duplicate_intervals() {
        let occupied = vec![
            (hm(5, 30), hm(5, 35)),
            (hm(5, 30), hm(5, 35)),
            (hm(5, 35), hm(5, 40)),
            (hm(5, 35), hm(5, 40)),
            (hm(5, 40), hm(5, 45)),
            (hm(5, 50), hm(7, 50)),
        ];
        let result = find_free_slot(hms(5, 34, 40), Duration::minutes(5), &occupied);
        assert_eq!(
            result,
            hm(5, 45),
            "duplicates must not push slot past the event; gap [05:45, 05:50) fits exactly"
        );
    }

    #[test]
    fn find_free_slot_back_to_back_no_gap() {
        let occupied = vec![
            (hm(10, 0), hm(10, 5)),
            (hm(10, 5), hm(10, 10)),
            (hm(10, 10), hm(10, 15)),
        ];
        let result = find_free_slot(hm(10, 0), Duration::minutes(5), &occupied);
        assert_eq!(result, hm(10, 15));
    }

    #[test]
    fn find_free_slot_interval_in_past() {
        let occupied = vec![(hm(8, 0), hm(8, 5))];
        let result = find_free_slot(hm(9, 0), Duration::minutes(5), &occupied);
        assert_eq!(result, hm(9, 0));
    }

    #[test]
    fn next_available_slot_before_action_and_event() {
        let mut pipeline = Pipeline::default();
        pipeline
            .queue
            .push(QueueItem::Action(make_action((23, 40), 5)));
        pipeline
            .queue
            .push(QueueItem::Event(make_event((23, 50), 120)));

        let slot = pipeline.next_available_slot(hm(22, 0), Duration::minutes(5));
        assert_eq!(slot, hm(22, 0));
    }

    #[test]
    fn next_available_slot_now_in_gap_before_event() {
        let mut pipeline = Pipeline::default();
        pipeline
            .queue
            .push(QueueItem::Action(make_action((23, 40), 5)));
        pipeline
            .queue
            .push(QueueItem::Event(make_event((23, 50), 120)));

        let slot = pipeline.next_available_slot(hm(23, 45), Duration::minutes(5));
        assert_eq!(slot, hm(23, 45), "23:45 fits exactly before event at 23:50");
    }

    #[test]
    fn next_available_slot_now_past_gap() {
        let mut pipeline = Pipeline::default();
        pipeline
            .queue
            .push(QueueItem::Action(make_action((23, 40), 5)));
        pipeline
            .queue
            .push(QueueItem::Event(make_event((23, 50), 120)));

        let slot = pipeline.next_available_slot(hms(23, 46, 0), Duration::minutes(5));
        assert!(
            slot >= hm(1, 50),
            "no room before event; must go after event end 01:50"
        );
    }

    #[test]
    fn queue_action_auto_finds_gap_between_action_and_event() {
        let mut pipeline = Pipeline::default();
        pipeline
            .queue
            .push(QueueItem::Action(make_action((23, 40), 5)));
        pipeline
            .queue
            .push(QueueItem::Event(make_event((23, 50), 120)));

        let new_action = Action {
            id: Uuid::now_v7(),
            lineage_id: Uuid::now_v7(),
            origin_routine_id: None,
            title: "new".into(),
            content: None,
            target: None,
            target_static: false,
            naive_date: None,
            duration: Some(Duration::minutes(5)),
            recurrence: None,
            ephemeral: true,
            completed_at: None,
        };

        pipeline.queue_action_auto(new_action, hm(23, 45));

        let added = pipeline
            .queue
            .iter()
            .find_map(|item| {
                if let QueueItem::Action(a) = item {
                    if a.title == "new" {
                        Some(a.target)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .flatten();

        assert_eq!(
            added,
            Some(hm(23, 45)),
            "new action should land at 23:45, not after event"
        );
    }

    /// Regression: inserting an event mid-chain must push displaced actions
    /// after the event, not before it, and must not overlap undisplaced actions.
    #[test]
    fn displace_non_static_conflicts_pushes_after_event() {
        let mut pipeline = Pipeline::default();

        for i in 0..5u32 {
            pipeline
                .queue
                .push(QueueItem::Action(make_action((10, i * 5), 5)));
        }

        let now = hm(9, 50);
        let event = make_event((10, 10), 20);
        pipeline.queue_event(event, now);

        let mut action_times: Vec<DateTime<Utc>> = pipeline
            .queue
            .iter()
            .filter_map(|item| {
                if let QueueItem::Action(a) = item {
                    a.target
                } else {
                    None
                }
            })
            .collect();
        action_times.sort();

        assert_eq!(action_times.len(), 5, "all 5 actions must remain in queue");

        let event_start = hm(10, 10);
        let event_end = hm(10, 30);

        for &t in &action_times {
            let end = t + Duration::minutes(5);
            assert!(
                !(t < event_end && end > event_start),
                "action at {} overlaps event [{}, {})",
                t.format("%H:%M"),
                event_start.format("%H:%M"),
                event_end.format("%H:%M"),
            );
        }

        for i in 0..action_times.len() {
            for j in (i + 1)..action_times.len() {
                let a_start = action_times[i];
                let a_end = a_start + Duration::minutes(5);
                let b_start = action_times[j];
                assert!(
                    a_end <= b_start,
                    "actions at {} and {} overlap",
                    a_start.format("%H:%M"),
                    b_start.format("%H:%M"),
                );
            }
        }

        assert_eq!(
            action_times[0],
            hm(10, 0),
            "first action should stay at 10:00"
        );
        assert_eq!(
            action_times[1],
            hm(10, 5),
            "second action should stay at 10:05"
        );

        assert!(
            action_times[2] >= event_end,
            "first displaced action {} must be >= event end {}",
            action_times[2].format("%H:%M"),
            event_end.format("%H:%M"),
        );
    }
}
