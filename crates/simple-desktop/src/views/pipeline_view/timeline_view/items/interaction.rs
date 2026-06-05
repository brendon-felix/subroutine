use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration as ChronoDuration, Local};
use gpui::{AsyncApp, Context, DragMoveEvent, Window};
use simple_core::{Action, ActionState, ActionTarget, AnyItem, Event};
use simple_parser::{parse_action_input, parse_event_input, recurrence_to_rule};
use uuid::Uuid;

use super::super::TimelineView;
use super::{
    ActiveDropState, ActiveResizeState, COMPLETE_CHECKBOX_DURATION, FALLBACK_ITEM_DURATION,
    NavDirection, ResizeDragData, ResizeEdge, SlotLayout, TimelineItem, TransitionState, uf_find,
};
use crate::{components::DragData, stores::AppDatabaseStore};

impl TimelineView {
    pub fn refresh_items(&mut self, queue: Vec<AnyItem>, cx: &mut Context<Self>) {
        let scheduled: Vec<AnyItem> = queue
            .into_iter()
            .filter(|item| item.time().is_some())
            .collect();

        let incoming_ids: HashSet<u64> = scheduled.iter().map(|i| i.truncated_id()).collect();
        let mut removed_ids = Vec::new();
        self.items.retain(|item| {
            let id = item.item.id();
            let keep = incoming_ids.contains(&item.item.truncated_id())
                || self.draft_item_ids.contains(&id);
            if !keep {
                removed_ids.push(item.element_id.clone());
            }
            keep
        });
        for id in removed_ids {
            self.detached_order.retain(|eid| eid != &id);
        }

        for incoming in &scheduled {
            if let Some(existing) = self
                .items
                .iter_mut()
                .find(|i| i.item.truncated_id() == incoming.truncated_id())
            {
                existing.item = incoming.clone();
            } else {
                self.items.push(TimelineItem::new(incoming.clone(), cx));
            }
        }

        self.items
            .sort_by_key(|i| i.item.time().map(|t| t.timestamp()));

        if !self.loaded {
            self.loaded = true;
        }

        let current_ids: HashSet<Uuid> = self.items.iter().map(|i| i.item.id()).collect();
        self.title_inputs.retain(|id, _| current_ids.contains(id));
        self.title_edit_states
            .retain(|id, _| current_ids.contains(id));
        self.editing_items.retain(|id| current_ids.contains(id));

        cx.notify();
    }

    /// Revert a timeline item's title input and edit state back to the last saved value.
    pub(super) fn revert_item(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let saved_title = {
            let store = AppDatabaseStore::global(cx);
            let store = store.read(cx);
            if let Some(action) = store.get_action(id) {
                Some(action.title.clone())
            } else {
                store.get_event(id).map(|e| e.title.clone())
            }
        };
        let Some(title) = saved_title else { return };

        if let Some(input) = self.title_inputs.get(&id) {
            let current = input.read(cx).value().to_string();
            if current != title {
                input.update(cx, |state, cx| {
                    state.set_value(title.clone(), window, cx);
                });
            }
        }
        self.editing_items.remove(&id);
        if let Some(state) = self.title_edit_states.get_mut(&id) {
            state.current_text = title.clone();
            state.draft = parse_action_input(&title).ok();
            state.parse_error = false;
        }
    }

