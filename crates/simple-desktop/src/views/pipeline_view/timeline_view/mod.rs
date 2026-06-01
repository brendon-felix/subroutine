use std::{
    collections::{HashMap, HashSet},
    ops::{Range, RangeInclusive},
    rc::Rc,
    time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, Local, Timelike};
use gpui::{
    App, AsyncApp, Bounds, Context, DragMoveEvent, Element, ElementId, Entity, EventEmitter,
    FocusHandle, Focusable, GlobalElementId, InteractiveElement, IntoElement, KeyBinding,
    MouseUpEvent, ParentElement, PinchEvent, Pixels, Point, Render, Size, Styled, Subscription,
    Window, actions, anchored, deferred, div, prelude::FluentBuilder, px,
};
use gpui_component::input::InputState;
use gpui_component::{
    ActiveTheme, ElementExt, VirtualListScrollHandle, h_flex, label::Label, menu::PopupMenu, v_flex,
};
use uuid::Uuid;

use crate::{
    components::{DragData, DropZone},
    utils::ZoomState,
    views::DeleteItem,
};
use simple_core::AnyItem;

mod items;
mod timeline;

use items::*;
use timeline::*;

actions!([
    ZoomIn,
    ZoomOut,
    ZoomReset,
    FocusItemUp,
    FocusItemDown,
    FocusItemLeft,
    FocusItemRight,
    NextHour,
    PreviousHour,
    NextDay,
    PreviousDay,
    ClosedViewedItem,
    RefreshPipeline
]);

pub(super) struct TimelineView {
    focus_handle: FocusHandle,
    // layout: TimelineLayout,
    hour_height: Pixels,
    start: DateTime<Local>,
    // visible_date: usize,
    past_hours: usize,
    future_hours: usize,
    zoom_state: ZoomState<Pixels>,
    pending_zoom_transition: Option<(ZoomState<Pixels>, Pixels, bool, bool)>,
    zoom_scroll_target: Option<Pixels>,
    pending_scroll_transition: Option<Pixels>,
    scroll_target: Option<Pixels>,
    hour_list_sizes: Rc<Vec<Size<Pixels>>>,
    scroll_handle: VirtualListScrollHandle,
    visible_range: Range<usize>,
    // focused_index: Option<usize>,
    active_drop: Option<ActiveDropState>,
    active_resize: Option<ActiveResizeState>,
    drop_active: bool,
    bounds: Option<Bounds<Pixels>>,
    items: Vec<TimelineItem>,
    loaded: bool,
    /// Tracks the order items were detached, oldest-first.
    /// Rendered in reverse so the newest detached item appears at the top.
    detached_order: Vec<ElementId>,
    context_menu: Option<Entity<PopupMenu>>,
    context_menu_position: Point<Pixels>,
    _context_menu_subscription: Option<Subscription>,
    title_inputs: HashMap<Uuid, Entity<InputState>>,
    title_edit_states: HashMap<Uuid, TitleEditState>,
    editing_items: HashSet<Uuid>,
    /// Remembered visual column fraction (0.0–1.0) for Up/Down navigation.
    /// Stored as the left-edge fraction of the focused column so it maps
    /// proportionally across slots with different column counts.
    target_column_fraction: Option<f32>,
    /// Items that have been created locally but not yet persisted to the database.
    draft_item_ids: HashSet<Uuid>,
    /// Continuous scroll speed (px/frame) applied while dragging near an edge.
    /// Positive = scroll up (toward past), negative = scroll down (toward future).
    edge_scroll_speed: Option<f32>,
    _title_subscriptions: Vec<Subscription>,
}

impl Focusable for TimelineView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Zero-size element whose sole job is to register a window-level `MouseUpEvent`
/// handler during the **paint** phase (the only phase where `window.on_mouse_event`
/// is valid). When the mouse is released anywhere — including outside the timeline
/// — this clears `active_resize` and `edge_scroll_speed` on the view, ensuring
/// the resize outline and edge-scroll never linger after a drag.
struct ResizeDragMouseUpHook(Entity<TimelineView>);

impl IntoElement for ResizeDragMouseUpHook {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for ResizeDragMouseUpHook {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, ()) {
        (window.request_layout(gpui::Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: gpui::Bounds<Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) -> () {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: gpui::Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let entity = self.0.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _window, cx| {
            if phase.bubble() {
                entity.update(cx, |view, cx| {
                    // commit_resize_state is a no-op when active_resize is None,
                    // so this is safe to call on every mouse-up.
                    view.commit_resize_state(cx);
                });
            }
        });
    }
}

impl TimelineView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        let hour_height = DEFAULT_HOUR_HEIGHT;

