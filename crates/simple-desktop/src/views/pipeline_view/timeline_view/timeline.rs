use std::{rc::Rc, time::Duration};

use chrono::{DateTime, Duration as ChronoDuration, Local, Timelike};
use gpui::{
    Along, App, Axis, ClickEvent, Context, DismissEvent, Div, Entity, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, Size, StatefulInteractiveElement, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Colorize,
    animation::ease_out_cubic,
    h_flex,
    menu::{PopupMenu, PopupMenuItem},
    v_virtual_list,
};
use gpui_transitions::{Lerp, WindowUseTransition};

use super::TimelineView;
use crate::components::Divider;

pub(super) const DEFAULT_HOUR_HEIGHT: Pixels = px(128.);
pub const HOUR_DIVIDER_HEIGHT: Pixels = px(32.);
const ZOOM_DURATION: Duration = Duration::from_millis(150);
const SCROLL_DURATION: Duration = Duration::from_millis(150);

/// A single interpolation target for a zoom step.
/// Combining hour_height and scroll into one value guarantees they are evaluated
/// at the exact same `Instant::now()` call — there is no way for the two fields
/// to be at different animation-progress values.
#[derive(Clone, PartialEq)]
struct ZoomFrame {
    hour_height: Pixels,
    scroll: Pixels,
}

impl Lerp for ZoomFrame {
    fn lerp(&self, to: &Self, delta: f32) -> Self {
        Self {
            hour_height: self.hour_height.lerp(&to.hour_height, delta),
            scroll: self.scroll.lerp(&to.scroll, delta),
        }
    }
}

fn hour_label(hour: usize, cx: &App) -> Div {
    let primary = match hour {
        0 => "12".to_string(),
        12 => "Noon".to_string(),
        _ if hour < 13 => format!("{}", hour),
        _ => format!("{}", hour - 12),
    };

    let muted = cx.theme().muted_foreground;
    let primary_color = cx.theme().foreground.mix_oklab(muted, 0.5);

    let secondary = match hour {
        12 => None,
        _ if hour < 13 => Some("AM"),
        _ => Some("PM"),
    };

    h_flex()
        .gap_0p5()
        .items_end()
        .child(div().child(primary).text_sm().text_color(primary_color))
        .when_some(secondary, |this, str| {
            this.child(div().child(str).text_xs().text_color(muted))
        })
}

fn time_label(datetime: DateTime<Local>, _cx: &App) -> Div {
    let str = datetime.format("%-I:%M").to_string();

    div().child(str).text_sm()
}

fn timeline_context_menu(
    time: DateTime<Local>,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _window, _cx| {
        let label = time.format("%-I:%M %p").to_string();
        menu.label(label)
            .item(
                PopupMenuItem::new("New action")
                    // .icon(AppIcon::ListPlus)
                    .on_click(move |_event, _window, _cx: &mut App| {
                        // let db_store = AppDatabaseStore::global(cx);
                        // db_store.update(cx, |store, cx| {
                        //     store.complete_action(action_id, cx);
                        // });
                    }),
            )
            .item(
                PopupMenuItem::new("New event")
                    // .icon(AppIcon::CalendarPlus)
                    .on_click(move |_event, _window, _cx: &mut App| {
                        // let db_store = AppDatabaseStore::global(cx);
                        // db_store.update(cx, |store, cx| {
                        //     store.backlog_action(action_id, cx);
                        // });
                    }),
            )
        // .separator()
        // .item(
        //     PopupMenuItem::new("Delete action")
        //         .icon(AppIcon::Trash)
        //         .on_click(move |_event, _window, cx: &mut App| {
        //             let db_store = AppDatabaseStore::global(cx);
        //             db_store.update(cx, |store, cx| {
        //                 store.delete_action(action_id, cx);
        //             });
        //         }),
        // )
    }
}

pub enum HourDivision {
    FiveMinutes,
    TenMinutes,
    QuarterHour,
    HalfHour,
    Hour,
}

