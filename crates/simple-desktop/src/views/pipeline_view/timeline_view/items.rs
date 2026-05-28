use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, Local, Timelike};
use gpui::{
    AnyElement, App, AppContext, AsyncApp, ClickEvent, Context, DragMoveEvent, ElementId, Entity,
    FocusHandle, Focusable, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, animation::ease_out_cubic, checkbox::Checkbox, h_flex, label::Label,
    menu::ContextMenuExt, skeleton::Skeleton,
};
use gpui_component::{
    Colorize,
    input::{Input, InputEvent, InputState, Position},
};
use gpui_transitions::{Lerp, Transition, WindowUseTransition};
use simple_core::{ActionState, AnyItem};
/// Fallback duration used when an item has no explicit duration (5 min).
const FALLBACK_ITEM_DURATION: ChronoDuration = ChronoDuration::minutes(5);
use simple_parser::{ParseDraft, parse_action_input, parse_event_input, recurrence_to_rule};
use uuid::Uuid;

use super::TimelineView;
use crate::{
    components::{DragData, Draggable},
    stores::AppDatabaseStore,
    utils::{ButtonColorizeExt, ButtonColors},
    views::pipeline_view::timeline_view::timeline::HOUR_DIVIDER_HEIGHT,
};

const RESCHEDULE_TRANSITION_DURATION: Duration = Duration::from_millis(150);
const COMPLETE_CHECKBOX_DURATION: Duration = Duration::from_millis(200);
const ATTACHED_ITEM_LEFT: Pixels = px(16. * 4.);
const ITEMS_RIGHT_GAP: Pixels = px(16. * 4.);
const SLOT_GAP: Pixels = px(3.);
const STICKY_TITLE_HEIGHT: Pixels = px(28.);
const STICKY_TITLE_PADDING: Pixels = px(2.);

/// Animatable representation of an attached item's layout in timeline-relative,
/// viewport-independent coordinates.
#[derive(Clone, Copy, PartialEq)]
struct ItemTimelineBounds {
    /// Seconds from `TimelineView::start` to the item's visual slot start.
    elapsed_secs: f64,
    /// Visual duration of the slot in seconds (used to compute height).
    duration_secs: f64,
    /// Left edge of the column as a fraction of the item-area width [0, 1).
    left_fraction: f32,
    /// Width of the column as a fraction of the item-area width (0, 1].
    width_fraction: f32,
}