        let now = Local::now();
        let current_hour = now.hour();
        let past_hours = 24 + current_hour as usize;
        let future_hours = 4 * 24 - current_hour as usize;
        let start = now
            .with_minute(0)
            .and_then(|d| d.with_second(0))
            .and_then(|d| d.with_nanosecond(0))
            .unwrap()
            - ChronoDuration::hours(past_hours as i64);
        let zoom_state = ZoomState::new(hour_height).with_range(RangeInclusive::new(0.25, 16.0));
        let hour_list_sizes = Rc::new(
            (0..(past_hours + future_hours))
                .map(|_| Size::new(Pixels::default(), zoom_state.current_value()))
                .collect(),
        );
        let visible_range = Range::default();
        let scroll_handle = VirtualListScrollHandle::new();
        let offset = DEFAULT_HOUR_HEIGHT * past_hours as f32;
        let point = Point::new(px(0.), -offset);
        scroll_handle.set_offset(point);

        cx.bind_keys([
            KeyBinding::new("cmd-=", ZoomIn, None),
            KeyBinding::new("cmd--", ZoomOut, None),
            KeyBinding::new("cmd-0", ZoomReset, None),
            KeyBinding::new("escape", ClosedViewedItem, None),
            KeyBinding::new("down", FocusItemDown, Some("!Input")),
            KeyBinding::new("up", FocusItemUp, Some("!Input")),
            KeyBinding::new("left", FocusItemLeft, Some("!Input")),
            KeyBinding::new("right", FocusItemRight, Some("!Input")),
            KeyBinding::new("j", FocusItemDown, Some("!Input")),
            KeyBinding::new("k", FocusItemUp, Some("!Input")),
            KeyBinding::new("h", FocusItemLeft, Some("!Input")),
            KeyBinding::new("l", FocusItemRight, Some("!Input")),
            KeyBinding::new("cmd-down", NextHour, Some("!Input")),
            KeyBinding::new("cmd-up", PreviousHour, Some("!Input")),
            // KeyBinding::new("alt-down", NextDay, Some("!Input")),
            // KeyBinding::new("alt-up", PreviousDay, Some("!Input")),
            KeyBinding::new("cmd-r", RefreshPipeline, None),
        ]);

        // let db_store = AppDatabaseStore::global(cx);
        // cx.subscribe(&db_store, |view, store, _: &DataChanged, cx| {
        //     let queue = store.read(cx).sorted_queue();
        //     view.refresh_items(queue, cx);
        // })
        // .detach();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let result = this.update(cx, |_, cx| cx.notify());
                if result.is_err() {
                    break;
                };
            }
        })
        .detach();

        Self {
            focus_handle,
            hour_height,
            start,
            // visible_date,
            past_hours,
            future_hours,
            zoom_state,
            zoom_scroll_target: None,
            pending_zoom_transition: None,
            pending_scroll_transition: None,
            scroll_target: None,
            hour_list_sizes,
            scroll_handle,
            visible_range,
            // focused_index,
            active_drop: None,
            active_resize: None,
            drop_active: false,
            bounds: None,
            items: vec![],
            loaded: false,
            detached_order: Vec::new(),
            context_menu: None,
            context_menu_position: Point::default(),
            _context_menu_subscription: None,
            title_inputs: HashMap::new(),
            title_edit_states: HashMap::new(),
            editing_items: HashSet::new(),
            target_column_fraction: None,
            draft_item_ids: HashSet::new(),
            edge_scroll_speed: None,
            _title_subscriptions: Vec::new(),
        }
    }

    fn zoom_by(&mut self, factor: f32, cx: &mut Context<Self>) {
        let mut new_zoom = self.zoom_state.clone();
        if new_zoom.zoom_by(factor) {
            // Pinch is a continuous gesture — scale from the live scroll position
            // and apply instantly with no eased transition.
            let offset = self.scroll_offset() * factor;
            self.pending_zoom_transition = Some((new_zoom, offset, false, false));
            cx.notify();
        }
    }

    fn render_header(&self, cx: &App) -> impl IntoElement {
        let current_pos = self.scroll_offset();
        let current_pos_hours = (current_pos * -1.0 / self.hour_height).floor() as i32;

        let relative_date_str = match current_pos_hours {
            ..24 => "yesterday".to_string(),
            24..48 => "today".to_string(),
            48..72 => "tomorrow".to_string(),
            72.. => format!("in {} days", (current_pos_hours - 24) / 24),
        };
        let absolute_date_str = (self.start + ChronoDuration::hours(current_pos_hours as i64))
            .format("%a %b %-d")
            .to_string();
        // let date_str = format!("{:+} hours", current_pos_hours);
        let date_str = format!("{} - {}", absolute_date_str, relative_date_str);
        h_flex()
            .justify_end()
            .border_b_1()
            .border_color(cx.theme().border)
            .p_2()
            .child(Label::new(date_str).text_sm())
    }

    pub fn drop_zone(&self) -> DropZone<DragData<AnyItem>> {
        DropZone::new("timeline-drop")
            .size_full()
            .active(self.drop_active)
            .rounded_none()
            .rounded_bl_2xl()
    }
}

