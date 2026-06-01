use std::{rc::Rc, time::Duration};

use chrono::{DateTime, Duration as ChronoDuration, Local, Timelike};
use gpui::{
    Along, App, Axis, ClickEvent, Context, DismissEvent, Div, Entity, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, Size, StatefulInteractiveElement, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Colorize, InteractiveElementExt,
    animation::ease_out_cubic,
    h_flex,
    label::Label,
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
    view: Entity<TimelineView>,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _window, _cx| {
        let label = time.format("%-I:%M %p").to_string();
        menu.label(label)
            .item(
                PopupMenuItem::new("New action")
                    // .icon(AppIcon::ListPlus)
                    .on_click({
                        let view = view.clone();
                        move |_event, window, cx: &mut App| {
                            view.update(cx, |this, cx| {
                                this.add_draft_action(time, window, cx);
                            });
                        }
                    }),
            )
            .item(
                PopupMenuItem::new("New event")
                    // .icon(AppIcon::CalendarPlus)
                    .on_click({
                        let view = view.clone();
                        move |_event, window, cx: &mut App| {
                            view.update(cx, |this, cx| {
                                this.add_draft_event(time, window, cx);
                            });
                        }
                    }),
            )
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
    /// Number of divisions per hour (e.g. 4 for QuarterHour)
    fn n_divisions(&self) -> usize {
        match self {
            HourDivision::Hour => 1,
            HourDivision::HalfHour => 2,
            HourDivision::QuarterHour => 4,
            HourDivision::TenMinutes => 6,
            HourDivision::FiveMinutes => 12,
        }
    }

    /// Number of seconds per division (e.g. 900 for QuarterHour)
    fn to_seconds(&self) -> i64 {
        match self {
            HourDivision::Hour => 3600,
            HourDivision::HalfHour => 1800,
            HourDivision::QuarterHour => 900,
            HourDivision::TenMinutes => 600,
            HourDivision::FiveMinutes => 300,
        }
    }

    /// Convert to `chrono::Duration`
    pub(super) fn to_duration(&self) -> ChronoDuration {
        match self {
            HourDivision::Hour => ChronoDuration::hours(1),
            HourDivision::HalfHour => ChronoDuration::minutes(30),
            HourDivision::QuarterHour => ChronoDuration::minutes(15),
            HourDivision::TenMinutes => ChronoDuration::minutes(10),
            HourDivision::FiveMinutes => ChronoDuration::minutes(5),
        }
    }

    /// Floor a datetime to the nearest division boundary in the past
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

    /// Ceil a datetime to the nearest division boundary in the future
    pub(super) fn ceil_division(&self, time: DateTime<Local>) -> DateTime<Local> {
        let floor = self.floor_division(time);
        if floor == time {
            time
        } else {
            floor + self.to_duration()
        }
    }

    /// Round a datetime to the nearest division boundary
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
    /// Ensure the hour list has the right number of entries, and that all entries have the specified height
    fn update_hour_list_heights(&mut self, height: Pixels) {
        let n_hours = (self.past_hours + self.future_hours) as usize;
        let new_size = Size::new(Pixels::default(), height);
        if let Some(vec) = Rc::get_mut(&mut self.hour_list_sizes) {
            vec.resize(n_hours, new_size);
            if vec.iter().any(|s| s.height != height) {
                vec.fill(new_size);
            }
        } else {
            self.hour_list_sizes = Rc::new(vec![new_size; n_hours]);
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

    pub(super) fn floor_time(&self, time: DateTime<Local>) -> DateTime<Local> {
        self.current_hour_division().floor_division(time)
    }

    fn day_height(&self) -> Pixels {
        self.hour_height * 24.
    }

    pub(super) fn center_relative(&self) -> Point<Pixels> {
        self.bounds
            .map(|b| b.center() - b.origin)
            .unwrap_or_default()
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
        // let new_offset = (base / self.day_height()).round() * self.day_height() - self.day_height();
        let new_offset = base - self.day_height();
        self.scroll_target = Some(new_offset);
        self.pending_scroll_transition = Some(new_offset);
        cx.notify();
    }

    pub(super) fn scroll_previous_day(&mut self, cx: &mut Context<Self>) {
        let base = self.scroll_target.unwrap_or_else(|| self.scroll_offset());
        // let new_offset = (base / self.day_height()).round() * self.day_height() + self.day_height();
        let new_offset = base + self.day_height();
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
            let a = self.center_relative().y;
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
            let a = self.center_relative().y;
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
            let a = self.center_relative().y;
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

    /// compute scroll speed from distance to the top/bottom edge
    pub(super) fn compute_edge_scroll_speed(local_y: Pixels, height: Pixels) -> Option<f32> {
        const ZONE: Pixels = px(80.0);
        const MAX_SPEED: f32 = 18.0;
        let y = local_y;
        let h = height;
        if y < ZONE {
            // Near top → scroll up (positive offset delta)
            let t = 1.0 - (y / ZONE).clamp(0.0, 1.0);
            Some(t * t * MAX_SPEED)
        } else if y > h - ZONE {
            // Near bottom → scroll down (negative offset delta)
            let t = 1.0 - ((h - y) / ZONE).clamp(0.0, 1.0);
            Some(-(t * t * MAX_SPEED))
        } else {
            None
        }
    }

    /// update hour list and scroll position based on zoom animation and edge-scroll
    pub(super) fn update_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let zoom_transition = window
            .use_keyed_transition("zoom", cx, ZOOM_DURATION, |_, _| ZoomFrame {
                hour_height: self.zoom_state.current_value(),
                scroll: self.scroll_offset(),
            })
            .with_easing(ease_out_cubic);

        if let Some((zoom, offset, animated, needs_jump)) = self.pending_zoom_transition.take() {
            if animated {
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

        self.update_hour_list_heights(frame.hour_height);
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

        // Apply continuous edge-scroll while dragging near the top/bottom border.
        if let Some(speed) = self.edge_scroll_speed {
            let new_offset = self.scroll_offset() + px(speed);
            self.scroll_to(new_offset);
            cx.notify();
        }
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
        // While edge-scrolling, snap any in-flight transition so it doesn't
        // fight the per-frame position updates and cause re-entrancy loops.
        if self.edge_scroll_speed.is_some() {
            scroll_transition.jump_to(self.scroll_offset(), cx);
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
                    view.update_hour_list_heights(view.zoom_state.current_value());
                }
                let start_hour = view.start.hour() as usize;
                // view.visible_date = (start_hour + visible_range.start) / 24;
                let n_ticks = view.current_hour_division().n_divisions();
                let spacing = hour_height / n_ticks as f32;

                // Compute which midnight row (if any) is currently claimed by
                // the sticky date label so we can suppress its in-list label.
                let sticky_midnight_h: Option<usize> = {
                    let scroll = view.scroll_offset();
                    let hh = view.hour_height;
                    let sh = view.start.hour() as i64;
                    let h_first = (24 - sh) % 24;
                    let hours_at_top = (-scroll / hh) as i64;
                    let d = (hours_at_top - h_first).div_euclid(24);
                    if d >= 0 {
                        let h = (h_first + d * 24) as usize;
                        let midnight_vp_y = hh * h as f32 + scroll;
                        if midnight_vp_y < px(0.) {
                            Some(h)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

                visible_range
                    .map(|h| {
                        let absolute_h = start_hour + h;
                        let hour = absolute_h % 24;
                        // let minute_height = DEFAULT_HOUR_ITEM_HEIGHT / 60.0;

                        div()
                            .id(("hour", h))
                            .size_full()
                            .items_start()
                            // hour label and divider
                            .child(
                                h_flex()
                                    .w_full()
                                    .h(HOUR_DIVIDER_HEIGHT)
                                    .gap_2()
                                    .child(Divider::horizontal().w_2().stroke(px(2.)))
                                    .child(hour_label(hour, cx))
                                    .when_else(
                                        hour != 0 || sticky_midnight_h == Some(absolute_h),
                                        |this| {
                                            if sticky_midnight_h == Some(absolute_h) {
                                                this.child(
                                                    Divider::horizontal().flex_1().stroke(px(2.)),
                                                )
                                            } else {
                                                this.child(Divider::horizontal().flex_1().dashed())
                                            }
                                        },
                                        |this| {
                                            let date = view.start
                                                + chrono::Duration::hours(absolute_h as i64);
                                            // let label = date.format("%b %-d").to_string();
                                            let label = date.format("%a %-d").to_string();
                                            // let label = match h {
                                            //     0 => "yesterday".to_string(),
                                            //     24 => "today".to_string(),
                                            //     48 => "tomorrow".to_string(),
                                            //     h => format!("day {}", (h - 48) / 24),
                                            // };
                                            this.child(
                                                Divider::horizontal().flex_1().stroke(px(2.)),
                                            )
                                            .child(
                                                Label::new(label)
                                                    .text_color(cx.theme().muted_foreground),
                                            )
                                            .child(div())
                                            // .child(Divider::horizontal().w_2().stroke(px(2.)))
                                        },
                                    ),
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
                            .on_double_click(cx.listener(|view, e: &ClickEvent, window, cx| {
                                let click_position = e.position();
                                if let Some(position) = view
                                    .bounds
                                    .and_then(|bounds| bounds.localize(&click_position))
                                {
                                    let time = view.position_to_time(position);
                                    let floor = view.floor_time(time);
                                    view.add_draft_action(floor, window, cx);
                                }
                            }))
                            .on_aux_click(cx.listener(|view, e: &ClickEvent, window, cx| {
                                let click_position = e.position();
                                if let Some(position) = view
                                    .bounds
                                    .and_then(|bounds| bounds.localize(&click_position))
                                {
                                    let time = view.position_to_time(position);
                                    let floor = view.floor_time(time);
                                    view.context_menu_position = click_position;
                                    let view_entity = cx.entity();
                                    let menu = PopupMenu::build(
                                        window,
                                        cx,
                                        timeline_context_menu(floor, view_entity),
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

    pub(super) fn render_sticky_date(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let scroll = self.scroll_offset();
        let hour_height = self.hour_height;

        let start_hour = self.start.hour() as i64;
        let h_first_midnight = (24 - start_hour) % 24;

        // Which hour index sits at the top of the viewport right now?
        // Pixels / Pixels → f32 in GPUI.
        let hours_at_top = (-scroll / hour_height) as i64;

        // Most recent midnight that has scrolled past (or to) the top.
        let d = (hours_at_top - h_first_midnight).div_euclid(24);
        if d < 0 {
            return None;
        }

        let sticky_h = h_first_midnight + d * 24;

        // If the current day's midnight row is still visible, the in-list label
        // is showing — no sticky needed yet.
        let current_midnight_viewport_y = hour_height * sticky_h as f32 + scroll;
        if current_midnight_viewport_y >= px(0.) {
            return None;
        }

        let date = self.start + ChronoDuration::hours(sticky_h);
        // let label = date.format("%b %-d").to_string();
        let label = date.format("%a %-d").to_string();

        // Viewport-relative position (Pixels) of the *next* day's midnight divider.
        let next_midnight_viewport_y = hour_height * (sticky_h + 24) as f32 + scroll;
        // Push the sticky label up as the next one slides in from below.
        let sticky_top = if next_midnight_viewport_y < HOUR_DIVIDER_HEIGHT {
            next_midnight_viewport_y - HOUR_DIVIDER_HEIGHT
        } else {
            px(0.)
        };

        Some(
            h_flex()
                .absolute()
                .top(sticky_top)
                .right(px(0.))
                .h(HOUR_DIVIDER_HEIGHT)
                .pr_2()
                // .gap_2()
                // .child(Divider::horizontal().w_2().stroke(px(2.)))
                .child(Label::new(label).text_color(cx.theme().muted_foreground)),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    // -------------------------------------------------------------------------
    // HourDivision::n_divisions
    // -------------------------------------------------------------------------

    #[test]
    fn test_hour_division_n_divisions() {
        assert_eq!(HourDivision::Hour.n_divisions(), 1);
        assert_eq!(HourDivision::HalfHour.n_divisions(), 2);
        assert_eq!(HourDivision::QuarterHour.n_divisions(), 4);
        assert_eq!(HourDivision::TenMinutes.n_divisions(), 6);
        assert_eq!(HourDivision::FiveMinutes.n_divisions(), 12);
    }

    // -------------------------------------------------------------------------
    // HourDivision::to_seconds
    // -------------------------------------------------------------------------

    #[test]
    fn test_hour_division_to_seconds() {
        assert_eq!(HourDivision::Hour.to_seconds(), 3600);
        assert_eq!(HourDivision::HalfHour.to_seconds(), 1800);
        assert_eq!(HourDivision::QuarterHour.to_seconds(), 900);
        assert_eq!(HourDivision::TenMinutes.to_seconds(), 600);
        assert_eq!(HourDivision::FiveMinutes.to_seconds(), 300);
    }

    /// n_divisions * to_seconds must always equal one hour (3600 s).
    #[test]
    fn test_hour_division_invariant() {
        for div in [
            HourDivision::Hour,
            HourDivision::HalfHour,
            HourDivision::QuarterHour,
            HourDivision::TenMinutes,
            HourDivision::FiveMinutes,
        ] {
            assert_eq!(
                div.n_divisions() as i64 * div.to_seconds(),
                3600,
                "invariant failed for a HourDivision variant"
            );
        }
    }

    // -------------------------------------------------------------------------
    // HourDivision::to_duration
    // -------------------------------------------------------------------------

    #[test]
    fn test_hour_division_to_duration() {
        assert_eq!(HourDivision::Hour.to_duration(), ChronoDuration::hours(1));
        assert_eq!(
            HourDivision::HalfHour.to_duration(),
            ChronoDuration::minutes(30)
        );
        assert_eq!(
            HourDivision::QuarterHour.to_duration(),
            ChronoDuration::minutes(15)
        );
        assert_eq!(
            HourDivision::TenMinutes.to_duration(),
            ChronoDuration::minutes(10)
        );
        assert_eq!(
            HourDivision::FiveMinutes.to_duration(),
            ChronoDuration::minutes(5)
        );
    }

    // -------------------------------------------------------------------------
    // HourDivision::floor_division
    // -------------------------------------------------------------------------

    fn local_hms(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local::now()
            .with_hour(h)
            .and_then(|t| t.with_minute(m))
            .and_then(|t| t.with_second(s))
            .and_then(|t| t.with_nanosecond(0))
            .unwrap()
    }

    #[test]
    fn test_floor_division_hour_on_boundary() {
        let t = local_hms(10, 0, 0);
        assert_eq!(HourDivision::Hour.floor_division(t), t);
    }

    #[test]
    fn test_floor_division_hour_mid_hour() {
        let t = local_hms(10, 37, 0);
        let expected = local_hms(10, 0, 0);
        assert_eq!(HourDivision::Hour.floor_division(t), expected);
    }

    #[test]
    fn test_floor_division_half_hour_before_midpoint() {
        let t = local_hms(10, 20, 0);
        let expected = local_hms(10, 0, 0);
        assert_eq!(HourDivision::HalfHour.floor_division(t), expected);
    }

    #[test]
    fn test_floor_division_half_hour_after_midpoint() {
        let t = local_hms(10, 45, 0);
        let expected = local_hms(10, 30, 0);
        assert_eq!(HourDivision::HalfHour.floor_division(t), expected);
    }

    #[test]
    fn test_floor_division_quarter_hour() {
        let t = local_hms(10, 17, 0);
        let expected = local_hms(10, 15, 0);
        assert_eq!(HourDivision::QuarterHour.floor_division(t), expected);
    }

    #[test]
    fn test_floor_division_five_minutes() {
        let t = local_hms(10, 22, 0);
        let expected = local_hms(10, 20, 0);
        assert_eq!(HourDivision::FiveMinutes.floor_division(t), expected);
    }

    #[test]
    fn test_floor_division_ten_minutes() {
        let t = local_hms(10, 37, 0);
        let expected = local_hms(10, 30, 0);
        assert_eq!(HourDivision::TenMinutes.floor_division(t), expected);
    }

    // -------------------------------------------------------------------------
    // HourDivision::ceil_division
    // -------------------------------------------------------------------------

    #[test]
    fn test_ceil_division_on_boundary_is_identity() {
        // When already on a boundary, ceil should return the same time.
        let t = local_hms(10, 15, 0);
        assert_eq!(HourDivision::QuarterHour.ceil_division(t), t);
    }

    #[test]
    fn test_ceil_division_quarter_hour() {
        let t = local_hms(10, 7, 0);
        let expected = local_hms(10, 15, 0);
        assert_eq!(HourDivision::QuarterHour.ceil_division(t), expected);
    }

    #[test]
    fn test_ceil_division_half_hour() {
        let t = local_hms(10, 31, 0);
        let expected = local_hms(11, 0, 0);
        assert_eq!(HourDivision::HalfHour.ceil_division(t), expected);
    }

    #[test]
    fn test_ceil_division_five_minutes() {
        let t = local_hms(10, 1, 0);
        let expected = local_hms(10, 5, 0);
        assert_eq!(HourDivision::FiveMinutes.ceil_division(t), expected);
    }

    // -------------------------------------------------------------------------
    // HourDivision::nearest_division
    // -------------------------------------------------------------------------

    #[test]
    fn test_nearest_division_rounds_down() {
        // 10:07 is closer to 10:05 than 10:10
        let t = local_hms(10, 7, 0);
        let expected = local_hms(10, 5, 0);
        assert_eq!(HourDivision::FiveMinutes.nearest_division(t), expected);
    }

    #[test]
    fn test_nearest_division_rounds_up() {
        // 10:08 is closer to 10:10 than 10:05
        let t = local_hms(10, 8, 0);
        let expected = local_hms(10, 10, 0);
        assert_eq!(HourDivision::FiveMinutes.nearest_division(t), expected);
    }

    #[test]
    fn test_nearest_division_quarter_hour_midpoint() {
        // 10:22:30 is the exact midpoint between 10:15 and 10:30; rounds to 10:30
        let t = local_hms(10, 22, 30);
        let expected = local_hms(10, 30, 0);
        assert_eq!(HourDivision::QuarterHour.nearest_division(t), expected);
    }

    // -------------------------------------------------------------------------
    // TimelineView::compute_edge_scroll_speed
    // -------------------------------------------------------------------------

    #[test]
    fn test_edge_scroll_speed_center_returns_none() {
        let height = px(600.0);
        assert!(TimelineView::compute_edge_scroll_speed(px(300.0), height).is_none());
    }

    #[test]
    fn test_edge_scroll_speed_top_edge_positive() {
        let height = px(600.0);
        // y=0 is at the very top — max upward speed
        let speed = TimelineView::compute_edge_scroll_speed(px(0.0), height);
        assert!(speed.is_some());
        let speed = speed.unwrap();
        assert!(speed > 0.0, "top-edge scroll should be positive (upward)");
        // t=1 → speed should equal MAX_SPEED (18.0)
        assert!((speed - 18.0).abs() < 1e-3);
    }

    #[test]
    fn test_edge_scroll_speed_bottom_edge_negative() {
        let height = px(600.0);
        // y == height is at the very bottom — max downward speed
        let speed = TimelineView::compute_edge_scroll_speed(height, height);
        assert!(speed.is_some());
        let speed = speed.unwrap();
        assert!(
            speed < 0.0,
            "bottom-edge scroll should be negative (downward)"
        );
        assert!((speed + 18.0).abs() < 1e-3);
    }

    #[test]
    fn test_edge_scroll_speed_just_inside_zone_boundary() {
        let height = px(600.0);
        // y = 80.0 is exactly at the zone boundary — should return None
        assert!(TimelineView::compute_edge_scroll_speed(px(80.0), height).is_none());
        // y = 79.9 is just inside the top zone — should return Some
        assert!(TimelineView::compute_edge_scroll_speed(px(79.9), height).is_some());
    }

    #[test]
    fn test_edge_scroll_speed_top_partial() {
        let height = px(600.0);
        // y = 40 is halfway into the 80-px zone → t = 0.5 → speed = 0.25 * 18 = 4.5
        let speed = TimelineView::compute_edge_scroll_speed(px(40.0), height).unwrap();
        assert!((speed - 4.5).abs() < 1e-3);
    }

    // -------------------------------------------------------------------------
    // ZoomFrame::lerp
    // -------------------------------------------------------------------------

    #[test]
    fn test_zoom_frame_lerp_identity() {
        let a = ZoomFrame {
            hour_height: px(128.0),
            scroll: px(-256.0),
        };
        let b = ZoomFrame {
            hour_height: px(256.0),
            scroll: px(-512.0),
        };
        let result = a.lerp(&b, 0.0);
        assert_eq!(result.hour_height, px(128.0));
        assert_eq!(result.scroll, px(-256.0));
    }

    #[test]
    fn test_zoom_frame_lerp_full() {
        let a = ZoomFrame {
            hour_height: px(128.0),
            scroll: px(-256.0),
        };
        let b = ZoomFrame {
            hour_height: px(256.0),
            scroll: px(-512.0),
        };
        let result = a.lerp(&b, 1.0);
        assert_eq!(result.hour_height, px(256.0));
        assert_eq!(result.scroll, px(-512.0));
    }

    #[test]
    fn test_zoom_frame_lerp_midpoint() {
        let a = ZoomFrame {
            hour_height: px(0.0),
            scroll: px(0.0),
        };
        let b = ZoomFrame {
            hour_height: px(200.0),
            scroll: px(-400.0),
        };
        let result = a.lerp(&b, 0.5);
        assert_eq!(result.hour_height, px(100.0));
        assert_eq!(result.scroll, px(-200.0));
    }
}