impl Lerp for ItemTimelineBounds {
    fn lerp(&self, to: &Self, delta: f32) -> Self {
        Self {
            elapsed_secs: self.elapsed_secs.lerp(&to.elapsed_secs, delta),
            duration_secs: self.duration_secs.lerp(&to.duration_secs, delta),
            left_fraction: self.left_fraction.lerp(&to.left_fraction, delta),
            width_fraction: self.width_fraction.lerp(&to.width_fraction, delta),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum TransitionState {
    Attached,
    // Detached,
    // Reattaching,
    // Detaching,
    Completing,
    // Dragging,
}

pub(super) struct TimelineItem {
    focus_handle: FocusHandle,
    element_id: ElementId,
    item: AnyItem,
    transition_state: TransitionState,
}

impl Focusable for TimelineItem {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TimelineItem {
    fn new(item: AnyItem, cx: &App) -> Self {
        let focus_handle = cx.focus_handle();
        let element_id = ("timeline-item", item.truncated_id()).into();
        let transition_state = TransitionState::Attached;
        Self {
            focus_handle,
            element_id,
            item,
            transition_state,
        }
    }

    // fn id(&self) -> Uuid {
    //     self.item.id()
    // }

    // fn title(&self) -> SharedString {
    //     self.item.title().into()
    // }

    // fn duration(&self) -> Option<ChronoDuration> {
    //     self.item.duration()
    // }

    fn colors(&self, cx: &App) -> ButtonColors {
        match self.item {
            AnyItem::Action(_) => ButtonColors::normal(cx.theme().button_primary, cx),
            AnyItem::Event(_) => ButtonColors::normal(
                cx.theme()
                    .button_primary
                    .mix_oklab(cx.theme().foreground, 0.5),
                cx,
            ),
        }
    }
}

/// Tracks the live editing state for a single timeline item's title input.
pub(super) struct TitleEditState {
    current_text: String,
    draft: Option<ParseDraft>,
    /// True when the user has typed something that doesn't parse cleanly.
    parse_error: bool,
}

impl TitleEditState {
    fn new(initial: String) -> Self {
        let draft = parse_action_input(&initial).ok();
        Self {
            current_text: initial,
            draft,
            parse_error: false,
        }
    }

    fn update_action(&mut self, text: String) {
        self.current_text = text.trim().to_string();
        match parse_action_input(&self.current_text) {
            Ok(d) => {
                self.draft = Some(d);
                self.parse_error = false;
            }
            Err(_) => {
                self.draft = None;
                self.parse_error = !self.current_text.is_empty();
            }
        }
    }

    fn update_event(&mut self, text: String) {
        self.current_text = text.trim().to_string();
        match parse_event_input(&self.current_text) {
            Ok(d) => {
                self.draft = Some(d);
                self.parse_error = false;
            }
            Err(_) => {
                self.draft = None;
                self.parse_error = !self.current_text.is_empty();
            }
        }
    }
}

///
/// Renders a drag-preview chip that matches the timeline item's appearance.
/// All inputs are plain values so the closure can be `'static`.
fn render_item_preview(
    colors: ButtonColors,
    title: SharedString,
    item_w: Pixels,
    item_h: Pixels,
    muted_fg: Hsla,
) -> impl IntoElement {
    use gpui_component::label::Label;
    // let too_short = item_h < DEFAULT_ITEM_HEIGHT;
    let too_short = false;
    h_flex()
        .w(item_w)
        .h(item_h)
        .px_2()
        .gap_2()
        .py_0p5()
        .rounded_lg()
        .border_1()
        .bg(colors.bg)
        .when_some(colors.border, |this, c| this.border_color(c))
        .when(!too_short, |this| {
            this.child(Label::new(title).text_sm().text_color(muted_fg))
        })
}

fn attach_transition<T: Lerp + Clone + PartialEq + 'static>(
    id: impl Into<ElementId>,
    init: T,
    duration: Duration,
    window: &mut Window,
    cx: &mut App,
) -> Transition<T> {
    window
        .use_keyed_transition(id, cx, duration, |_, _| init.clone())
        .with_easing(ease_out_cubic)
}

/// Layout information computed for a single attached item in slot-based rendering.
struct SlotLayout {
    visual_start: DateTime<Local>,
    visual_end: DateTime<Local>,
    column_index: usize,
    total_columns: usize,
}

/// Iterative union-find with path compression.
fn uf_find(parent: &mut Vec<usize>, i: usize) -> usize {
    let mut root = i;
    while parent[root] != root {
        root = parent[root];
    }
    let mut node = i;
    while node != root {
        let next = parent[node];
        parent[node] = root;
        node = next;
    }
    root
}

/// Carries all information needed to render and commit a pending drop.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct ActiveDropInfo {
    /// The time at which the dragged item will be scheduled.
    pub drop_time: DateTime<Local>,
    /// The explicit duration of the dragged item, or `None` if the item has no duration.
    /// Used to size the outline accurately; `None` renders as a single slot height.
    pub drop_duration: Option<ChronoDuration>,
    /// Visual start of the slot the item will be appended into.
    pub slot_visual_start: DateTime<Local>,
    /// Visual end of the slot (= slot_visual_start + current division).
    pub slot_visual_end: DateTime<Local>,
    /// Column index the drop indicator occupies (= number of existing items in the slot).
    pub column_index: usize,
    /// Total columns including the drop indicator itself.
    pub total_columns: usize,
}

impl TimelineView {
    /// Width of the item area for the current frame — fills all available space
    /// to the right of `ATTACHED_ITEM_LEFT` (minus `ITEMS_RIGHT_GAP`).
    fn item_area_width(&self) -> Pixels {
        if let Some(bounds) = self.bounds {
            let available = bounds.size.width - ATTACHED_ITEM_LEFT - ITEMS_RIGHT_GAP;
            if available > px(0.) {
                return available;
            }
        }
        // Bounds not yet captured (first frame) — return a placeholder.
        px(400.)
    }

    pub fn refresh_items(&mut self, queue: Vec<AnyItem>, cx: &mut Context<Self>) {
        let scheduled: Vec<AnyItem> = queue
            .into_iter()
            .filter(|item| item.time().is_some())
            .collect();

        // Drop items no longer in the queue, keeping detached_order in sync.
        let incoming_ids: std::collections::HashSet<u64> =
            scheduled.iter().map(|i| i.truncated_id()).collect();
        let mut removed_ids = Vec::new();
        self.items.retain(|item| {
            let keep = incoming_ids.contains(&item.item.truncated_id());
            if !keep {
                removed_ids.push(item.element_id.clone());
            }
            keep
        });
        for id in removed_ids {
            self.detached_order.retain(|eid| eid != &id);
        }

        // Update existing items and insert new ones.
        for incoming in &scheduled {
            if let Some(existing) = self
                .items
                .iter_mut()
                .find(|i| i.item.truncated_id() == incoming.truncated_id())
            {
                // Preserve transition state — just refresh the underlying data.
                existing.item = incoming.clone();
            } else {
                self.items.push(TimelineItem::new(incoming.clone(), cx));
            }
        }

        // Keep items sorted by scheduled time.
        self.items
            .sort_by_key(|i| i.item.time().map(|t| t.timestamp()));

        if !self.loaded {
            self.loaded = true;
        }

        // Sync title inputs and edit states with the current item list.
        let current_ids: HashSet<Uuid> = self.items.iter().map(|i| i.item.id()).collect();
        self.title_inputs.retain(|id, _| current_ids.contains(id));
        self.title_edit_states
            .retain(|id, _| current_ids.contains(id));
        self.editing_items.retain(|id| current_ids.contains(id));

        cx.notify();
    }

    pub(super) fn render_skeleton_items(&self) -> Vec<impl IntoElement> {
        let now = Local::now();
        let hour_start = now
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap();

        let h = self.duration_to_height(chrono::Duration::minutes(30));
        (0..2)
            .map(|i| {
                let time = hour_start + chrono::Duration::hours(2) * i;
                let y = self.time_to_offset(time);
                div()
                    .absolute()
                    .top(y)
                    .w_full()
                    .h(h)
                    .pl_16()
                    .child(Skeleton::new().w_64().h_full().rounded_lg().opacity(0.7))
            })
            .collect()
    }

    /// Revert a timeline item's title input and edit state back to the last saved value.
    fn revert_item(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
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
    fn commit_action(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let store = AppDatabaseStore::global(cx);
        let original = match store.read(cx).get_action(id) {
            Some(a) => a.clone(),
            None => return,
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

        store.update(cx, |store, cx| store.upsert_action(updated_action, cx));
        window.blur();
    }

    /// Commit edits for an event item.
    fn commit_event(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let store = AppDatabaseStore::global(cx);
        let original = match store.read(cx).get_event(id) {
            Some(e) => e.clone(),
            None => return,
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

        store.update(cx, |store, cx| store.upsert_event(updated_event, cx));
        window.blur();
    }

    /// Get or create a title input for the given item, subscribing for change/enter/blur events.
    fn get_or_create_title_input(
        &mut self,
        id: Uuid,
        title: String,
        is_action: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        // Ensure edit state exists.
        self.title_edit_states
            .entry(id)
            .or_insert_with(|| TitleEditState::new(title.clone()));

        // Return existing input if already created.
        if let Some(existing) = self.title_inputs.get(&id) {
            return existing.clone();
        }

        // Create new InputState.
        let input = cx.new(|cx| InputState::new(window, cx).default_value(title));
        self.title_inputs.insert(id, input.clone());

        // Subscribe: Change → update parse state.
        self._title_subscriptions.push(cx.subscribe(
            &input,
            move |this, _, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    let value = this
                        .title_inputs
                        .get(&id)
                        .map(|e| e.read(cx).value().to_string())
                        .unwrap_or_default();
                    if is_action {
                        if let Some(state) = this.title_edit_states.get_mut(&id) {
                            state.update_action(value);
                        }
                    } else if let Some(state) = this.title_edit_states.get_mut(&id) {
                        state.update_event(value);
                    }
                }
            },
        ));

        // Subscribe: PressEnter → commit.
        self._title_subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    if is_action {
                        this.commit_action(id, window, cx);
                    } else {
                        this.commit_event(id, window, cx);
                    }
                }
            },
        ));