    /// Commit edits for an action item.
    pub(super) fn commit_action(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let store = AppDatabaseStore::global(cx);
        let original = if self.draft_item_ids.contains(&id) {
            match self.items.iter().find(|i| i.item.id() == id) {
                Some(ti) => match &ti.item {
                    AnyItem::Action(a) => a.clone(),
                    _ => return,
                },
                None => return,
            }
        } else {
            match store.read(cx).get_action(id) {
                Some(a) => a.clone(),
                None => return,
            }
        };

        let draft = self
            .title_edit_states
            .get(&id)
            .and_then(|s| s.draft.clone())
            .or_else(|| {
                let text = self
                    .title_inputs
                    .get(&id)
                    .map(|e| e.read(cx).value().to_string())
                    .unwrap_or_default();
                parse_action_input(text.trim()).ok()
            });

        let updated_action = match draft {
            Some(ref d) => {
                let mut a = original.clone();
                a.title = d.title.clone();
                if let Some(ref when) = d.when {
                    use simple_parser::ast::WhenSpec;
                    match when {
                        WhenSpec::DateTime(dt) => {
                            a.queue_static(*dt);
                        }
                        WhenSpec::NaiveDate(date) => {
                            a.backlog(Some(*date));
                        }
                    }
                }
                if let Some(dur) = d.duration {
                    a.duration = Some(dur);
                }
                if let Some(ref rec) = d.recurrence {
                    if let Some(rule) = recurrence_to_rule(Some(rec)) {
                        a.recurrence = Some(rule);
                    }
                }
                if let Some(ref content) = d.content {
                    a.content = Some(content.clone());
                }
                a
            }
            None => {
                let raw = self
                    .title_inputs
                    .get(&id)
                    .map(|e| e.read(cx).value().to_string())
                    .unwrap_or_default();
                let raw = raw.trim();
                if raw.is_empty() {
                    return;
                }
                let mut a = original.clone();
                a.title = raw.to_string();
                a
            }
        };

        self.draft_item_ids.remove(&id);
        store.update(cx, |store, cx| store.upsert_action(updated_action, cx));
        self.editing_items.remove(&id);
        if let Some(item) = self.items.iter().find(|i| i.item.id() == id) {
            item.focus_handle.focus(window, cx);
        }
    }

    /// Commit edits for an event item.
    pub(super) fn commit_event(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let store = AppDatabaseStore::global(cx);
        let original = if self.draft_item_ids.contains(&id) {
            match self.items.iter().find(|i| i.item.id() == id) {
                Some(ti) => match &ti.item {
                    AnyItem::Event(e) => e.clone(),
                    _ => return,
                },
                None => return,
            }
        } else {
            match store.read(cx).get_event(id) {
                Some(e) => e.clone(),
                None => return,
            }
        };

        let draft = self
            .title_edit_states
            .get(&id)
            .and_then(|s| s.draft.clone())
            .or_else(|| {
                let text = self
                    .title_inputs
                    .get(&id)
                    .map(|e| e.read(cx).value().to_string())
                    .unwrap_or_default();
                parse_event_input(text.trim()).ok()
            });

        let updated_event = match draft {
            Some(ref d) => {
                let mut e = original.clone();
                e.title = d.title.clone();
                if let Some(simple_parser::ast::WhenSpec::DateTime(dt)) = d.when {
                    e.time = dt;
                }
                if let Some(dur) = d.duration {
                    e.duration = Some(dur);
                }
                if let Some(ref rec) = d.recurrence {
                    if let Some(rule) = recurrence_to_rule(Some(rec)) {
                        e.recurrence = Some(rule);
                    }
                }
                if let Some(ref content) = d.content {
                    e.content = Some(content.clone());
                }
                e
            }
            None => {
                let raw = self
                    .title_inputs
                    .get(&id)
                    .map(|e| e.read(cx).value().to_string())
                    .unwrap_or_default();
                let raw = raw.trim();
                if raw.is_empty() {
                    return;
                }
                let mut e = original.clone();
                e.title = raw.to_string();
                e
            }
        };

        self.draft_item_ids.remove(&id);
        store.update(cx, |store, cx| store.upsert_event(updated_event, cx));
        self.editing_items.remove(&id);
        if let Some(item) = self.items.iter().find(|i| i.item.id() == id) {
            item.focus_handle.focus(window, cx);
        }
    }