impl HourDivision {
    fn n_divisions(&self) -> usize {
        match self {
            HourDivision::Hour => 1,
            HourDivision::HalfHour => 2,
            HourDivision::QuarterHour => 4,
            HourDivision::TenMinutes => 6,
            HourDivision::FiveMinutes => 12,
        }
    }

    fn to_seconds(&self) -> i64 {
        match self {
            HourDivision::Hour => 3600,
            HourDivision::HalfHour => 1800,
            HourDivision::QuarterHour => 900,
            HourDivision::TenMinutes => 600,
            HourDivision::FiveMinutes => 300,
        }
    }

    pub(super) fn to_duration(&self) -> ChronoDuration {
        match self {
            HourDivision::Hour => ChronoDuration::hours(1),
            HourDivision::HalfHour => ChronoDuration::minutes(30),
            HourDivision::QuarterHour => ChronoDuration::minutes(15),
            HourDivision::TenMinutes => ChronoDuration::minutes(10),
            HourDivision::FiveMinutes => ChronoDuration::minutes(5),
        }
    }

    // fn label(&self) -> &'static str {
    //     match self {
    //         HourDivision::Hour => "hour",
    //         HourDivision::HalfHour => "30 min",
    //         HourDivision::QuarterHour => "15 min",
    //         HourDivision::TenMinutes => "10 min",
    //         HourDivision::FiveMinutes => "5 min",
    //     }
    // }

    /// Floor a datetime to the nearest division boundary (toward the past).
    pub(super) fn floor_division(&self, time: DateTime<Local>) -> DateTime<Local> {
        let secs = self.to_seconds();
        let hour_start = time
            .with_minute(0)
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .unwrap();
        let elapsed = (time - hour_start).num_seconds();
        let floored_secs = (elapsed / secs) * secs;
        hour_start + ChronoDuration::seconds(floored_secs)
    }

    /// Ceil a datetime to the nearest division boundary (toward the future).
    /// Returns `time` unchanged if it already lands exactly on a boundary.
    pub(super) fn ceil_division(&self, time: DateTime<Local>) -> DateTime<Local> {
        let floor = self.floor_division(time);
        if floor == time {
            time
        } else {
            floor + self.to_duration()
        }
    }

    fn nearest_division(&self, time: DateTime<Local>) -> DateTime<Local> {
        let hour_start = time
            .with_minute(0)
            .and_then(|t| t.with_second(0))
            .and_then(|t| t.with_nanosecond(0))
            .unwrap();
        let elapsed = (time - hour_start).num_seconds();
        let nearest_division_secs =
            (elapsed as f32 / self.to_seconds() as f32).round() * self.to_seconds() as f32;
        hour_start + chrono::Duration::seconds(nearest_division_secs as i64)
    }
}

impl TimelineView {
    fn refresh_item_sizes(&mut self, height: Pixels) {
        let needed = (self.past_hours + self.future_hours) as usize;
        let new_size = Size::new(Pixels::default(), height);
        // Mutate in place when we have exclusive ownership, otherwise reallocate.
        if let Some(vec) = Rc::get_mut(&mut self.hour_list_sizes) {
            vec.resize(needed, new_size);
            if vec.iter().any(|s| s.height != height) {
                vec.fill(new_size);
            }
        } else {
            self.hour_list_sizes = Rc::new(vec![new_size; needed]);
        }
    }

    pub fn time_to_offset(&self, time: DateTime<Local>) -> Pixels {
        let elapsed_secs = (time - self.start).num_seconds();
        self.scroll_offset()
            + self.hour_height * (elapsed_secs as f32 / 3600.0)
            + (HOUR_DIVIDER_HEIGHT / 2.)
    }

    /// Pixel height for a `chrono::Duration`.
    pub fn duration_to_height(&self, duration: ChronoDuration) -> Pixels {
        self.hour_height * (duration.num_seconds() as f32 / 3600.0)
    }

    pub(super) fn position_to_time(&self, position: Point<Pixels>) -> DateTime<Local> {
        let hours =
            (position.y - self.scroll_offset() - HOUR_DIVIDER_HEIGHT / 2.) / self.hour_height;
        self.start + chrono::Duration::seconds((hours * 3600.0) as i64)
    }