        // Subscribe: Blur → revert and exit editing mode.
        self._title_subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::Blur = event {
                    this.editing_items.remove(&id);
                    this.revert_item(id, window, cx);
                }
            },
        ));

        input
    }

    /// Render the hybrid title element: a plain label when not editing (double-click to start),
    /// and a styled input with visible border/background while editing.
    fn render_title_input(
        &self,
        item_id: Uuid,
        title_input: Entity<InputState>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        // let border_color = theme.border;
        let input_color = theme.input;
        let error_color = theme.danger;

        let is_editing = self.editing_items.contains(&item_id);

        let edit_state = self.title_edit_states.get(&item_id);
        let parse_error = edit_state.map(|s| s.parse_error).unwrap_or(false);

        if !is_editing {
            // Label mode: looks like plain text, double-click enters edit mode.
            let title_text: SharedString = title_input.read(cx).value().to_string().into();
            let activate = cx.listener(move |this, event: &ClickEvent, window, cx| {
                if event.click_count() != 2 {
                    return;
                }
                this.editing_items.insert(item_id);
                if let Some(input) = this.title_inputs.get(&item_id) {
                    input.update(cx, |state, cx| {
                        state.set_cursor_position(Position::new(0, u32::MAX), window, cx);
                    });
                }
                cx.notify();
            });
            return div()
                .id(("title-label", item_id.as_u128() as u64))
                .w_full()
                .flex()
                .items_center()
                .px_1()
                .on_click(activate)
                .child(Label::new(title_text).text_sm())
                .into_any_element();
        }

        // Edit mode: input with background and border.
        div()
            .id(("title-input-wrap", item_id.as_u128() as u64))
            .w_full()
            .rounded_sm()
            .border_1()
            .bg(input_color.opacity(0.15))
            .border_color(if parse_error {
                error_color.opacity(0.6)
            } else {
                input_color
            })
            .hover(move |s| {
                s.border_color(if parse_error {
                    error_color
                } else {
                    input_color
                })
                .bg(input_color.opacity(0.15))
            })
            .child(
                Input::new(&title_input)
                    .text_sm()
                    .w_full()
                    .py_0()
                    .px_1()
                    .appearance(false),
            )
            .into_any_element()
    }

    fn render_attached_item(
        &mut self,
        item_id: Uuid,
        item_element_id: ElementId,
        item_colors: ButtonColors,
        _item_title: SharedString,
        item_any: AnyItem,
        title_input: Entity<InputState>,
        is_completing: bool,
        layout: Option<SlotLayout>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let id = item_element_id.clone();
        // let id_clone = item_element_id.clone();
        let colors = item_colors;
        // let title = item_title;
        let any_item = item_any;
        let preview_title: SharedString = any_item.title().into();

        // Slot-based positioning: snap to division boundaries and lay out columns.
        let total_w = self.item_area_width();
        let half_gap = SLOT_GAP * 0.5;
        let scroll = self.scroll_offset();

        // Build the target in time-based coordinates so that neither zoom scaling
        // (hour_height) nor scrolling ever flows through the transition.
        let target = if let Some(ref layout) = layout {
            ItemTimelineBounds {
                elapsed_secs: (layout.visual_start - self.start).num_seconds() as f64,
                duration_secs: (layout.visual_end - layout.visual_start).num_seconds() as f64,
                left_fraction: layout.column_index as f32 / layout.total_columns as f32,
                width_fraction: 1.0 / layout.total_columns as f32,
            }
        } else {
            let time = any_item.time_local()?;
            ItemTimelineBounds {
                elapsed_secs: (time - self.start).num_seconds() as f64,
                duration_secs: any_item
                    .duration()
                    .unwrap_or(FALLBACK_ITEM_DURATION)
                    .num_seconds() as f64,
                left_fraction: 0.0,
                width_fraction: 1.0,
            }
        };

        let bounds_t = attach_transition(
            ("item-bounds", any_item.truncated_id()),
            target,
            RESCHEDULE_TRANSITION_DURATION,
            window,
            cx,
        );
        let changed = bounds_t.update(cx, |val, _| *val = target);
        if changed {
            cx.notify();
        }
        let anim = *bounds_t.evaluate(window, cx);

        // Convert animated time-coordinates to viewport pixels.
        // `hour_height` and `scroll` are applied here — live, outside the transition —
        // so zoom scaling and scrolling are never part of the animated value.
        let y = self.hour_height * (anim.elapsed_secs / 3600.0) as f32
            + HOUR_DIVIDER_HEIGHT / 2.0
            + half_gap
            + scroll;
        let h = self.hour_height * (anim.duration_secs / 3600.0) as f32 - SLOT_GAP;
        let item_left = ATTACHED_ITEM_LEFT + total_w * anim.left_fraction + half_gap;
        let item_w = total_w * anim.width_fraction - SLOT_GAP;

        let fg = cx.theme().muted_foreground;

        // let too_short = h < DEFAULT_ITEM_HEIGHT;
        let too_short = false;
        let is_editing = self.editing_items.contains(&item_id);

        let title_y = (y + STICKY_TITLE_PADDING)
            .max(STICKY_TITLE_PADDING)
            .min(y + h - STICKY_TITLE_HEIGHT - STICKY_TITLE_PADDING);
        let title_rel_y = title_y - y;

        let is_action = matches!(any_item, AnyItem::Action(_));
        let inner = div()
            .id(id)
            .relative()
            .size_full()
            .rounded_lg()
            .button_colors(colors)
            .overflow_hidden()
            .child(
                h_flex()
                    .absolute()
                    .top(title_rel_y)
                    .left(px(0.))
                    .w_full()
                    .h(STICKY_TITLE_HEIGHT)
                    .px_2()
                    .gap_2()
                    .overflow_hidden()
                    .when(!too_short && is_action, |this| {
                        let action_id = item_id;
                        this.child(
                            Checkbox::new(("complete", item_id.as_u128() as u64))
                                .checked(is_completing)
                                .occlude()
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    this.begin_complete_item(action_id, cx);
                                })),
                        )
                    })
                    .when(!too_short, |this| {
                        this.child(self.render_title_input(
                            item_id,
                            title_input.clone(),
                            window,
                            cx,
                        ))
                    }),
            );

        let positioned = div().absolute().top(y).h(h).left(item_left).w(item_w);

        Some(if is_editing {
            // While editing, skip the Draggable so text-selection drags don't
            // move the item.
            positioned.child(inner.w_full()).into_any_element()
        } else {
            let drag_data = DragData::new(any_item.clone())
                .with_label(any_item.title())
                .with_preview(move || {
                    render_item_preview(colors, preview_title.clone(), px(64. * 4.), h, fg)
                        .into_any_element()
                });
            positioned
                .child(
                    Draggable::new((item_element_id.clone(), "draggable"), drag_data)
                        // .block_mouse_except_scroll()
                        .h_full()
                        .w_full()
                        .child(inner)
                        .on_aux_click(|_, _, cx| cx.stop_propagation())
                        .context_menu(move |menu, window, cx| match &any_item {
                            AnyItem::Action(a) => {
                                super::super::action_context_menu(a.id)(menu, window, cx)
                            }
                            AnyItem::Event(e) => {
                                super::super::event_context_menu(e.id)(menu, window, cx)
                            }
                        }),
                )
                .into_any_element()
        })
    }

    fn begin_complete_item(&mut self, action_id: Uuid, cx: &mut Context<Self>) {
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
    ///
    /// All items are snapped to division boundaries (floor toward past, ceil toward future).
    /// Items whose visual spans overlap are grouped together and assigned side-by-side columns.
    fn compute_slot_layouts(&self, items: &[AnyItem]) -> Vec<Option<SlotLayout>> {
        let division = self.current_hour_division();
        let n = items.len();

        // Step 1: visual span for each item.
        let spans: Vec<Option<(DateTime<Local>, DateTime<Local>)>> = items
            .iter()
            .map(|item| {
                let time = item.time_local()?;
                let vs = division.floor_division(time);
                let ve = if let Some(duration) = item.duration() {
                    // Items with a duration: ceil the actual end to the next boundary.
                    let actual_end = time + duration;
                    let ve_raw = division.ceil_division(actual_end);
                    // Guarantee at least one slot.
                    if ve_raw <= vs {
                        vs + division.to_duration()
                    } else {
                        ve_raw
                    }
                } else {
                    // No duration: occupy exactly one slot regardless of where within
                    // the slot the item starts.
                    vs + division.to_duration()
                };
                Some((vs, ve))
            })
            .collect();

        // Step 2: union-find to group items with overlapping visual spans.
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

        // Step 3: group indices by conflict-group root.
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            if spans[i].is_none() {
                continue;
            }
            let root = uf_find(&mut parent, i);
            groups.entry(root).or_default().push(i);
        }

        // Step 4: greedy column assignment per group.
        let mut result: Vec<Option<SlotLayout>> = (0..n).map(|_| None).collect();
        for group in groups.values() {
            // Sort by (visual_start, actual_time) for deterministic ordering.
            let mut sorted = group.clone();
            sorted.sort_by_key(|&i| {
                let (vs, _) = spans[i].unwrap();
                (vs, items[i].time_local().unwrap_or(vs))
            });

            // col_ends[c] = visual_end of the last item placed in column c.
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

    pub(super) fn render_attached_items(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        // Step 1: collect item data (immutable borrow of self.items).
        let item_data: Vec<(
            usize,
            Uuid,
            ElementId,
            ButtonColors,
            SharedString,
            AnyItem,
            TransitionState,
        )> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                (
                    i,
                    item.item.id(),
                    item.element_id.clone(),
                    item.colors(cx),
                    SharedString::from(item.item.title().to_string()),
                    item.item.clone(),
                    item.transition_state,
                )
            })
            .collect();

        // Step 2: compute slot-based layouts for all items at once.
        let any_items: Vec<AnyItem> = item_data
            .iter()
            .map(|(_, _, _, _, _, any, _)| any.clone())
            .collect();
        let slot_layouts = self.compute_slot_layouts(&any_items);

        // Step 3: pre-create title inputs (mutable borrow).
        let mut title_inputs: Vec<Option<Entity<InputState>>> = Vec::with_capacity(item_data.len());
        for (_, item_id, _, _, title, any_item, transition_state) in &item_data {
            if matches!(
                transition_state,
                TransitionState::Attached | TransitionState::Completing
            ) {
                let is_action = matches!(any_item, AnyItem::Action(_));
                let input = self.get_or_create_title_input(
                    *item_id,
                    title.to_string(),
                    is_action,
                    window,
                    cx,
                );
                title_inputs.push(Some(input));
            } else {
                title_inputs.push(None);
            }
        }

        // Step 4: render.
        item_data
            .into_iter()
            .zip(title_inputs)
            .zip(slot_layouts)
            .filter_map(
                |(
                    (
                        (_i, item_id, element_id, colors, title, any_item, transition_state),
                        title_input,
                    ),
                    layout,
                )| {
                    let input = title_input?;
                    self.render_attached_item(
                        item_id,
                        element_id,
                        colors,
                        title,
                        any_item,
                        input,
                        transition_state == TransitionState::Completing,
                        layout,
                        window,
                        cx,
                    )
                    .map(|e| e.into_any_element())
                },
            )
            .collect()
    }

    pub(super) fn handle_drag_move(
        &mut self,
        event: &DragMoveEvent<DragData<AnyItem>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mouse_pos = window.mouse_position();
        let local_pos = self.bounds.and_then(|b| b.localize(&mouse_pos));

        let new_drop = local_pos.map(|pos| {
            let raw_time = self.position_to_time(pos);
            let division = self.current_hour_division();
            let slot_start = division.floor_division(raw_time);
            let slot_end = slot_start + division.to_duration();
            let drop_duration = event.drag(cx).data.duration();

            // Collect items that start in this slot, sorted by actual time.
            let mut items_in_slot: Vec<(DateTime<Local>, ChronoDuration)> = self
                .items
                .iter()
                .filter_map(|ti| {
                    let time = ti.item.time_local()?;
                    if division.floor_division(time) == slot_start {
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

            // Map mouse X to a column index within the slot.
            let area_w = self.item_area_width();
            let col_w = area_w / total_columns as f32;
            let item_relative_x = pos.x - ATTACHED_ITEM_LEFT;
            let column_index = ((item_relative_x / col_w).floor() as usize)
                .clamp(0, total_columns.saturating_sub(1));

            // Scheduled time = insertion point for the chosen column:
            //   column 0 → slot_start
            //   column k → end of the (k-1)-th existing item
            let drop_time = if column_index == 0 || items_in_slot.is_empty() {
                slot_start
            } else {
                let (prev_t, prev_d) = items_in_slot[column_index - 1];
                prev_t + prev_d
            };

            ActiveDropInfo {
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
    }

    pub(super) fn handle_drop(
        &mut self,
        data: &DragData<AnyItem>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    // fn drop_duration(item: &AnyItem) -> ChronoDuration {
    //     item.duration().unwrap_or(FALLBACK_ITEM_DURATION)
    // }

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

    pub(super) fn render_active_drop(
        &self,
        drop_info: ActiveDropInfo,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let half_gap = SLOT_GAP * 0.5;
        let division = self.current_hour_division();
        // Mirror compute_slot_layouts: items with a duration ceil their actual end to the next
        // boundary; items without a duration occupy exactly one slot.
        let visual_end = match drop_info.drop_duration {
            Some(d) => {
                let actual_end = drop_info.drop_time + d;
                let ve_raw = division.ceil_division(actual_end);
                if ve_raw <= drop_info.slot_visual_start {
                    drop_info.slot_visual_end
                } else {
                    ve_raw
                }
            }
            None => drop_info.slot_visual_end,
        };
        let y = self.time_to_offset(drop_info.slot_visual_start) + half_gap;
        let h = self.duration_to_height(visual_end - drop_info.slot_visual_start) - SLOT_GAP;
        let area_w = self.item_area_width();
        let col_w = area_w * (1.0 / drop_info.total_columns as f32);
        let col_x = col_w * drop_info.column_index as f32;
        div()
            .absolute()
            .top(y)
            .h(h)
            .left(ATTACHED_ITEM_LEFT + col_x + half_gap)
            .w(col_w - SLOT_GAP)
            .child(
                div()
                    .w_full()
                    .h_full()
                    .rounded_lg()
                    .border_1()
                    // .border(px(1.5))
                    .border_dashed()
                    .border_color(cx.theme().drag_border),
            )
    }
}
