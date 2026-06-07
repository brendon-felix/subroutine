use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Local};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, Context, ElementId, FocusHandle, Focusable, Hsla, IntoElement, ParentElement, Pixels,
    Render, SharedString, Styled, Window, div, px,
};
use gpui_component::Colorize;
use gpui_component::label::Label;
use gpui_component::{ActiveTheme, animation::ease_out_cubic};
use gpui_transitions::{Lerp, Transition, WindowUseTransition};
use simple_core::AnyItem;
use simple_parser::{ParseDraft, parse_action_input, parse_event_input};
use uuid::Uuid;

use crate::utils::ButtonColors;

pub(super) mod interaction;
pub(super) mod render;

/// Fallback duration used when an item has no explicit duration (5 min).
pub(super) const FALLBACK_ITEM_DURATION: ChronoDuration = ChronoDuration::minutes(5);

pub(super) const RESCHEDULE_TRANSITION_DURATION: Duration = Duration::from_millis(150);
pub(super) const COMPLETE_CHECKBOX_DURATION: Duration = Duration::from_millis(200);
pub(super) const ATTACHED_ITEM_LEFT: Pixels = px(16. * 4.);
pub(super) const ITEMS_RIGHT_GAP: Pixels = px(24. * 4.);
pub(super) const SLOT_GAP: Pixels = px(3.);
pub(super) const RESIZE_HANDLE_HEIGHT: Pixels = px(6.);
pub(super) const STICKY_TITLE_HEIGHT: Pixels = px(28.);
pub(super) const STICKY_TITLE_PADDING: Pixels = px(2.);
/// Height of the secondary metadata row (time · duration) shown in tall items.
pub(super) const META_ROW_HEIGHT: Pixels = px(18.);

/// Animatable representation of an attached item's layout in timeline-relative,
/// viewport-independent coordinates.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct ItemTimelineBounds {
    /// Unix timestamp (UTC seconds since epoch) of the item's visual slot start.
    /// Stored as an absolute value so that buffer shifts and division changes —
    /// which move `TimelineView::start` — do not alter this field and thus do
    /// not trigger spurious position animations.  Only a genuine reschedule
    /// (a new `visual_start` time) will animate this value.
    pub elapsed_secs: f64,
    /// Visual duration of the slot in seconds (used to compute height).
    pub duration_secs: f64,
    /// Left edge of the column as a fraction of the item-area width [0, 1).
    pub left_fraction: f32,
    /// Width of the column as a fraction of the item-area width (0, 1].
    pub width_fraction: f32,
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
pub(super) enum TransitionState {
    Attached,
    Completing,
}

pub(crate) enum NavDirection {
    Up,
    Down,
    Left,
    Right,
}

pub(super) struct TimelineItem {
    pub focus_handle: FocusHandle,
    pub element_id: ElementId,
    pub item: AnyItem,
    pub transition_state: TransitionState,
}

impl Focusable for TimelineItem {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TimelineItem {
    pub(super) fn new(item: AnyItem, cx: &App) -> Self {
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

    pub(super) fn colors(&self, cx: &App) -> ButtonColors {
        match self.item {
            AnyItem::Action(_) => ButtonColors::normal(cx.theme().button_primary, cx),
            AnyItem::Event(_) => ButtonColors::normal(
                cx.theme()
                    .button_primary
                    .mix_oklab(cx.theme().foreground, 0.5),
                cx,
            ),
            AnyItem::Routine(_) => ButtonColors::normal(cx.theme().foreground, cx),
        }
    }
}

/// Tracks the live editing state for a single timeline item's title input.
pub(super) struct TitleEditState {
    pub current_text: String,
    pub draft: Option<ParseDraft>,
    /// True when the user has typed something that doesn't parse cleanly.
    pub parse_error: bool,
}

impl TitleEditState {
    pub(super) fn new(initial: String) -> Self {
        let draft = parse_action_input(&initial).ok();
        Self {
            current_text: initial,
            draft,
            parse_error: false,
        }
    }

    pub(super) fn update_action(&mut self, text: String) {
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

    pub(super) fn update_event(&mut self, text: String) {
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

/// Renders a drag-preview chip that matches the timeline item's appearance.
/// All inputs are plain values so the closure can be `'static`.
pub(super) fn render_item_preview(
    colors: ButtonColors,
    title: SharedString,
    item_w: Pixels,
    item_h: Pixels,
    muted_fg: Hsla,
) -> impl IntoElement {
    use gpui_component::h_flex;
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

pub(super) fn attach_transition<T: Lerp + Clone + PartialEq + 'static>(
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
#[derive(Clone, Copy)]
pub(super) struct SlotLayout {
    pub visual_start: DateTime<Local>,
    pub visual_end: DateTime<Local>,
    pub column_index: usize,
    pub total_columns: usize,
}

/// Iterative union-find with path compression.
pub(super) fn uf_find(parent: &mut Vec<usize>, i: usize) -> usize {
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

/// Which edge of an item is being dragged to resize it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ResizeEdge {
    Top,
    Bottom,
}

/// Drag data emitted by the resize handles on timeline items.
#[derive(Clone, Debug)]
pub(crate) struct ResizeDragData {
    pub item_id: Uuid,
    pub edge: ResizeEdge,
}

/// Tracks the in-progress resize of a timeline item.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ActiveResizeState {
    pub item_id: Uuid,
    pub edge: ResizeEdge,
    pub original_time: DateTime<Local>,
    pub original_end: DateTime<Local>,
    /// Snapped new start time (only changes when dragging the top edge).
    pub new_time: DateTime<Local>,
    /// Snapped new end time (only changes when dragging the bottom edge).
    pub new_end: DateTime<Local>,
}

/// Invisible ghost view used as the drag preview for resize handles.
pub(super) struct ResizeGhost;
impl Render for ResizeGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Carries all information needed to render and commit a pending drop.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ActiveDropState {
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
