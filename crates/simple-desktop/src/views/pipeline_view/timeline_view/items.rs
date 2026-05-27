use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, Local, Timelike};
use gpui::{
    AnyElement, App, AppContext, AsyncApp, Bounds, ClickEvent, Context, DragMoveEvent, ElementId,
    Entity, FocusHandle, Focusable, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    Point, SharedString, Size, StatefulInteractiveElement, Styled, Window, div, point,
    prelude::FluentBuilder, px, size,
};
use gpui_component::{
    ActiveTheme, ElementExt, Icon, animation::ease_out_cubic, checkbox::Checkbox, h_flex,
    label::Label, menu::ContextMenuExt, skeleton::Skeleton, v_flex,
};
use gpui_component::{
    Colorize,
    input::{Input, InputEvent, InputState, Position},
};
use gpui_transitions::{Lerp, Transition, WindowUseTransition};
use simple_core::{ActionState, ActionTarget, AnyItem};
/// Fallback duration used when an item has no explicit duration (5 min).
const FALLBACK_ITEM_DURATION: ChronoDuration = ChronoDuration::minutes(5);
use simple_parser::{ParseDraft, parse_action_input, parse_event_input, recurrence_to_rule};
use uuid::Uuid;

use super::TimelineView;
use crate::{
    AppIcon,
    components::{DragData, Draggable},
    stores::AppDatabaseStore,
    utils::{ButtonColorizeExt, ButtonColors},
    views::pipeline_view::timeline_view::timeline::HOUR_DIVIDER_HEIGHT,
};

const ATTACH_TRANSITION_DURATION: Duration = Duration::from_millis(150);
/// Duration for animating an item sliding to its newly-scheduled time slot.
const RESCHEDULE_TRANSITION_DURATION: Duration = Duration::from_millis(150);
/// How long the checkbox check-mark animation takes when completing an action.
const COMPLETE_CHECKBOX_DURATION: Duration = Duration::from_millis(250);
const DEFAULT_ITEM_HEIGHT: Pixels = px(40.);
const MIN_ITEM_GAP: Pixels = px(2.);
/// Left edge of an attached item within the container.
const ATTACHED_ITEM_LEFT: Pixels = px(16. * 4.);
/// Maximum width of the item area (shrinks to fit when the panel is narrower).
// const ATTACHED_ITEM_MAX_WIDTH: Pixels = px(120. * 4.);
/// Gap between the right edge of the item area and the panel boundary (mirrors ATTACHED_ITEM_LEFT).
const ITEMS_RIGHT_GAP: Pixels = px(16. * 4.);
/// Gap (in pixels) inset on every side of each slot cell, giving consistent
/// horizontal and vertical breathing room between adjacent items.
const SLOT_GAP: Pixels = px(3.);
/// Fixed height of each item card in the detached (panel) scroll area.
const DETACHED_ITEM_HEIGHT: Pixels = px(80.);
/// Height of the sticky title row (checkbox + label) for the "sticky within bounds" effect.
const STICKY_TITLE_HEIGHT: Pixels = px(28.);
/// Padding applied above/below the sticky title — used both as the gap from the item's top
/// and as the minimum distance from the container's top edge while stuck.
const STICKY_TITLE_PADDING: Pixels = px(2.);

/// Animatable representation of an attached item's layout in timeline-relative,
/// viewport-independent coordinates.
///
/// Using time-based coordinates instead of pixels means:
/// - Zoom changes `hour_height` at render time only → zoom never triggers this transition.
/// - Scroll is added to `y` at render time only → scrolling never triggers this transition.
/// - Window/panel resize changes `total_w` at render time only → resizing never triggers this.
///
/// The transition fires only for genuine timeline changes:
/// - Reschedule: `elapsed_secs` changes.
/// - Duration edit: `duration_secs` changes.
/// - Slot reorganisation (zoom crosses a division threshold, or items added/removed):
///   `left_fraction` / `width_fraction` change.
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
    Detached,
    Reattaching,
    Detaching,
    Completing,
    Dragging,
}

// pub(super) struct TimelineItemAnimation {
//     id: ElementId,
//     size: Transition<Size<Pixels>>,
//     position: Transition<Point<Pixels>>,
//     opacity: Transition<f32>,
// }

