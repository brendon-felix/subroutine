use chrono::{DateTime, Duration, Utc};

use crate::Action;

use super::{
    CONSECUTIVE_GAP_THRESHOLD, EXPEDITE_HORIZON, OverlapWarning, QueueItem,
    SEMI_CONSECUTIVE_GAP_THRESHOLD,
    schedule::{
        action_effective_duration, all_anchor_intervals, compact_cluster,
        compacted_cluster_duration, event_intervals, find_free_slot, group_by_gap,
        group_consecutive,
    },
};

pub(super) fn next_available_slot(
    queue: &[QueueItem],
    now: DateTime<Utc>,
    duration: Duration,
) -> DateTime<Utc> {
    let mut occupied: Vec<(DateTime<Utc>, DateTime<Utc>)> = queue
        .iter()
        .filter_map(|item| match item {
            QueueItem::Event(e) => Some((e.time, e.end_time())),
            QueueItem::Action(a) => {
                let t = a.target?;
                Some((t, t + action_effective_duration(a)))
            }
        })
        .filter(|(_, end)| *end > now)
        .collect();
    occupied.sort_by_key(|(start, _)| *start);

    find_free_slot(now, duration, &occupied)
}

pub(super) fn check_static_overlaps(
    queue: &[QueueItem],
    inserted_title: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Vec<OverlapWarning> {
    queue
        .iter()
        .filter_map(|item| {
            let (item_start, item_end, item_title) = match item {
                QueueItem::Event(e) => (e.time, e.end_time(), e.title.as_str()),
                QueueItem::Action(a) if a.target_static => {
                    let t = a.target?;
                    (t, t + action_effective_duration(a), a.title.as_str())
                }
                _ => return None,
            };
            if start < item_end && end > item_start {
                Some(OverlapWarning {
                    inserted_title: inserted_title.to_string(),
                    conflicting_title: item_title.to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn collect_trailing_chain(
    queue: &mut Vec<QueueItem>,
    anchor_end: DateTime<Utc>,
) -> Vec<Action> {
    let mut trailing: Vec<Action> = Vec::new();

    queue.retain(|item| {
        let QueueItem::Action(action) = item else {
            return true;
        };
        if action.target_static {
            return true;
        }
        let Some(target) = action.target else {
            return true;
        };
        let gap = target - anchor_end;
        if gap >= Duration::zero() && gap <= CONSECUTIVE_GAP_THRESHOLD {
            trailing.push(action.clone());
            false
        } else {
            true
        }
    });

    trailing.sort_by_key(|a| a.target.map(|t| t.timestamp()).unwrap_or(i64::MAX));
    trailing
}

pub(super) fn reanchor_trailing_chain(
    queue: &mut Vec<QueueItem>,
    trailing: Vec<Action>,
    new_anchor_end: DateTime<Utc>,
) {
    if trailing.is_empty() {
        return;
    }

    let anchors = all_anchor_intervals(queue);
    let total_duration: Duration = trailing
        .iter()
        .map(action_effective_duration)
        .fold(Duration::zero(), |acc, d| acc + d);
    let slot_start = find_free_slot(new_anchor_end, total_duration, &anchors);
    let mut cursor = slot_start;

    for mut action in trailing {
        action.target = Some(cursor);
        cursor = cursor + action_effective_duration(&action);
        queue.push(QueueItem::Action(action));
    }
}

pub(super) fn displace_non_static_conflicts(queue: &mut Vec<QueueItem>, now: DateTime<Utc>) {
    let all_anchors = all_anchor_intervals(queue);

    let mut displaced: Vec<Action> = Vec::new();
    let mut last_displacing_anchor_end = now;

    queue.retain(|item| {
        let QueueItem::Action(action) = item else {
            return true;
        };
        if action.target_static {
            return true;
        }
        let Some(target) = action.target else {
            return true;
        };
        let end = target + action_effective_duration(action);
        let conflicting_anchor_end = all_anchors
            .iter()
            .filter(|(a_start, a_end)| target < *a_end && end > *a_start)
            .map(|(_, a_end)| *a_end)
            .max();
        if let Some(anchor_end) = conflicting_anchor_end {
            if anchor_end > last_displacing_anchor_end {
                last_displacing_anchor_end = anchor_end;
            }
            displaced.push(action.clone());
            false
        } else {
            true
        }
    });

    if displaced.is_empty() {
        return;
    }

    displaced.sort_by_key(|a| a.target.map(|t| t.timestamp()).unwrap_or(i64::MAX));

    let groups = group_consecutive(displaced);

    let mut occupied: Vec<(DateTime<Utc>, DateTime<Utc>)> = queue
        .iter()
        .filter_map(|item| match item {
            QueueItem::Event(e) => Some((e.time, e.end_time())),
            QueueItem::Action(a) => {
                let t = a.target?;
                Some((t, t + action_effective_duration(a)))
            }
        })
        .collect();
    occupied.sort_by_key(|(start, _)| *start);

    let mut cursor = last_displacing_anchor_end;

    for group in groups {
        let total_duration = group
            .iter()
            .map(action_effective_duration)
            .fold(Duration::zero(), |acc, d| acc + d);

        let slot_start = find_free_slot(cursor, total_duration, &occupied);
        let mut group_cursor = slot_start;

        for (i, mut action) in group.into_iter().enumerate() {
            let new_target = if i == 0 { slot_start } else { group_cursor };
            group_cursor = new_target + action_effective_duration(&action);
            action.target = Some(new_target);
            occupied.push((new_target, group_cursor));
            queue.push(QueueItem::Action(action));
        }
        occupied.sort_by_key(|(start, _)| *start);

        cursor = slot_start + total_duration;
    }
}

pub(super) fn expedite_queue(queue: &mut Vec<QueueItem>, now: DateTime<Utc>) {
    let horizon = now + EXPEDITE_HORIZON;

    let mut to_expedite: Vec<Action> = Vec::new();

    queue.retain(|item| match item {
        QueueItem::Action(action) if !action.target_static => {
            let within_horizon = match action.target {
                Some(target) => target <= horizon,
                None => true,
            };
            if within_horizon {
                to_expedite.push(action.clone());
                false
            } else {
                true
            }
        }
        _ => true,
    });

    if to_expedite.is_empty() {
        return;
    }

    let semi_clusters = group_by_gap(to_expedite, SEMI_CONSECUTIVE_GAP_THRESHOLD);
    let intervals = event_intervals(queue);
    let mut cursor = now;

    for cluster in semi_clusters {
        let compacted_duration = compacted_cluster_duration(&cluster);
        let slot_start = find_free_slot(cursor, compacted_duration, &intervals);
        let compacted = compact_cluster(cluster, slot_start);

        cursor = slot_start + compacted_duration;

        for action in compacted {
            queue.push(QueueItem::Action(action));
        }
    }
}