    pub(super) fn current_hour_division(&self) -> HourDivision {
        match self.zoom_state.zoom {
            ..0.5 => HourDivision::Hour,
            0.5..1.0 => HourDivision::HalfHour,
            1.0..2.0 => HourDivision::QuarterHour,
            2.0..4.0 => HourDivision::TenMinutes,
            4.0.. => HourDivision::FiveMinutes,
            _ => HourDivision::Hour,
        }
    }

    pub(super) fn nearest_time(&self, time: DateTime<Local>) -> DateTime<Local> {
        self.current_hour_division().nearest_division(time)
    }

    fn day_height(&self) -> Pixels {
        self.hour_height * 24.
    }

    /// The screen-Y position (in local/bounds-relative coordinates) used as the
    /// fixed anchor point during zoom. Content at this Y stays in place visually.
    fn zoom_anchor_y(&self) -> Pixels {
        self.bounds.map(|b| b.size.height / 2.0).unwrap_or(px(0.))
    }

    // fn hour_height(&self) -> Pixels {
    //     self.hour_height
    // }

    pub(super) fn scroll_offset(&self) -> Pixels {
        self.scroll_handle
            .offset()
            .along(Axis::Vertical)
            .min(px(0.))
    }

    fn scroll_to(&mut self, offset: Pixels) {
        let point = Point::new(px(0.), offset);
        self.scroll_handle.set_offset(point);
    }

    // fn scroll_by(&mut self, offset: Pixels, cx: &mut Context<Self>) {
    //     let point = point(px(0.), )
    // }

    pub(super) fn scroll_next_hour(&mut self, cx: &mut Context<Self>) {
        let base = self.scroll_target.unwrap_or_else(|| self.scroll_offset());
        let new_offset = (base / self.hour_height).round() * self.hour_height - self.hour_height;
        self.scroll_target = Some(new_offset);
        self.pending_scroll_transition = Some(new_offset);
        cx.notify();
    }

    pub(super) fn scroll_previous_hour(&mut self, cx: &mut Context<Self>) {
        let base = self.scroll_target.unwrap_or_else(|| self.scroll_offset());
        let new_offset = (base / self.hour_height).round() * self.hour_height + self.hour_height;
        self.scroll_target = Some(new_offset);
        self.pending_scroll_transition = Some(new_offset);
        cx.notify();
    }

    pub(super) fn scroll_next_day(&mut self, cx: &mut Context<Self>) {
        let base = self.scroll_target.unwrap_or_else(|| self.scroll_offset());
        let new_offset = (base / self.day_height()).round() * self.day_height() - self.day_height();
        self.scroll_target = Some(new_offset);
        self.pending_scroll_transition = Some(new_offset);
        cx.notify();
    }

    pub(super) fn scroll_previous_day(&mut self, cx: &mut Context<Self>) {
        let base = self.scroll_target.unwrap_or_else(|| self.scroll_offset());
        let new_offset = (base / self.day_height()).round() * self.day_height() + self.day_height();
        self.scroll_target = Some(new_offset);
        self.pending_scroll_transition = Some(new_offset);
        cx.notify();
    }

    pub(super) fn zoom_in(&mut self, cx: &mut Context<Self>) {
        let mut new_zoom = self.zoom_state.clone();
        if new_zoom.zoom_in() {
            // needs_jump: true only when no animation is in progress, meaning the scroll
            // handle may have moved independently (user scrolled). When animating, the
            // handle is kept in sync with the transition each frame — no jump needed.
            let needs_jump = self.zoom_scroll_target.is_none();
            let base = self
                .zoom_scroll_target
                .unwrap_or_else(|| self.scroll_offset());
            let a = self.zoom_anchor_y();
            let d = HOUR_DIVIDER_HEIGHT / 2.0;
            let offset = a - d - (a - base - d) * self.zoom_state.zoom_factor;
            self.zoom_scroll_target = Some(offset);
            self.pending_zoom_transition = Some((new_zoom, offset, true, needs_jump));
            cx.notify();
        }
    }