// impl TimelineItemAnimation {
//     fn new(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Self {
//         let id = id.into();
//         let size = attach_transition((id.clone(), "size"), Size::default(), window, cx);
//         let position = attach_transition((id.clone(), "pos"), Point::default(), window, cx);
//         let opacity = attach_transition((id.clone(), "opacity"), 0.0, window, cx);
//         Self {
//             id,
//             size,
//             position,
//             opacity,
//         }
//     }

//     fn transitions(&mut self, window: &mut Window, cx: &mut App) {}

//     fn size(&self, window: &mut Window, cx: &mut App) -> Size<Pixels> {
//         *self.size.evaluate(window, cx)
//     }

//     fn set_size(&mut self, size: Size<Pixels>, window: &mut Window, cx: &mut App) {
//         self.size.update(cx, |s, _| *s = size);
//     }

//     fn update_size(&mut self, size: Size<Pixels>, cx: &mut App) {
//         self.size.update(cx, |s, _| *s = size);
//     }

//     fn position(&self, window: &mut Window, cx: &mut App) -> Point<Pixels> {
//         *self.position.evaluate(window, cx)
//     }

//     fn set_position(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut App) {
//         self.position.update(cx, |p, _| *p = position);
//     }

//     fn update_position(&mut self, position: Point<Pixels>, cx: &mut App) {
//         self.position.update(cx, |p, _| *p = position);
//     }

//     fn opacity(&self, window: &mut Window, cx: &mut App) -> f32 {
//         *self.opacity.evaluate(window, cx)
//     }

//     fn set_opacity(&mut self, opacity: f32, window: &mut Window, cx: &mut App) {
//         self.opacity.update(cx, |o, _| *o = opacity);
//     }

//     fn update_opacity(&mut self, opacity: f32, cx: &mut App) {
//         self.opacity.update(cx, |o, _| *o = opacity);
//     }
// }

pub(super) struct TimelineItem {
    focus_handle: FocusHandle,
    element_id: ElementId,
    item: AnyItem,
    transition_state: TransitionState,
    /// Bounds of this item in its attached (timeline) position, container-relative.
    /// Computed when detach/reattach begins.
    attached_bounds: Option<Bounds<Pixels>>,
    /// Bounds of this item's slot in the detached panel, container-relative.
    /// Captured each frame via on_prepaint while the slot is rendered.
    detached_bounds: Option<Bounds<Pixels>>,
    animation_started: bool,
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
            attached_bounds: None,
            detached_bounds: None,
            animation_started: false,
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

    // fn attached_position(&self, view: &TimelineView) -> Option<Point<Pixels>> {
    //     self.item.time_local().map(|time| {
    //         let y = view.time_to_offset(time);
    //         point(px(16. * 4.), y)
    //     })
    // }

    // fn attached_heght(&self, view: &TimelineView) -> Pixels {
    //     if let Some(duration) = self.item.duration() {
    //         view.duration_to_height(duration)
    //     } else {
    //         DEFAULT_ITEM_HEIGHT
    //     }
    // }

    // fn animation(&self, window: &mut Window, cx: &mut App) -> TimelineItemAnimation {
    //     TimelineItemAnimation::new(self.element_id.clone(), window, cx)
    // }
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

pub struct ActiveDrag {
    dragging_item: AnyItem,
    item_height: Pixels,
    active_snap: Option<DateTime<Local>>,
}

impl ActiveDrag {
    pub fn new(dragging_item: AnyItem, item_height: Pixels) -> Self {
        let active_snap = None;
        Self {
            dragging_item,
            item_height,
            active_snap,
        }
    }

    pub fn update_snap(
        &self,
        mouse_pos: Point<Pixels>,
        scroll_offset: Pixels,
        hour_height: Pixels,
        start_time: DateTime<Local>,
    ) {
        let secs = (mouse_pos.y - scroll_offset - HOUR_DIVIDER_HEIGHT / 2.0) / hour_height * 3600.;
        let _time = start_time + chrono::Duration::seconds(secs.round() as i64);
    }
}

// struct TimelineFloatingItem {
//     item: AnyItem,
//     size: Size<Pixels>,
//     position: Point<Pixels>,
// }

// impl TimelineFloatingItem {
//     fn new(item: AnyItem) -> Self {
//         let size = size(px(64. * 4.), MIN_ITEM_HEIGHT);
//         let position = point(px(16. * 4.), px(0.));
//         Self {
//             item,
//             size,
//             position,
//         }
//     }

//     fn with_size(mut self, size: Size<Pixels>) -> Self {
//         self.size = size;
//         self
//     }

//     fn with_position(mut self, position: Point<Pixels>) -> Self {
//         self.position = position;
//         self
//     }

//     fn set_size(&mut self, size: Size<Pixels>) {
//         self.size = size;
//     }

//     fn set_position(&mut self, position: Point<Pixels>) {
//         self.position = position;
//     }

//     fn id(&self) -> Uuid {
//         self.item.id()
//     }
// }

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