impl Render for TimelineView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let hook_entity = entity.clone();
        self.update_layout(window, cx);

        v_flex()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.zoom_in(cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.zoom_out(cx)))
            .on_action(cx.listener(|this, _: &ZoomReset, _, cx| this.zoom_reset(cx)))
            .on_action(cx.listener(|this, _: &FocusItemDown, window, cx| {
                this.navigate_items(NavDirection::Down, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FocusItemUp, window, cx| {
                this.navigate_items(NavDirection::Up, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FocusItemLeft, window, cx| {
                this.navigate_items(NavDirection::Left, window, cx)
            }))
            .on_action(cx.listener(|this, _: &FocusItemRight, window, cx| {
                this.navigate_items(NavDirection::Right, window, cx)
            }))
            .on_action(cx.listener(|this, _: &NextHour, _, cx| this.scroll_next_hour(cx)))
            .on_action(cx.listener(|this, _: &PreviousHour, _, cx| this.scroll_previous_hour(cx)))
            // .on_action(cx.listener(|this, _: &NextDay, _, cx| this.scroll_next_day(cx)))
            // .on_action(cx.listener(|this, _: &PreviousDay, _, cx| this.scroll_previous_day(cx)))
            .size_full()
            // .child(self.render_header(cx))
            .child(
                self.drop_zone()
                    .on_prepaint(move |bounds, _, cx| {
                        entity.update(cx, |view, _| {
                            view.bounds = Some(bounds);
                        });
                    })
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .on_drag_move(cx.listener(
                        |this, event: &DragMoveEvent<DragData<AnyItem>>, window, cx| {
                            let is_over = event.bounds.contains(&window.mouse_position());
                            if is_over != this.drop_active {
                                this.drop_active = is_over;
                                cx.notify();
                            }
                            this.handle_drag_move(event, window, cx);
                        },
                    ))
                    .on_drag_move(cx.listener(
                        |this, event: &DragMoveEvent<ResizeDragData>, window, cx| {
                            this.handle_resize_move(event, window, cx);
                        },
                    ))
                    .on_drop(cx.listener(|this, data: &DragData<AnyItem>, window, cx| {
                        this.handle_drop(data, window, cx);
                        this.drop_active = false;
                        cx.notify();
                    }))
                    .on_pinch(cx.listener(move |view, e: &PinchEvent, _, cx| {
                        view.zoom_by(1.0 + e.delta, cx);
                        cx.notify();
                    }))
                    .child(self.render_hour_list(window, cx))
                    .child(self.render_now_cursor(cx))
                    .children(self.render_sticky_date(cx))
                    .when_some(self.active_drop, |this, drop| {
                        this.child(self.render_active_drop(drop, cx))
                    })
                    .when_some(self.active_resize.clone(), |this, resize| {
                        this.child(self.render_active_resize(&resize, cx))
                            .child(ResizeDragMouseUpHook(hook_entity.clone()))
                    })
                    .when(self.loaded, |this| {
                        this.children(self.render_attached_items(window, cx))
                    })
                    .when(!self.loaded, |this| {
                        this.children(self.render_skeleton_items())
                    }),
            )
            .when_some(self.context_menu.clone(), |this, menu| {
                let position = self.context_menu_position;
                this.child(
                    deferred(
                        anchored()
                            .position(position)
                            .snap_to_window_with_margin(px(8.))
                            .child(menu),
                    )
                    .with_priority(1),
                )
            })
    }
}

impl EventEmitter<DeleteItem> for TimelineView {}