    pub(super) fn zoom_out(&mut self, cx: &mut Context<Self>) {
        let mut new_zoom = self.zoom_state.clone();
        if new_zoom.zoom_out() {
            let needs_jump = self.zoom_scroll_target.is_none();
            let base = self
                .zoom_scroll_target
                .unwrap_or_else(|| self.scroll_offset());
            let a = self.zoom_anchor_y();
            let d = HOUR_DIVIDER_HEIGHT / 2.0;
            let offset = a - d - (a - base - d) / self.zoom_state.zoom_factor;
            self.zoom_scroll_target = Some(offset);
            self.pending_zoom_transition = Some((new_zoom, offset, true, needs_jump));
            cx.notify();
        }
    }

    pub(super) fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        if self.zoom_state.is_zoomed() {
            let needs_jump = self.zoom_scroll_target.is_none();
            let base = self
                .zoom_scroll_target
                .unwrap_or_else(|| self.scroll_offset());
            let a = self.zoom_anchor_y();
            let d = HOUR_DIVIDER_HEIGHT / 2.0;
            // h1/h = DEFAULT_HOUR_HEIGHT / current_value = 1/zoom
            let offset = a - d - (a - base - d) / self.zoom_state.zoom;
            self.zoom_scroll_target = Some(offset);
            let mut new_zoom = self.zoom_state.clone();
            new_zoom.zoom_reset();
            self.pending_zoom_transition = Some((new_zoom, offset, true, needs_jump));
            cx.notify();
        }
    }

    pub(super) fn update_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let zoom_transition = window
            .use_keyed_transition("zoom", cx, ZOOM_DURATION, |_, _| ZoomFrame {
                hour_height: self.zoom_state.current_value(),
                scroll: self.scroll_offset(),
            })
            .with_easing(ease_out_cubic);

        if let Some((zoom, offset, animated, needs_jump)) = self.pending_zoom_transition.take() {
            if animated {
                // When no zoom was running the scroll handle may have moved independently
                // (user scrolled), so sync the transition's starting point to reality first.
                if needs_jump {
                    zoom_transition.jump_to(
                        ZoomFrame {
                            hour_height: self.zoom_state.current_value(),
                            scroll: self.scroll_offset(),
                        },
                        cx,
                    );
                }
                zoom_transition.update(cx, |frame, _cx| {
                    frame.hour_height = zoom.current_value();
                    frame.scroll = offset;
                });
            } else {
                // Instant (e.g. pinch): snap with no animation.
                zoom_transition.jump_to(
                    ZoomFrame {
                        hour_height: zoom.current_value(),
                        scroll: offset,
                    },
                    cx,
                );
                self.scroll_to(offset);
            }
            self.zoom_state = zoom;
        }

        let frame = zoom_transition.evaluate(window, cx).clone();

        self.refresh_item_sizes(frame.hour_height);
        // Drive the scroll handle on every frame a zoom is active, *including* the
        // completion frame. Stopping one frame early (the old else-branch) left the
        // scroll handle at a sub-pixel offset while item sizes had already snapped to
        // their final value, letting the virtual list make a small corrective jump.
        if self.zoom_scroll_target.is_some() {
            self.scroll_to(frame.scroll);
            if frame.hour_height == self.zoom_state.current_value() {
                self.zoom_scroll_target = None;
            }
        }
        self.hour_height = frame.hour_height;
    }

    pub(super) fn render_hour_list(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let hour_height = self.hour_height;

        let scroll_transition = window
            .use_keyed_transition("scroll-transition", cx, SCROLL_DURATION, |_, _| {
                self.scroll_offset()
            })
            .with_easing(ease_out_cubic);
        if let Some(new_offset) = self.pending_scroll_transition.take() {
            scroll_transition.jump_to(self.scroll_offset(), cx);
            scroll_transition.update(cx, |offset, _| {
                *offset = new_offset;
            });
        }
        if scroll_transition.evaluate_delta(cx) != 1.0 {
            let scroll_offset = *scroll_transition.evaluate(window, cx);
            self.scroll_to(scroll_offset);
        } else {
            self.scroll_target = None;
        }

        v_virtual_list(
            cx.entity(),
            "timeline",
            self.hour_list_sizes.clone(),
            move |view, visible_range, _, cx| {
                view.visible_range = visible_range.clone();
                if visible_range.end > view.future_hours - 24 {
                    view.future_hours += 24;
                    view.refresh_item_sizes(view.zoom_state.current_value());
                }
                let start_hour = view.start.hour() as usize;
                // view.visible_date = (start_hour + visible_range.start) / 24;
                let n_ticks = view.current_hour_division().n_divisions();
                let spacing = hour_height / n_ticks as f32;

                visible_range
                    .map(|h| {
                        let absolute_h = start_hour + h;
                        let hour = absolute_h % 24;
                        // let minute_height = DEFAULT_HOUR_ITEM_HEIGHT / 60.0;

                        div()
                            .id(("hour", h))
                            .size_full()
                            .items_start()
                            //
                            .child(
                                h_flex()
                                    .w_full()
                                    .h(HOUR_DIVIDER_HEIGHT)
                                    .gap_2()
                                    .child(Divider::horizontal().w_2().stroke(px(2.)))
                                    .child(hour_label(hour, cx))
                                    .child(Divider::horizontal().flex_1().when_else(
                                        hour == 0,
                                        |this| this.stroke(px(2.)),
                                        |this| this.dashed(),
                                    )),
                            )
                            // intra-hour tick marks
                            .children((0..n_ticks).filter_map(|i| {
                                if i % n_ticks == 0 {
                                    None
                                } else {
                                    Some(
                                        Divider::horizontal()
                                            .absolute()
                                            .top(HOUR_DIVIDER_HEIGHT / 2.0 + spacing * i as f32)
                                            .w_2()
                                            .h(px(1.))
                                            .stroke(px(1.)), // .dashed(),
                                    )
                                }
                            }))
                            // .context_menu(timeline_context_menu())
                            // .on_any_mouse_down(|_, window, cx| {
                            //     window.dispatch_action(Box::new(ClosedViewedItem), cx);
                            // })
                            .on_aux_click(cx.listener(|view, e: &ClickEvent, window, cx| {
                                let click_position = e.position();
                                if let Some(position) = view
                                    .bounds
                                    .and_then(|bounds| bounds.localize(&click_position))
                                {
                                    let time = view.position_to_time(position);
                                    let nearest = view.nearest_time(time);
                                    view.context_menu_position = click_position;
                                    let menu = PopupMenu::build(
                                        window,
                                        cx,
                                        timeline_context_menu(nearest),
                                    );
                                    let subscription = cx.subscribe(
                                        &menu,
                                        |this: &mut TimelineView,
                                         _: Entity<PopupMenu>,
                                         _: &DismissEvent,
                                         cx| {
                                            this.context_menu = None;
                                            this._context_menu_subscription = None;
                                            cx.notify();
                                        },
                                    );
                                    view.context_menu = Some(menu);
                                    view._context_menu_subscription = Some(subscription);
                                    cx.notify();
                                }
                            }))
                    })
                    .collect()
            },
        )
        .track_scroll(&self.scroll_handle)
    }

    pub(super) fn render_now_cursor(&self, cx: &Context<Self>) -> impl IntoElement {
        let now = Local::now();
        // let color = cx.theme().red_light.mix_oklab(cx.theme().foreground, 0.8);
        let color = cx.theme().red.mix_oklab(cx.theme().foreground, 0.8);

        h_flex()
            .absolute()
            .top(self.time_to_offset(now) - HOUR_DIVIDER_HEIGHT / 2.)
            .w_full()
            .h(HOUR_DIVIDER_HEIGHT)
            .child(Divider::horizontal().color(color).w_2())
            .child(
                div()
                    // .mx_1()
                    .px(px(7.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background.alpha(0.8))
                    .rounded_xl()
                    .child(time_label(now, cx).text_sm().text_color(color)),
            )
            .child(Divider::horizontal().color(color).flex_1())
    }
}