    // fn get_detach_transitions(
    //     &self,
    //     item: &AnyItem,
    //     // init_size: Size<Pixels>,
    //     // init_position: Point<Pixels>,
    //     // init_opacity: f32,
    //     window: &mut Window,
    //     cx: &mut Context<Self>,
    // ) -> (
    //     Transition<f32>,
    //     Transition<Size<Pixels>>,
    //     Transition<Point<Pixels>>,
    //     Transition<f32>,
    // ) {
    //     let idu64 = item.truncated_id();
    //     let transition = attach_transition(("", idu64), 0.0, window, cx);
    //     let size_transition = attach_transition(("size", idu64), Size::default(), window, cx);
    //     let position_transition = attach_transition(("pos", idu64), Point::default(), window, cx);
    //     let opacity_transition = attach_transition(("opacity", idu64), 0.0, window, cx);
    //     (
    //         transition,
    //         size_transition,
    //         position_transition,
    //         opacity_transition,
    //     )
    // }

    // fn render_outline_item(
    //     &self,
    //     item: &TimelineItem,
    //     layout: Option<&SlotLayout>,
    //     cx: &Context<Self>,
    // ) -> Option<impl IntoElement> {
    //     let id = item.element_id.clone();
    //     let id_clone = item.element_id.clone();
    //     let animation_id = item.element_id.clone();
    //     let colors = item.colors(cx);
    //     let title: SharedString = item.item.title().into();
    //     let any_item = item.item.clone();
    //     let preview_title: SharedString = any_item.title().into();

    //     // Slot-based positioning.
    //     let (y, h, item_left, item_w) = if let Some(layout) = layout {
    //         let top = self.time_to_offset(layout.visual_start);
    //         let height = self.duration_to_height(layout.visual_end - layout.visual_start);
    //         let col_w = ATTACHED_ITEM_WIDTH * (1.0 / layout.total_columns as f32);
    //         let col_x = col_w * layout.column_index as f32;
    //         (top, height, ATTACHED_ITEM_LEFT + col_x, col_w)
    //     } else {
    //         let time = item.item.time_local()?;
    //         let top = self.time_to_offset(time);
    //         let height = item
    //             .item
    //             .duration()
    //             .map(|d| self.duration_to_height(d))
    //             .unwrap_or(DEFAULT_ITEM_HEIGHT);
    //         (top, height, ATTACHED_ITEM_LEFT, ATTACHED_ITEM_WIDTH)
    //     };

    //     let too_short = h < DEFAULT_ITEM_HEIGHT;

    //     Some(
    //         div()
    //             .absolute()
    //             .top(y)
    //             .h(h)
    //             .left(item_left)
    //             .w(item_w)
    //             .child(
    //                 div()
    //                     .id(id)
    //                     .h_full()
    //                     .w_full()
    //                     .rounded_lg()
    //                     .border_1()
    //                     .border_color(cx.theme().border)
    //                     .border_dashed()
    //                     // .p_2()
    //                     .when(!too_short, |this| {
    //                         this.p_2().child(
    //                             Label::new(title.clone())
    //                                 .text_sm()
    //                                 .text_color(cx.theme().muted_foreground),
    //                         )
    //                     })
    //                     // .on_mouse_pressure(cx.listener(
    //                     //     move |view, e: &MousePressureEvent, window, cx| {
    //                     //         // println!("Pressure: {}", e.pressure);
    //                     //         let new_value = match e.stage {
    //                     //             PressureStage::Zero => {
    //                     //                 println!("Pressure: {}", e.pressure);
    //                     //                 0.0
    //                     //             }
    //                     //             PressureStage::Normal => {
    //                     //                 println!("Pressure: {}", e.pressure);
    //                     //                 e.pressure
    //                     //             }
    //                     //             PressureStage::Force => {
    //                     //                 println!("Pressure: {}", e.pressure);
    //                     //                 (1.0 + e.pressure)
    //                     //             }
    //                     //         };
    //                     //         pressure_state.update(cx, |value, cx| {
    //                     //             *value = new_value;
    //                     //         });
    //                     //         cx.notify();
    //                     //     },
    //                     // ))
    //                     .on_click(cx.listener(move |view, _, window, cx| {
    //                         view.detach_item(id_clone.clone(), cx);
    //                     })),
    //             )
    //             .context_menu(move |menu, window, cx| match &any_item {
    //                 AnyItem::Action(a) => super::super::action_context_menu(a.id)(menu, window, cx),
    //                 AnyItem::Event(e) => super::super::event_context_menu(e.id)(menu, window, cx),
    //             }),
    //     )
    // }

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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let border_color = theme.border;
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
        item_title: SharedString,
        item_any: AnyItem,
        title_input: Entity<InputState>,
        is_completing: bool,
        layout: Option<SlotLayout>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let id = item_element_id.clone();
        let id_clone = item_element_id.clone();
        let colors = item_colors;
        let title = item_title;
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