    /// Create a draft action at the given time, add it to the timeline, and enter edit mode.
    pub(crate) fn add_draft_action(
        &mut self,
        time: DateTime<Local>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Uuid {
        use gpui_component::input::Position;
        let time_utc = time.with_timezone(&chrono::Utc);
        let state = ActionState::Queued(ActionTarget {
            time: time_utc,
            is_static: true,
        });
        let action = Action::new("").with_state(state);
        let id = action.id;
        let any_item = AnyItem::Action(action);
        self.items.push(TimelineItem::new(any_item, cx));
        self.items
            .sort_by_key(|i| i.item.time().map(|t| t.timestamp()));
        self.draft_item_ids.insert(id);
        self.editing_items.insert(id);
        let input = self.get_or_create_title_input(id, String::new(), true, window, cx);
        input.update(cx, |state, cx| {
            state.set_cursor_position(Position::new(0, u32::MAX), window, cx);
        });
        cx.notify();
        id
    }

    /// Find the next sequential start time within the given slot.
    ///
    /// Scans all currently loaded items whose slot-floor equals `slot_start`
    /// and returns the maximum of their end times (= item.time + duration),
    /// falling back to `slot_start` itself when the slot is empty.
    ///
    /// The returned time is where a new item should be inserted to follow the
    /// existing queue without overlapping any of them.
    fn find_next_slot_time(&self, slot_start: DateTime<Local>) -> DateTime<Local> {
        let state = self.current_division_state();
        let base = state.base_division;
        let sub = state.current_subdivision();

        let mut cursor = slot_start;
        for ti in &self.items {
            let Some(item_time) = ti.item.time_local() else {
                continue;
            };
            let item_slot = sub
                .map(|s| s.floor_boundary(item_time))
                .unwrap_or_else(|| base.floor_boundary(item_time));
            if item_slot == slot_start {
                let end = item_time + ti.item.duration().unwrap_or(FALLBACK_ITEM_DURATION);
                if end > cursor {
                    cursor = end;
                }
            }
        }
        cursor
    }

    /// Double-click creation: place a new draft action at the next sequential
    /// position within the slot, then push any conflicting persisted actions
    /// forward to make room.
    pub(crate) fn add_draft_action_in_slot(
        &mut self,
        slot_start: DateTime<Local>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let placement = self.find_next_slot_time(slot_start);
        let placement_utc = placement.with_timezone(&chrono::Utc);
        let id = self.add_draft_action(placement, window, cx);
        // Push any persisted actions that now overlap with the draft's intended
        // time slot forward so there is no conflict when it is committed.
        self.push_conflicting_actions(id, placement_utc, FALLBACK_ITEM_DURATION, cx);
    }

    /// Create a draft event at the given time, add it to the timeline, and enter edit mode.
    pub(crate) fn add_draft_event(
        &mut self,
        time: DateTime<Local>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use gpui_component::input::Position;
        let time_utc = time.with_timezone(&chrono::Utc);
        let event = Event::new("", time_utc);
        let id = event.id;
        let any_item = AnyItem::Event(event);
        self.items.push(TimelineItem::new(any_item, cx));
        self.items
            .sort_by_key(|i| i.item.time().map(|t| t.timestamp()));
        self.draft_item_ids.insert(id);
        self.editing_items.insert(id);
        let input = self.get_or_create_title_input(id, String::new(), false, window, cx);
        input.update(cx, |state, cx| {
            state.set_cursor_position(Position::new(0, u32::MAX), window, cx);
        });
        cx.notify();
    }

    pub(super) fn begin_complete_item(
        &mut self,
        action_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next_focus = {
            let pos = self.items.iter().position(|i| i.item.id() == action_id);
            pos.and_then(|p| {
                self.items
                    .get(p + 1)
                    .or_else(|| if p > 0 { self.items.get(p - 1) } else { None })
                    .map(|i| i.focus_handle.clone())
            })
        };
        if let Some(handle) = next_focus {
            handle.focus(window, cx);
        } else {
            self.focus_handle.focus(window, cx);
        }

        if let Some(item) = self.items.iter_mut().find(|i| i.item.id() == action_id) {
            item.transition_state = TransitionState::Completing;
        }
        cx.notify();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            cx.background_executor()
                .timer(COMPLETE_CHECKBOX_DURATION)
                .await;
            let _ = this.update(cx, |_view, cx| {
                let store = AppDatabaseStore::global(cx);
                store.update(cx, |store, cx| {
                    store.complete_action(action_id, cx);
                });
            });
        })
        .detach();
    }

    /// Compute slot-based layout for every item in `items`.
    /// Returns one `Option<SlotLayout>` per item; `None` for items without a scheduled time.
    pub(super) fn compute_slot_layouts(&self, items: &[AnyItem]) -> Vec<Option<SlotLayout>> {
        let state = self.current_division_state();
        let base = state.base_division;
        let sub = state.current_subdivision();
        let n = items.len();

        let spans: Vec<Option<(DateTime<Local>, DateTime<Local>)>> = items
            .iter()
            .map(|item| {
                let time = item.time_local()?;
                let vs = sub
                    .map(|s| s.floor_boundary(time))
                    .unwrap_or_else(|| base.floor_boundary(time));
                let slot_dur = sub
                    .map(|s| s.exact_duration(vs))
                    .unwrap_or_else(|| base.exact_duration(vs));
                let ve = if let Some(duration) = item.duration() {
                    let actual_end = time + duration;
                    let ve_raw = sub
                        .map(|s| s.ceil_boundary(actual_end))
                        .unwrap_or_else(|| base.ceil_boundary(actual_end));
                    if ve_raw <= vs { vs + slot_dur } else { ve_raw }
                } else {
                    vs + slot_dur
                };
                Some((vs, ve))
            })
            .collect();

        let mut parent: Vec<usize> = (0..n).collect();
        for i in 0..n {
            let Some((s_i, e_i)) = spans[i] else {
                continue;
            };
            for j in (i + 1)..n {
                let Some((s_j, e_j)) = spans[j] else {
                    continue;
                };
                if s_i < e_j && s_j < e_i {
                    let ri = uf_find(&mut parent, i);
                    let rj = uf_find(&mut parent, j);
                    if ri != rj {
                        parent[ri] = rj;
                    }
                }
            }
        }

        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            if spans[i].is_none() {
                continue;
            }
            let root = uf_find(&mut parent, i);
            groups.entry(root).or_default().push(i);
        }

        let mut result: Vec<Option<SlotLayout>> = (0..n).map(|_| None).collect();
        for group in groups.values() {
            let mut sorted = group.clone();
            sorted.sort_by_key(|&i| {
                let (vs, _) = spans[i].unwrap();
                (vs, items[i].time_local().unwrap_or(vs))
            });

            let mut col_ends: Vec<DateTime<Local>> = Vec::new();
            let mut assignments: Vec<usize> = vec![0; sorted.len()];

            for (g_idx, &item_idx) in sorted.iter().enumerate() {
                let (vs, ve) = spans[item_idx].unwrap();
                let col = match col_ends.iter().position(|&end| end <= vs) {
                    Some(c) => {
                        col_ends[c] = ve;
                        c
                    }
                    None => {
                        col_ends.push(ve);
                        col_ends.len() - 1
                    }
                };
                assignments[g_idx] = col;
            }

            let total_columns = col_ends.len();
            for (g_idx, &item_idx) in sorted.iter().enumerate() {
                let (visual_start, visual_end) = spans[item_idx].unwrap();
                result[item_idx] = Some(SlotLayout {
                    visual_start,
                    visual_end,
                    column_index: assignments[g_idx],
                    total_columns,
                });
            }
        }

        result
    }

    pub(crate) fn navigate_items(
        &mut self,
        dir: NavDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focused_idx = self
            .items
            .iter()
            .position(|i| i.focus_handle.is_focused(window));
        let view_focused = self.focus_handle.is_focused(window);
        if !view_focused && focused_idx.is_none() {
            return;
        }

        let any_items: Vec<AnyItem> = self.items.iter().map(|i| i.item.clone()).collect();
        let layouts = self.compute_slot_layouts(&any_items);

        let col_frac = |col: usize, total: usize| col as f32 / total as f32;

        let Some(focused_idx) = focused_idx else {
            let target = match dir {
                NavDirection::Down | NavDirection::Right => {
                    layouts.iter().enumerate().find_map(|(i, l)| l.map(|_| i))
                }
                NavDirection::Up | NavDirection::Left => layouts
                    .iter()
                    .enumerate()
                    .rev()
                    .find_map(|(i, l)| l.map(|_| i)),
            };
            if let Some(idx) = target {
                if let Some(layout) = layouts[idx] {
                    self.target_column_fraction =
                        Some(col_frac(layout.column_index, layout.total_columns));
                }
                self.items[idx].focus_handle.focus(window, cx);
            }
            return;
        };

        let Some(focused_layout) = layouts[focused_idx] else {
            return;
        };

        let target_frac: f32 = match dir {
            NavDirection::Up | NavDirection::Down => {
                self.target_column_fraction.unwrap_or_else(|| {
                    col_frac(focused_layout.column_index, focused_layout.total_columns)
                })
            }
            NavDirection::Left | NavDirection::Right => {
                col_frac(focused_layout.column_index, focused_layout.total_columns)
            }
        };

        let target_idx: Option<usize> = match dir {
            NavDirection::Down => {
                let s = focused_layout.visual_start;
                layouts
                    .iter()
                    .enumerate()
                    .filter_map(|(i, l)| l.map(|l| (i, l)))
                    .filter(|(i, l)| *i != focused_idx && l.visual_start > s)
                    .min_by(|(_, la), (_, lb)| {
                        la.visual_start.cmp(&lb.visual_start).then_with(|| {
                            let da =
                                (col_frac(la.column_index, la.total_columns) - target_frac).abs();
                            let db =
                                (col_frac(lb.column_index, lb.total_columns) - target_frac).abs();
                            da.total_cmp(&db)
                        })
                    })
                    .map(|(i, _)| i)
            }
            NavDirection::Up => {
                let s = focused_layout.visual_start;
                layouts
                    .iter()
                    .enumerate()
                    .filter_map(|(i, l)| l.map(|l| (i, l)))
                    .filter(|(i, l)| *i != focused_idx && l.visual_start < s)
                    .max_by(|(_, la), (_, lb)| {
                        la.visual_start.cmp(&lb.visual_start).then_with(|| {
                            let da =
                                (col_frac(la.column_index, la.total_columns) - target_frac).abs();
                            let db =
                                (col_frac(lb.column_index, lb.total_columns) - target_frac).abs();
                            db.total_cmp(&da)
                        })
                    })
                    .map(|(i, _)| i)
            }
            NavDirection::Left => {
                let s = focused_layout.visual_start;
                let e = focused_layout.visual_end;
                let col = focused_layout.column_index;
                if col == 0 {
                    return;
                }
                layouts
                    .iter()
                    .enumerate()
                    .filter_map(|(i, l)| l.map(|l| (i, l)))
                    .filter(|(i, l)| {
                        *i != focused_idx
                            && l.column_index == col - 1
                            && l.visual_start < e
                            && s < l.visual_end
                    })
                    .min_by_key(|(_, l)| (l.visual_start - s).num_seconds().unsigned_abs())
                    .map(|(i, _)| i)
            }
            NavDirection::Right => {
                let s = focused_layout.visual_start;
                let e = focused_layout.visual_end;
                let col = focused_layout.column_index;
                layouts
                    .iter()
                    .enumerate()
                    .filter_map(|(i, l)| l.map(|l| (i, l)))
                    .filter(|(i, l)| {
                        *i != focused_idx
                            && l.column_index == col + 1
                            && l.visual_start < e
                            && s < l.visual_end
                    })
                    .min_by_key(|(_, l)| (l.visual_start - s).num_seconds().unsigned_abs())
                    .map(|(i, _)| i)
            }
        };

        if let Some(idx) = target_idx {
            if matches!(dir, NavDirection::Left | NavDirection::Right) {
                if let Some(layout) = layouts[idx] {
                    self.target_column_fraction =
                        Some(col_frac(layout.column_index, layout.total_columns));
                }
            }
            self.items[idx].focus_handle.focus(window, cx);
        }
    }

    pub(crate) fn handle_drag_move(
        &mut self,
        event: &DragMoveEvent<DragData<AnyItem>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mouse_pos = window.mouse_position();
        let local_pos = self.bounds.and_then(|b| b.localize(&mouse_pos));

        let new_drop = local_pos.map(|pos| {
            let raw_time = self.position_to_time(pos);
            let state = self.current_division_state();
            let base = state.base_division;
            let sub = state.current_subdivision();
            let slot_start = sub
                .map(|s| s.floor_boundary(raw_time))
                .unwrap_or_else(|| base.floor_boundary(raw_time));
            let slot_dur = sub
                .map(|s| s.exact_duration(slot_start))
                .unwrap_or_else(|| base.exact_duration(slot_start));
            let slot_end = slot_start + slot_dur;
            let drop_duration = event.drag(cx).data.duration();

            let mut items_in_slot: Vec<(DateTime<Local>, ChronoDuration)> = self
                .items
                .iter()
                .filter_map(|ti| {
                    let time = ti.item.time_local()?;
                    let time_slot = sub
                        .map(|s| s.floor_boundary(time))
                        .unwrap_or_else(|| base.floor_boundary(time));
                    if time_slot == slot_start {
                        let dur = ti.item.duration().unwrap_or(FALLBACK_ITEM_DURATION);
                        Some((time, dur))
                    } else {
                        None
                    }
                })
                .collect();
            items_in_slot.sort_by_key(|(t, _)| *t);

            let existing_count = items_in_slot.len();
            let total_columns = existing_count + 1;

            use super::ATTACHED_ITEM_LEFT;
            let area_w = self.item_area_width();
            let col_w = area_w / total_columns as f32;
            let item_relative_x = pos.x - ATTACHED_ITEM_LEFT;
            let column_index = ((item_relative_x / col_w).floor() as usize)
                .clamp(0, total_columns.saturating_sub(1));

            let drop_time = if column_index == 0 || items_in_slot.is_empty() {
                slot_start
            } else {
                let (prev_t, prev_d) = items_in_slot[column_index - 1];
                prev_t + prev_d
            };

            ActiveDropState {
                drop_time,
                drop_duration,
                slot_visual_start: slot_start,
                slot_visual_end: slot_end,
                column_index,
                total_columns,
            }
        });

        if new_drop != self.active_drop {
            self.active_drop = new_drop;
            cx.notify();
        }

        let new_speed = self.bounds.and_then(|b| {
            let local_y = local_pos.map(|p| p.y)?;
            Self::compute_edge_scroll_speed(local_y, b.size.height)
        });
        if new_speed != self.edge_scroll_speed {
            self.edge_scroll_speed = new_speed;
            cx.notify();
        }
    }

    pub(crate) fn handle_drop(
        &mut self,
        data: &DragData<AnyItem>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edge_scroll_speed = None;
        let Some(drop_info) = self.active_drop.take() else {
            cx.notify();
            return;
        };
        cx.notify();

        let item = data.data.clone();
        let drop_time = drop_info.drop_time;
        let drop_time_utc = drop_time.with_timezone(&chrono::Utc);

        let store = AppDatabaseStore::global(cx);
        match item {
            AnyItem::Action(mut action) => {
                let action_duration = action.duration.unwrap_or(FALLBACK_ITEM_DURATION);
                action.queue_static(drop_time_utc);
                let action_id = action.id;
                store.update(cx, |s, cx| {
                    s.upsert_action(action, cx);
                });
                self.push_conflicting_actions(action_id, drop_time_utc, action_duration, cx);
            }
            AnyItem::Event(mut event) => {
                event.time = drop_time_utc;
                store.update(cx, |s, cx| {
                    s.upsert_event(event, cx);
                });
            }
        }
    }

    fn push_conflicting_actions(
        &mut self,
        skip_id: Uuid,
        drop_start: chrono::DateTime<chrono::Utc>,
        drop_duration: ChronoDuration,
        cx: &mut Context<Self>,
    ) {
        let store = AppDatabaseStore::global(cx);

        let mut actions: Vec<simple_core::Action> = store
            .read(cx)
            .actions()
            .into_iter()
            .filter(|a| a.id != skip_id && a.is_queued_static())
            .collect();

        actions.sort_by_key(|a| match a.state {
            ActionState::Queued(t) => t.time,
            _ => unreachable!(),
        });

        let mut cursor = drop_start + drop_duration;
        let mut updated: Vec<simple_core::Action> = Vec::new();

        for mut action in actions {
            let ActionState::Queued(target) = action.state else {
                continue;
            };
            let action_dur = action.duration.unwrap_or(FALLBACK_ITEM_DURATION);
            let action_end = target.time + action_dur;

            if target.time < cursor && action_end > drop_start {
                action.queue(cursor);
                cursor = cursor + action_dur;
                updated.push(action);
            } else if target.time >= cursor {
                break;
            }
        }

        if !updated.is_empty() {
            store.update(cx, |s, cx| {
                for action in updated {
                    s.upsert_action(action, cx);
                }
            });
        }
    }

    pub(crate) fn handle_resize_move(
        &mut self,
        event: &DragMoveEvent<ResizeDragData>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mouse_pos = window.mouse_position();
        let local_pos = self.bounds.and_then(|b| b.localize(&mouse_pos));

        let Some(local_pos) = local_pos else {
            if self.active_resize.is_some() || self.edge_scroll_speed.is_some() {
                self.active_resize = None;
                self.edge_scroll_speed = None;
                cx.notify();
            }
            return;
        };

        let data = event.drag(cx);
        let item_id = data.item_id;
        let edge = data.edge;

        let Some(ti) = self.items.iter().find(|i| i.item.id() == item_id) else {
            return;
        };
        let Some(original_time) = ti.item.time_local() else {
            return;
        };
        let original_duration = ti.item.duration().unwrap_or(FALLBACK_ITEM_DURATION);
        let original_end = original_time + original_duration;

        let raw_time = self.position_to_time(local_pos);
        let state = self.current_division_state();
        let base = state.base_division;
        let sub = state.current_subdivision();
        let snapped = sub
            .map(|s| s.floor_boundary(raw_time))
            .unwrap_or_else(|| base.floor_boundary(raw_time));
        let min_duration = sub
            .map(|s| s.exact_duration(raw_time))
            .unwrap_or_else(|| base.exact_duration(raw_time));

        let (new_time, new_end) = match edge {
            ResizeEdge::Top => {
                let clamped = snapped.min(original_end - min_duration);
                (clamped, original_end)
            }
            ResizeEdge::Bottom => {
                let clamped = snapped.max(original_time + min_duration);
                (original_time, clamped)
            }
        };

        let new_info = ActiveResizeState {
            item_id,
            edge,
            original_time,
            original_end,
            new_time,
            new_end,
        };

        if self.active_resize.as_ref() != Some(&new_info) {
            self.active_resize = Some(new_info);
            cx.notify();
        }

        let new_speed = self
            .bounds
            .map(|b| Self::compute_edge_scroll_speed(local_pos.y, b.size.height))
            .flatten();
        if new_speed != self.edge_scroll_speed {
            self.edge_scroll_speed = new_speed;
            cx.notify();
        }
    }

    /// Commits the active resize to the store and clears all related state.
    /// Called from the paint-time `MouseUpEvent` handler so it fires regardless
    /// of whether the cursor is over the timeline when the button is released.
    pub(crate) fn commit_resize_state(&mut self, cx: &mut Context<Self>) {
        self.edge_scroll_speed = None;
        let Some(resize) = self.active_resize.take() else {
            return;
        };
        cx.notify();

        let new_duration = resize.new_end - resize.new_time;
        let new_time_utc = resize.new_time.with_timezone(&chrono::Utc);

        let Some(ti) = self.items.iter().find(|i| i.item.id() == resize.item_id) else {
            return;
        };
        let store = AppDatabaseStore::global(cx);
        match ti.item.clone() {
            AnyItem::Action(mut action) => {
                action.queue_static(new_time_utc);
                action.duration = Some(new_duration);
                store.update(cx, |s, cx| s.upsert_action(action, cx));
            }
            AnyItem::Event(mut event) => {
                event.time = new_time_utc;
                event.duration = Some(new_duration);
                store.update(cx, |s, cx| s.upsert_event(event, cx));
            }
        }
    }
}