        // Compute the "sticky within bounds" title position.
        //
        // When the item is fully visible, the title sits STICKY_TITLE_PADDING from the item's
        // top edge. When the item is partially scrolled past (its top is above the container's
        // top edge), the title sticks at STICKY_TITLE_PADDING from the container top — as if
        // pinned in place. As the item scrolls far enough away that the title no longer fits
        // (padding + title height exceed the remaining visible height), the title follows the
        // item's bottom edge and scrolls out of view naturally.
        //
        // `title_y`     — position of the title row in the container's coordinate space.
        // `title_rel_y` — same position expressed relative to the item's own top edge,
        //                  used for `absolute().top(title_rel_y)` inside the item div.
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
        // .child(
        //     div()
        //         .flex()
        //         .id((item_element_id.clone(), "detach"))
        //         .h_full()
        //         .p_1()
        //         .rounded_md()
        //         .cursor_pointer()
        //         .hover(|s| s.bg(gpui::black().opacity(0.15)))
        //         .items_center()
        //         .justify_center()
        //         .child(
        //             Icon::new(AppIcon::ListIndentIncrease)
        //                 .size_3()
        //                 .text_color(cx.theme().muted_foreground),
        //         )
        //         .on_click(cx.listener(move |view, _, _, cx| {
        //             view.detach_item(id_clone.clone(), cx);
        //             cx.notify();
        //         })),
        // )
        // .on_click(cx.listener(move |_view, _, _window, _cx| {}));

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

    // fn detach_item(&mut self, item_id: ElementId, cx: &mut Context<Self>) {
    //     // Don't re-detach an item that's already in the panel or animating.
    //     let already_out = self
    //         .items
    //         .iter()
    //         .find(|i| i.element_id == item_id)
    //         .map(|i| {
    //             matches!(
    //                 i.transition_state,
    //                 TransitionState::Detached | TransitionState::Detaching
    //             )
    //         })
    //         .unwrap_or(false);
    //     if already_out {
    //         return;
    //     }
    //     // Compute bounds before mutably borrowing items.
    //     let attached_bounds = self
    //         .items
    //         .iter()
    //         .find(|i| i.element_id == item_id)
    //         .and_then(|item| item.item.time_local())
    //         .map(|time| {
    //             let y = self.time_to_offset(time);
    //             let h = self
    //                 .items
    //                 .iter()
    //                 .find(|i| i.element_id == item_id)
    //                 .and_then(|item| item.item.duration())
    //                 .map(|d| self.duration_to_height(d))
    //                 .unwrap_or(DEFAULT_ITEM_HEIGHT);
    //             Bounds {
    //                 origin: point(ATTACHED_ITEM_LEFT, y),
    //                 size: size(ATTACHED_ITEM_WIDTH, h),
    //             }
    //         });
    //     if let Some(index) = self.items.iter().position(|i| i.element_id == item_id) {
    //         if let Some(item) = self.items.get_mut(index) {
    //             item.attached_bounds = attached_bounds;
    //             item.detached_bounds = None; // clear stale bounds from any previous cycle
    //             item.animation_started = false;
    //             item.transition_state = TransitionState::Detaching;
    //             self.detached_order.push(item_id);
    //         }
    //     }
    // }

    // fn reattach_item(&mut self, item_id: ElementId, cx: &mut Context<Self>) {
    //     // Compute bounds before mutably borrowing items.
    //     let attached_bounds = self
    //         .items
    //         .iter()
    //         .find(|i| i.element_id == item_id)
    //         .and_then(|item| item.item.time_local())
    //         .map(|time| {
    //             let y = self.time_to_offset(time);
    //             let h = self
    //                 .items
    //                 .iter()
    //                 .find(|i| i.element_id == item_id)
    //                 .and_then(|item| item.item.duration())
    //                 .map(|d| self.duration_to_height(d))
    //                 .unwrap_or(DEFAULT_ITEM_HEIGHT);
    //             Bounds {
    //                 origin: point(ATTACHED_ITEM_LEFT, y),
    //                 size: size(ATTACHED_ITEM_WIDTH, h),
    //             }
    //         });
    //     if let Some(index) = self.items.iter().position(|i| i.element_id == item_id) {
    //         if let Some(item) = self.items.get_mut(index) {
    //             item.attached_bounds = attached_bounds;
    //             item.animation_started = false;
    //             item.transition_state = TransitionState::Reattaching;
    //             self.detached_order.retain(|id| id != &item_id);
    //         }
    //     }
    // }

    // pub(super) fn render_detached_items(
    //     &mut self,
    //     window: &mut Window,
    //     cx: &mut Context<Self>,
    // ) -> impl IntoElement {
    //     let entity = cx.entity();

    //     // Collect IDs newest-first so the latest detached item appears at the top.
    //     let ordered_ids: Vec<ElementId> = self.detached_order.iter().rev().cloned().collect();

    //     // Pre-collect data for Detached items (immutable borrow).
    //     struct DetachedItemData {
    //         element_id: ElementId,
    //         uuid: Uuid,
    //         is_action: bool,
    //         title: String,
    //         colors: ButtonColors,
    //     }
    //     let detached_data: Vec<DetachedItemData> = ordered_ids
    //         .iter()
    //         .filter_map(|eid| {
    //             let item = self.items.iter().find(|i| &i.element_id == eid)?;
    //             if !matches!(
    //                 item.transition_state,
    //                 TransitionState::Detached | TransitionState::Completing
    //             ) {
    //                 return None;
    //             }
    //             Some(DetachedItemData {
    //                 element_id: item.element_id.clone(),
    //                 uuid: item.item.id(),
    //                 is_action: matches!(item.item, AnyItem::Action(_)),
    //                 title: item.item.title().to_string(),
    //                 colors: item.colors(cx),
    //             })
    //         })
    //         .collect();

    //     // Pre-create title inputs for Detached items (mutable borrow).
    //     let mut detached_inputs: HashMap<ElementId, Entity<InputState>> = HashMap::new();
    //     for data in &detached_data {
    //         let input = self.get_or_create_title_input(
    //             data.uuid,
    //             data.title.clone(),
    //             data.is_action,
    //             window,
    //             cx,
    //         );
    //         detached_inputs.insert(data.element_id.clone(), input);
    //     }

    //     div()
    //         .id("detached-list")
    //         .absolute()
    //         .inset_0()
    //         .left(px(128. * 4.))
    //         .overflow_y_scroll()
    //         // .occlude()
    //         // .bg(cx.theme().secondary)
    //         // .border_1()
    //         // .border_color(cx.theme().border)
    //         // .rounded_lg()
    //         .flex()
    //         .flex_col_reverse()
    //         .p_2()
    //         .gap_2()
    //         .children(ordered_ids.into_iter().filter_map(|item_id| {
    //             let item = self.items.iter().find(|i| i.element_id == item_id)?;
    //             let entity = entity.clone();
    //             let item_id2 = item_id.clone();

    //             Some(match item.transition_state {
    //                 TransitionState::Detaching => {
    //                     // Invisible placeholder so on_prepaint captures the target
    //                     // slot bounds for the flying card animation.
    //                     div()
    //                         .w_full()
    //                         .h(DETACHED_ITEM_HEIGHT)
    //                         .flex_shrink_0()
    //                         .rounded_lg()
    //                         .on_prepaint(move |bounds, _window, cx| {
    //                             entity.update(cx, |view, _cx| {
    //                                 // Read container_origin now — the container's own
    //                                 // on_prepaint has already run and updated view.bounds.
    //                                 let container_origin =
    //                                     view.bounds.map(|b| b.origin).unwrap_or_default();
    //                                 if let Some(item) =
    //                                     view.items.iter_mut().find(|i| i.element_id == item_id)
    //                                 {
    //                                     item.detached_bounds = Some(Bounds {
    //                                         origin: bounds.origin - container_origin,
    //                                         size: bounds.size,
    //                                     });
    //                                 }
    //                             });
    //                         })
    //                         .into_any_element()
    //                 }
    //                 TransitionState::Detached | TransitionState::Completing => {
    //                     let id = item.element_id.clone();
    //                     let close_id: ElementId = (item.element_id.clone(), "close").into();
    //                     let colors = item.colors(cx);
    //                     let uuid = item.item.id();
    //                     let is_action = item.item.is_action();
    //                     let is_completing = item.transition_state == TransitionState::Completing;
    //                     let title_input = detached_inputs.get(&item.element_id).cloned();
    //                     v_flex()
    //                         .id(item.element_id.clone())
    //                         .relative()
    //                         .w_full()
    //                         .h(DETACHED_ITEM_HEIGHT)
    //                         .flex_shrink_0()
    //                         .button_colors(colors)
    //                         .rounded_lg()
    //                         .p_2()
    //                         .child(
    //                             h_flex()
    //                                 .w_full()
    //                                 .gap_2()
    //                                 .items_center()
    //                                 .when(is_action, |this| {
    //                                     let action_id = uuid;
    //                                     this.child(
    //                                         Checkbox::new((
    //                                             "detached-complete",
    //                                             uuid.as_u128() as u64,
    //                                         ))
    //                                         .checked(is_completing)
    //                                         .occlude()
    //                                         .on_click(cx.listener(move |this, _, _window, cx| {
    //                                             this.begin_complete_item(action_id, cx);
    //                                         })),
    //                                     )
    //                                 })
    //                                 .when_some(title_input, |this, input| {
    //                                     this.child(self.render_title_input(uuid, input, window, cx))
    //                                 }),
    //                         )
    //                         .child(
    //                             div()
    //                                 .id(close_id)
    //                                 .absolute()
    //                                 .top_1()
    //                                 .right_1()
    //                                 .p_1()
    //                                 .rounded_md()
    //                                 .cursor_pointer()
    //                                 .hover(|s| s.bg(gpui::black().opacity(0.15)))
    //                                 .child(
    //                                     Icon::new(AppIcon::Close)
    //                                         .size_3()
    //                                         .text_color(cx.theme().muted_foreground),
    //                                 )
    //                                 .on_click(cx.listener(move |view, _, _, cx| {
    //                                     view.reattach_item(id.clone(), cx)
    //                                 })),
    //                         )
    //                         .on_prepaint(move |bounds, _window, cx| {
    //                             entity.update(cx, |view, _cx| {
    //                                 // Read container_origin now — the container's own
    //                                 // on_prepaint has already run and updated view.bounds.
    //                                 let container_origin =
    //                                     view.bounds.map(|b| b.origin).unwrap_or_default();
    //                                 if let Some(item) =
    //                                     view.items.iter_mut().find(|i| i.element_id == item_id2)
    //                                 {
    //                                     item.detached_bounds = Some(Bounds {
    //                                         origin: bounds.origin - container_origin,
    //                                         size: bounds.size,
    //                                     });
    //                                 }
    //                             });
    //                         })
    //                         .into_any_element()
    //                 }
    //                 // Reattaching items are handled by render_flying_items; no slot needed.
    //                 _ => return None,
    //             })
    //         }))
    // }

    // pub(super) fn render_flying_items(
    //     &mut self,
    //     window: &mut Window,
    //     cx: &mut Context<Self>,
    // ) -> Vec<gpui::AnyElement> {
    //     self.items
    //         .iter_mut()
    //         .filter(|i| {
    //             matches!(
    //                 i.transition_state,
    //                 TransitionState::Detaching | TransitionState::Reattaching
    //             )
    //         })
    //         .filter_map(|item| {
    //             let mut anim = item.animation(window, cx);
    //             let colors = item.colors(cx);
    //             let title: SharedString = item.item.title().into();

    //             match item.transition_state {
    //                 TransitionState::Detaching => {
    //                     if !item.animation_started {
    //                         match (item.attached_bounds, item.detached_bounds) {
    //                             (Some(from), Some(to)) => {
    //                                 anim.position.jump_to(from.origin, cx);
    //                                 anim.update_position(to.origin, cx);
    //                                 anim.size.jump_to(from.size, cx);
    //                                 anim.update_size(to.size, cx);
    //                                 item.animation_started = true;
    //                                 cx.notify();
    //                             }
    //                             (Some(from), None) => {
    //                                 // detached_bounds not yet captured — hold at
    //                                 // attached position for one frame while the
    //                                 // slot's on_prepaint fires.
    //                                 anim.position.jump_to(from.origin, cx);
    //                                 anim.size.jump_to(from.size, cx);
    //                             }
    //                             _ => return None,
    //                         }
    //                     }

    //                     let pos = *anim.position.evaluate(window, cx);
    //                     let sz = *anim.size.evaluate(window, cx);

    //                     if item.animation_started && anim.position.evaluate_delta(cx) >= 1.0 {
    //                         item.transition_state = TransitionState::Detached;
    //                         item.animation_started = false;
    //                         cx.notify();
    //                         return None; // let render_detached_items show the card this frame
    //                     }

    //                     Some(
    //                         div()
    //                             .id((item.element_id.clone(), "flying-detach"))
    //                             .absolute()
    //                             .top(pos.y)
    //                             .left(pos.x)
    //                             .w(sz.width)
    //                             .h(sz.height)
    //                             .rounded_lg()
    //                             .button_colors(colors)
    //                             .p_2()
    //                             .child(Label::new(title).text_sm())
    //                             .into_any_element(),
    //                     )
    //                 }
    //                 TransitionState::Reattaching => {
    //                     if !item.animation_started {
    //                         match (item.detached_bounds, item.attached_bounds) {
    //                             (Some(from), Some(to)) => {
    //                                 anim.position.jump_to(from.origin, cx);
    //                                 anim.update_position(to.origin, cx);
    //                                 anim.size.jump_to(from.size, cx);
    //                                 anim.update_size(to.size, cx);
    //                                 item.animation_started = true;
    //                                 cx.notify();
    //                             }
    //                             _ => {
    //                                 // Missing bounds — skip animation, advance immediately.
    //                                 item.transition_state = TransitionState::Attached;
    //                                 cx.notify();
    //                                 return None;
    //                             }
    //                         }
    //                     }

    //                     let pos = *anim.position.evaluate(window, cx);
    //                     let sz = *anim.size.evaluate(window, cx);

    //                     if item.animation_started && anim.position.evaluate_delta(cx) >= 1.0 {
    //                         item.transition_state = TransitionState::Attached;
    //                         item.animation_started = false;
    //                         cx.notify();
    //                         return None; // animation done, attached item takes over this frame
    //                     }

    //                     Some(
    //                         div()
    //                             .id((item.element_id.clone(), "flying-reattach"))
    //                             .absolute()
    //                             .top(pos.y)
    //                             .left(pos.x)
    //                             .w(sz.width)
    //                             .h(sz.height)
    //                             .rounded_lg()
    //                             .button_colors(colors)
    //                             .p_2()
    //                             .child(Label::new(title).text_sm())
    //                             .into_any_element(),
    //                     )
    //                 }
    //                 _ => None,
    //             }
    //         })
    //         .collect()
    // }

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
                        (i, item_id, element_id, colors, title, any_item, transition_state),
                        title_input,
                    ),
                    layout,
                )| {
                    // match transition_state {
                    //     TransitionState::Attached | TransitionState::Completing => {
                    //         let input = title_input?;
                    //         self.render_attached_item(
                    //             item_id,
                    //             element_id,
                    //             colors,
                    //             title,
                    //             any_item,
                    //             input,
                    //             transition_state == TransitionState::Completing,
                    //             layout,
                    //             window,
                    //             cx,
                    //         )
                    //     }
                    //     _ => {
                    //         // For outline rendering we need the original TimelineItem reference;
                    //         // find it by id.
                    //         let outline_item =
                    //             self.items.iter().find(|i| i.element_id == element_id);
                    //         outline_item
                    //             .and_then(|item| {
                    //                 self.render_outline_item(item, layout.as_ref(), cx)
                    //             })
                    //             .map(|e| e.into_any_element())
                    //     }
                    // }
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

    fn drop_duration(item: &AnyItem) -> ChronoDuration {
        item.duration().unwrap_or(FALLBACK_ITEM_DURATION)
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
