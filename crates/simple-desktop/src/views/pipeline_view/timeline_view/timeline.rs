use std::{rc::Rc, time::Duration};

use chrono::{DateTime, Datelike, Days, Duration as ChronoDuration, Local, LocalResult, NaiveDate};
use gpui::Lerp;
use gpui::{
    Along, App, Axis, ClickEvent, Context, DismissEvent, Entity, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, Size, StatefulInteractiveElement, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Colorize, Disableable, Icon, InteractiveElementExt,
    animation::ease_out_cubic,
    button::Button,
    h_flex,
    label::Label,
    menu::{PopupMenu, PopupMenuItem},
    v_virtual_list,
};

use super::{
    BaseTimeDivision, HOUR_DIVIDER_HEIGHT, TimeDivisionState, TimeZoomLevel, TimelineView,
};
use crate::{AppIcon, components::Divider};

const ZOOM_DURATION: Duration = Duration::from_millis(100);
const SCROLL_DURATION: Duration = Duration::from_millis(150);

// const UPCOMING_DURATION: ChronoDuration = ChronoDuration::minutes(15);

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

// pub enum TimeLabel {
//     HourMinute,
//     Hour,
//     MonthDay,

// }

impl TimelineView {
    /// Ensure the hour list has the right number of entries, and that all entries have the specified height
    /// Recomputes the pixel height of every list item based on the current
    /// `hour_height` (px / hour) and the exact calendar duration of each item
    /// stored in `item_start_times`. Called every animation frame during zoom.
    fn update_list_item_heights(&mut self, hour_height: Pixels) {
        let n = self.loaded_divisions.clone().count();
        let division = self.current_division_state().base_division;

        // Pre-compute heights before taking a mutable reference to `hour_list_sizes`.
        let mut heights: Vec<Pixels> = Vec::with_capacity(n);
        for i in 0..n {
            let h = self
                .item_start_times
                .get(i)
                .map(|&t| {
                    let secs = division.exact_duration(t).as_seconds_f32();
                    hour_height * (secs / 3600.0)
                })
                .unwrap_or(hour_height);
            heights.push(h);
        }

        if let Some(vec) = Rc::get_mut(&mut self.hour_list_sizes) {
            vec.resize(n, Size::default());
            for (size, h) in vec.iter_mut().zip(heights.iter().copied()) {
                size.height = h;
            }
        } else {
            self.hour_list_sizes = Rc::new(
                heights
                    .into_iter()
                    .map(|h| Size::new(Pixels::default(), h))
                    .collect(),
            );
        }
    }

    /// Rebuilds `item_start_times` from scratch by iterating `next_boundary`
    /// starting at `self.start`. Must be called whenever `self.start` changes
    /// or the loaded item count changes (buffer prepend / trim / append).
    fn rebuild_item_start_times(&mut self) {
        let n = self.loaded_divisions.clone().count();
        let division = self.current_division_state().base_division;
        self.item_start_times.clear();
        self.item_start_times.reserve(n);
        let mut t = self.start;
        for _ in 0..n {
            self.item_start_times.push(t);
            t = division.next_boundary(t);
        }
    }

    /// Called when the base division changes due to zoom crossing a threshold.
    /// Re-centres the item list around the viewport, computes correct scroll
    /// positions for both ends of the zoom animation, and updates
    /// `time_division_state`.
    ///
    /// Returns `target_scroll` — the scroll position that keeps `center_time`
    /// at the viewport centre once `hour_height` has fully transitioned to the
    /// new zoom level.  The caller must use this as the animation's target
    /// scroll and as the new `zoom_scroll_target`.
    ///
    /// # Why not `old_scroll − delta_px`?
    ///
    /// The naïve approach of adjusting the existing scroll by the pixel
    /// distance the list origin moved fails because:
    ///
    /// * When zooming **in** (e.g. Hour → FiveMinutes) the new list origin is
    ///   *after* the old one, making `delta_px` negative and the corrected
    ///   scroll positive — above the list top.
    /// * The animation *target* scroll is derived from `zoom_out()`'s formula
    ///   at the old `hour_height`.  Subtracting a delta computed at the old
    ///   scale does not yield the correct value at the *new* scale, producing
    ///   multi-year jumps for large-ratio divisions (Day → Month, etc.).
    ///
    /// Instead, both scrolls are derived directly from `center_time` and the
    /// real `hour_height` values at the start and end of the animation:
    ///
    /// ```text
    /// immediate_scroll = center_y - DIVIDER/2 - old_h  * centre_hours
    /// target_scroll    = center_y - DIVIDER/2 - new_h  * centre_hours
    /// ```
    ///
    /// Because `ZoomFrame::lerp` interpolates both `hour_height` and `scroll`
    /// linearly with the same easing delta, `center_time` stays fixed at
    /// `center_y` for every intermediate animation frame.
    fn apply_division_change(&mut self, new_state: TimeDivisionState) -> Pixels {
        let new_div = new_state.base_division;
        let buffer = Self::division_buffer_size(new_div);

        // Capture both hour_heights before any state mutation.
        // `self.hour_height` is still the OLD value here (updated at the end
        // of `update_layout`).  `zoom_state` is already committed to the new.
        let old_hour_height = self.hour_height;
        let new_hour_height = self.zoom_state.current_value();

        // Centre time: the real-world instant currently at the viewport centre.
        let center_y = self.center_relative().y;
        let elapsed_hours =
            (center_y - self.scroll_offset() - HOUR_DIVIDER_HEIGHT / 2.0) / old_hour_height;
        let center_time = self.start + ChronoDuration::seconds((elapsed_hours * 3600.0) as i64);

        // Floor centre to the new division boundary.
        let center_floor = new_div.floor_boundary(center_time);

        // Compute n_past dynamically so that after the rebuild,
        // `visible_range.start` lands inside [buffer, 2*buffer], preventing
        // the buffer-management code from firing immediately and corrupting
        // the scroll offset.
        //
        // visible_range.start ≈ n_past - center_y_in_items, so we solve for
        // n_past that puts it well inside the safe zone [buffer, 3*buffer].
        // Using 4× multiplier ensures that even if the zoom animation shrinks
        // items and advances visible_range.start toward the top, we don't
        // cross the prepend threshold (< buffer) before the animation
        // completes and buffer-management gates re-open.
        //
        // IMPORTANT: size the buffer against `new_hour_height` (the destination
        // scale), not `old_hour_height`.  Using the old scale when jumping many
        // levels at once (e.g. Year→Hour on zoom-reset) produces thousands of
        // items because the old items were enormous — causing GPU buffer overflow.
        let approx_item_hours =
            new_div.exact_duration(center_floor).as_seconds_f32() as f32 / 3600.0;
        let item_height_at_new_h = (new_hour_height * approx_item_hours).max(px(1.));
        let center_y_in_items =
            ((center_y - HOUR_DIVIDER_HEIGHT / 2.0) / item_height_at_new_h).max(0.0);
        let n_past =
            ((buffer as f32 * 4.0 + center_y_in_items).ceil() as usize).max(buffer * 2 + 1);

        // n_future: cover the past mirror, the current viewport, and generous
        // headroom for zooming out (items shrink → more become visible).
        // 4× viewport_items + 6× buffer ensures the append threshold isn't
        // crossed on the frame the zoom animation settles.
        let viewport_height = self.bounds.map(|b| b.size.height).unwrap_or(px(800.));
        let viewport_items = (viewport_height / item_height_at_new_h).ceil() as usize;
        let n_future = n_past + viewport_items * 4 + buffer * 6;

        // New list origin: step n_past boundaries back from center_floor.
        let new_start = Self::step_back_n_divisions(new_div, center_floor, n_past);

        // Exact hours from the new list origin to the centre time.
        // This value is the same whether we use old_hour_height or new_hour_height
        // to express it in pixels — it just scales differently.
        let center_hours = (center_time - new_start).as_seconds_f32() / 3600.0;

        // Scroll at the START of the animation: centre_time at centre_y, with
        // item heights still computed from old_hour_height.
        let immediate_scroll =
            center_y - HOUR_DIVIDER_HEIGHT / 2.0 - old_hour_height * center_hours;

        // Scroll at the END of the animation: centre_time at centre_y, with
        // item heights now computed from new_hour_height.
        let target_scroll = center_y - HOUR_DIVIDER_HEIGHT / 2.0 - new_hour_height * center_hours;

        self.start = new_start;
        self.loaded_divisions = -(n_past as i64)..(n_future as i64);
        self.rebuild_item_start_times();

        // Cancel any in-flight smooth-scroll; its target is now stale.
        self.pending_scroll_transition = None;
        self.scroll_to(immediate_scroll);

        self.time_division_state = new_state;
        target_scroll
    }

    /// Number of items to load per buffer chunk for the given division.
    /// Also used as the near-edge threshold for triggering a load.
    fn division_buffer_size(div: BaseTimeDivision) -> usize {
        match div {
            BaseTimeDivision::Minute => 30,      // 5 minutes of minute items
            BaseTimeDivision::FiveMinutes => 12, // 1 hour of 5-min items
            BaseTimeDivision::Hour => 24,        // 1 day of hours
            BaseTimeDivision::Day => 30,         // ~1 month of days
            BaseTimeDivision::Month => 12,       // 1 year of months
            BaseTimeDivision::Year => 5,
        }
    }

    /// Steps backward `n` division boundaries from `from` using calendar-aware
    /// arithmetic so the result is always exactly on a boundary.
    fn step_back_n_divisions(
        div: BaseTimeDivision,
        from: DateTime<Local>,
        n: usize,
    ) -> DateTime<Local> {
        match div {
            BaseTimeDivision::Minute => {
                BaseTimeDivision::Minute.floor_boundary(from - ChronoDuration::minutes(n as i64))
            }
            BaseTimeDivision::FiveMinutes => BaseTimeDivision::FiveMinutes
                .floor_boundary(from - ChronoDuration::minutes(5 * n as i64)),
            BaseTimeDivision::Hour => {
                BaseTimeDivision::Hour.floor_boundary(from - ChronoDuration::hours(n as i64))
            }
            BaseTimeDivision::Day => {
                let naive = (from.date_naive() - Days::new(n as u64))
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => from - ChronoDuration::days(n as i64),
                }
            }
            BaseTimeDivision::Month => {
                // Subtract n months via an absolute month index using Euclidean
                // division so the result is always a valid (year, month) pair.
                let abs = from.year() * 12 + from.month() as i32 - 1 - n as i32;
                let year = abs.div_euclid(12);
                let month = abs.rem_euclid(12) as u32 + 1;
                let naive = NaiveDate::from_ymd_opt(year, month, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => from - ChronoDuration::days(30 * n as i64),
                }
            }
            BaseTimeDivision::Year => {
                let year = (from.year() - n as i32).max(1);
                let naive = NaiveDate::from_ymd_opt(year, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                match naive.and_local_timezone(Local) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
                    LocalResult::None => from - ChronoDuration::days(365 * n as i64),
                }
            }
        }
    }

    pub fn time_to_offset(&self, time: DateTime<Local>) -> Pixels {
        let elapsed_secs = (time - self.start).as_seconds_f32();
        self.scroll_offset()
            + self.hour_height * (elapsed_secs / 3600.0)
            + (HOUR_DIVIDER_HEIGHT / 2.)
    }

    /// Pixel height for a `chrono::Duration`.
    pub fn duration_to_height(&self, duration: ChronoDuration) -> Pixels {
        self.hour_height * (duration.as_seconds_f32() / 3600.0)
    }

    pub(super) fn position_to_time(&self, position: Point<Pixels>) -> DateTime<Local> {
        let hours =
            (position.y - self.scroll_offset() - HOUR_DIVIDER_HEIGHT / 2.) / self.hour_height;
        self.start + chrono::Duration::seconds((hours * 3600.0) as i64)
    }

    pub(super) fn current_division_state(&self) -> TimeDivisionState {
        // Represent zoom as an integer power-of-2 exponent so every division
        // boundary falls exactly on a power of two and the match arms are
        // unambiguous even under floating-point arithmetic.
        //
        // level n means  2^n <= zoom < 2^(n+1),  i.e. level = floor(log2(zoom)).
        //
        // Approximate item heights at each level (item_height = 128 * 2^n * hours):
        //
        //  level |  FiveMin   Hour      Day     Month     Year
        //    4   |  170 px     —        —        —         —     ZoomedIn
        //    3   |   85 px     —        —        —         —     Normal
        //    2   |   43 px     —        —        —         —     ZoomedOut
        //    1   |    —      256 px     —        —         —     ZoomedIn
        //    0   |    —      128 px     —        —         —     Normal
        //   -1   |    —       64 px     —        —         —     ZoomedOut
        //   -2   |    —        —      768 px     —         —     ZoomedIn
        //   -3   |    —        —      384 px     —         —     Normal
        // -4..-7 |    —        —    192..24 px   —         —     ZoomedOut
        //   -8   |    —        —        —      365 px      —     ZoomedIn
        //   -9   |    —        —        —      182 px      —     Normal
        // -10.-11|    —        —        —     91..46 px    —     ZoomedOut
        //  -12   |    —        —        —        —       273 px  ZoomedIn
        //  -13   |    —        —        —        —       137 px  Normal
        // <=-14  |    —        —        —        —      <=68 px  ZoomedOut
        let level = self.zoom_state.zoom.log2().floor() as i32;
        match level {
            8..=i32::MAX => TimeDivisionState {
                base_division: BaseTimeDivision::Minute,
                zoom_level: TimeZoomLevel::ZoomedIn,
            },
            7 => TimeDivisionState {
                base_division: BaseTimeDivision::Minute,
                zoom_level: TimeZoomLevel::Normal,
            },
            6 => TimeDivisionState {
                base_division: BaseTimeDivision::Minute,
                zoom_level: TimeZoomLevel::ZoomedOut,
            },
            5 => TimeDivisionState {
                base_division: BaseTimeDivision::FiveMinutes,
                zoom_level: TimeZoomLevel::ZoomedIn,
            },
            3..=4 => TimeDivisionState {
                base_division: BaseTimeDivision::FiveMinutes,
                zoom_level: TimeZoomLevel::Normal,
            },
            2 => TimeDivisionState {
                base_division: BaseTimeDivision::FiveMinutes,
                zoom_level: TimeZoomLevel::ZoomedOut,
            },
            1 => TimeDivisionState {
                base_division: BaseTimeDivision::Hour,
                zoom_level: TimeZoomLevel::ZoomedIn,
            },
            // 1 => TimeDivisionState {
            //     base_division: BaseTimeDivision::Hour,
            //     zoom_level: TimeZoomLevel::Normal,
            // },
            -1..=0 => TimeDivisionState {
                base_division: BaseTimeDivision::Hour,
                zoom_level: TimeZoomLevel::ZoomedOut,
            },
            // -2 => TimeDivisionState {
            //     base_division: BaseTimeDivision::Hour,
            //     zoom_level: TimeZoomLevel::ZoomedOutFar,
            // },
            -3..=-2 => TimeDivisionState {
                base_division: BaseTimeDivision::Day,
                zoom_level: TimeZoomLevel::ZoomedIn,
            },
            -4 => TimeDivisionState {
                base_division: BaseTimeDivision::Day,
                zoom_level: TimeZoomLevel::Normal,
            },
            -5 => TimeDivisionState {
                base_division: BaseTimeDivision::Day,
                zoom_level: TimeZoomLevel::ZoomedOut,
            },
            -6 => TimeDivisionState {
                base_division: BaseTimeDivision::Day,
                zoom_level: TimeZoomLevel::ZoomedOutFar,
            },
            -7 => TimeDivisionState {
                base_division: BaseTimeDivision::Month,
                zoom_level: TimeZoomLevel::ZoomedIn,
            },
            -9..=-8 => TimeDivisionState {
                base_division: BaseTimeDivision::Month,
                zoom_level: TimeZoomLevel::Normal,
            },
            -11..=-10 => TimeDivisionState {
                base_division: BaseTimeDivision::Month,
                zoom_level: TimeZoomLevel::ZoomedOut,
            },
            -12 => TimeDivisionState {
                base_division: BaseTimeDivision::Year,
                zoom_level: TimeZoomLevel::ZoomedIn,
            },
            -13 => TimeDivisionState {
                base_division: BaseTimeDivision::Year,
                zoom_level: TimeZoomLevel::Normal,
            },
            _ => TimeDivisionState {
                base_division: BaseTimeDivision::Year,
                zoom_level: TimeZoomLevel::ZoomedOut,
            },
        }
    }

    /// Floor a raw time to the current slot boundary (subdivision if present, else base division).
    pub(super) fn floor_to_slot(&self, time: DateTime<Local>) -> DateTime<Local> {
        let state = self.current_division_state();
        let sub = state.current_subdivision();
        sub.map(|s| s.floor_boundary(time))
            .unwrap_or_else(|| state.base_division.floor_boundary(time))
    }

    /// True when the zoom level is fine-grained enough to create/interact with individual items
    /// (FiveMinutes, Hour, or Day/ZoomedIn or Day/Normal).
    pub(super) fn is_item_create_enabled(&self) -> bool {
        let state = self.current_division_state();
        matches!(
            state.base_division,
            BaseTimeDivision::FiveMinutes | BaseTimeDivision::Hour
        ) || (state.base_division == BaseTimeDivision::Day
            && matches!(
                state.zoom_level,
                TimeZoomLevel::ZoomedIn | TimeZoomLevel::Normal
            ))
    }

    /// True when the zoom level is fine enough to render individual items as full-height chips.
    /// At coarser zooms, a summary count label is shown instead.
    pub(super) fn should_render_items(&self) -> bool {
        // let state = self.current_division_state();
        // !matches!(
        //     state.base_division,
        //     BaseTimeDivision::Month | BaseTimeDivision::Year
        // ) && !(state.base_division == BaseTimeDivision::Day
        //     && matches!(
        //         state.zoom_level,
        //         TimeZoomLevel::ZoomedOut | TimeZoomLevel::ZoomedOutFar
        //     ))
        true
    }

    // pub(super) fn nearest_time(&self, time: DateTime<Local>) -> DateTime<Local> {
    //     self.current_hour_division().nearest_division(time)
    // }

    // pub(super) fn floor_time(&self, time: DateTime<Local>) -> DateTime<Local> {
    //     self.current_hour_division().floor_division(time)
    // }

    // fn day_height(&self) -> Pixels {
    //     self.hour_height * 24.
    // }

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
        self.scroll_target = Some(offset);
    }

    // fn scroll_by(&mut self, offset: Pixels, cx: &mut Context<Self>) {
    //     let point = point(px(0.), )
    // }

    pub(super) fn scroll_by(&mut self, duration: ChronoDuration, cx: &mut Context<Self>) {
        let base = self.scroll_target.unwrap_or_else(|| self.scroll_offset());
        let dist = self.duration_to_height(duration);
        self.scroll_to(base - dist);
        cx.notify();
    }

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

    // pub(super) fn scroll_next_day(&mut self, cx: &mut Context<Self>) {
    //     let base = self.scroll_target.unwrap_or_else(|| self.scroll_offset());
    //     // let new_offset = (base / self.day_height()).round() * self.day_height() - self.day_height();
    //     let new_offset = base - self.day_height();
    //     self.scroll_target = Some(new_offset);
    //     self.pending_scroll_transition = Some(new_offset);
    //     cx.notify();
    // }

    // pub(super) fn scroll_previous_day(&mut self, cx: &mut Context<Self>) {
    //     let base = self.scroll_target.unwrap_or_else(|| self.scroll_offset());
    //     // let new_offset = (base / self.day_height()).round() * self.day_height() + self.day_height();
    //     let new_offset = base + self.day_height();
    //     self.scroll_target = Some(new_offset);
    //     self.pending_scroll_transition = Some(new_offset);
    //     cx.notify();
    // }

    pub(super) fn can_zoom_in(&self) -> bool {
        self.zoom_state.can_zoom_in()
    }

    pub(super) fn can_zoom_out(&self) -> bool {
        self.zoom_state.can_zoom_out()
    }

    pub(super) fn is_zoomed(&self) -> bool {
        self.zoom_state.is_zoomed()
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
            // self.pending_zoom_transition = Some((new_zoom, offset, true, true));
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
            // self.pending_zoom_transition = Some((new_zoom, offset, true, true));
            cx.notify();
        }
    }

    pub(super) fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        if self.zoom_state.is_zoomed() {
            self.zoom_to(1.0, cx);
        }
    }

    /// Zooms to an absolute zoom level using the same chained-step
    /// infrastructure as [`zoom_reset`].  Each step advances at most one
    /// `zoom_factor` toward `target`, with division-boundary crossings
    /// triggering a full list rebuild.
    pub(super) fn zoom_to(&mut self, target: f32, cx: &mut Context<Self>) {
        // Clamp to zoom range; don't move if already there.
        let target = target.clamp(*self.zoom_state.range.start(), *self.zoom_state.range.end());
        if (self.zoom_state.zoom - target).abs() < 1e-9 {
            return;
        }
        self.pending_zoom_reset = Some(target);
        self.step_zoom_reset(cx);
    }

    /// Advances zoom one step toward `pending_zoom_reset`.  Each call crosses
    /// at most one base-division boundary, keeping the per-frame element count
    /// within safe limits for the renderer.
    fn step_zoom_reset(&mut self, cx: &mut Context<Self>) {
        let target = match self.pending_zoom_reset {
            Some(t) => t,
            None => return,
        };

        // Find the zoom value at the nearest base-division boundary between
        // the current zoom and the target.  Each base-division boundary sits
        // at an exact power of two.  When zooming in (target > zoom) we want
        // the smallest power-of-two ≥ current; when zooming out (target <
        // zoom) we want the largest power-of-two ≤ current.
        //
        // We also skip the ZoomedOut / ZoomedIn boundaries *within* the same
        // base division because those don't require a list rebuild — only
        // true base-division crossings do.  The boundary zoom values (from
        // `current_division_state`) are:
        //
        //   Year  → Month crossing: 2^-12  (= 1/4096)
        //   Month → Day   crossing: 2^-7   (= 1/128)
        //   Day   → Hour  crossing: 2^-2   (= 1/4)   [was -2 → -1 boundary]
        //   Hour  → FiveMin crossing: 2^2  (= 4)
        //
        // We step the zoom to exactly the boundary value, then let the
        // `apply_division_change` path in `update_layout` do its rebuild.
        // On the next frame, chaining fires again if not yet at target.
        let current = self.zoom_state.zoom;
        let zooming_in = target > current;

        // Candidate: zoom_factor step toward target (the normal zoom step).
        let factor_step = if zooming_in {
            current * self.zoom_state.zoom_factor
        } else {
            current / self.zoom_state.zoom_factor
        };

        // Clamp to [min, max] and to target; don't overshoot.
        let clamped = if zooming_in {
            factor_step.min(target).min(*self.zoom_state.range.end())
        } else {
            factor_step.max(target).max(*self.zoom_state.range.start())
        };

        // If zoom can't change (either hit the target or the range boundary),
        // clear the chain so we don't loop forever.
        if (clamped - current).abs() < 1e-9 {
            self.pending_zoom_reset = None;
            return;
        }
        if (clamped - target).abs() < 1e-9 {
            self.pending_zoom_reset = None;
        }

        // Compute scroll offset that keeps the viewport centre fixed.
        // General formula: new_scroll = a - d - (a - base - d) * ratio
        // where ratio = new_hour_height / old_hour_height = clamped / current.
        // Derivation: centre stays fixed → base + oh * h = offset + nh * h
        // → offset = base + (oh - nh) * h
        //          = base + (1 - ratio) * (a - base - d)
        //          = a - d - ratio * (a - base - d)
        let needs_jump = self.zoom_scroll_target.is_none();
        let base = self
            .zoom_scroll_target
            .unwrap_or_else(|| self.scroll_offset());
        let a = self.center_relative().y;
        let d = HOUR_DIVIDER_HEIGHT / 2.0;
        let ratio = clamped / current;
        let offset = a - d - (a - base - d) * ratio;

        self.zoom_scroll_target = Some(offset);
        let mut new_zoom = self.zoom_state.clone();
        new_zoom.zoom = clamped;
        // self.pending_zoom_transition = Some((new_zoom, offset, true, needs_jump));
        self.pending_zoom_transition = Some((new_zoom, offset, true, needs_jump));
        cx.notify();
    }

    pub(super) fn scroll_reset(&mut self, cx: &mut Context<Self>) {
        let now = Local::now();
        let center_y = self.center_relative().y;
        let d = HOUR_DIVIDER_HEIGHT / 2.0;
        let division = self.current_division_state().base_division;
        let buffer = Self::division_buffer_size(division);
        let viewport_height = self.bounds.map(|b| b.size.height).unwrap_or(px(800.));

        // Check whether 'now' falls inside the currently loaded item range.
        let loaded_end = self
            .item_start_times
            .last()
            .copied()
            .map(|t| division.next_boundary(t));
        let now_in_range = self.item_start_times.first().map_or(false, |&s| s <= now)
            && loaded_end.map_or(false, |e| now < e);

        if now_in_range {
            // Simple path: animate the scroll handle to centre 'now'.
            let elapsed_secs = (now - self.start).as_seconds_f32();
            let target = center_y - d - self.hour_height * (elapsed_secs / 3600.0);
            self.scroll_target = Some(target);
            self.pending_scroll_transition = Some(target);
        } else {
            // 'now' is outside the loaded range.  Rebuild the list centred on
            // 'now', then animate from a point ¼-viewport away in the approach
            // direction so the transition feels like a deliberate scroll.
            let now_floor = division.floor_boundary(now);
            let approx_item_hours = division.exact_duration(now_floor).as_seconds_f32() / 3600.0;
            let item_height = (self.hour_height * approx_item_hours).max(px(1.));
            let viewport_items = (viewport_height / item_height).ceil() as usize;

            // Size n_past so visible_range.start ≈ 2×buffer at animation end,
            // comfortably inside [buffer, 3×buffer] — buffer management won't
            // fire during the short animation.
            let n_past = (2 * buffer + viewport_items).max(buffer + 1);
            let n_future = n_past + viewport_items * 4 + buffer * 6;
            let new_start = Self::step_back_n_divisions(division, now_floor, n_past);

            // Target: 'now' at the viewport centre.
            let now_secs = (now - new_start).as_seconds_f32();
            let target_scroll = center_y - d - self.hour_height * (now_secs / 3600.0);

            // Approach direction: start ¼-viewport away on the side we're coming from.
            let current_scroll = self.scroll_target.unwrap_or_else(|| self.scroll_offset());
            let current_center_secs = (center_y - current_scroll - d) / self.hour_height * 3600.0;
            let current_center_time =
                self.start + ChronoDuration::seconds(current_center_secs as i64);
            let animation_distance = viewport_height * 0.25;
            let immediate_scroll = if current_center_time < now {
                // Coming from the past: start slightly above 'now' (less scrolled).
                target_scroll + animation_distance
            } else {
                // Coming from the future: start slightly below 'now' (more scrolled).
                target_scroll - animation_distance
            };

            self.start = new_start;
            self.loaded_divisions = -(n_past as i64)..(n_future as i64);
            self.rebuild_item_start_times();
            self.scroll_to(immediate_scroll);

            // Reset visible_range to an empty sentinel so that buffer management
            // is skipped on the very next frame (before the virtual list reports
            // the real range).  Without this, the stale range from the old list
            // could trigger a spurious prepend/trim against the new layout.
            self.visible_range = n_past..n_past;

            self.scroll_target = Some(target_scroll);
            self.pending_scroll_transition = Some(target_scroll);
        }

        cx.notify();
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

    /// Update list item heights and scroll position based on the zoom animation
    /// and continuous edge-scroll. Also detects base-division changes that occur
    /// when zoom crosses a threshold and rebuilds the item list accordingly.
    pub(super) fn update_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let zoom_transition = window
            .use_keyed_transition("zoom", cx, ZOOM_DURATION, |_, _| ZoomFrame {
                hour_height: self.zoom_state.current_value(),
                scroll: self.scroll_offset(),
            })
            .with_easing(ease_out_cubic);

        if let Some((zoom, mut offset, animated, needs_jump)) = self.pending_zoom_transition.take()
        {
            // Use the actual animated hour_height (not the zoom target) so the
            // jump-to FROM frame is consistent with the current scroll position,
            // even when a second zoom fires while an animation is still running.
            let old_hour_height = self.hour_height;

            // Commit the new zoom state so current_division_state() is up to date.
            self.zoom_state = zoom;

            // Detect a base-division crossing and replace the stale animation
            // target with one derived directly from the physical centre time.
            let new_state = self.current_division_state();
            let division_changed =
                new_state.base_division != self.time_division_state.base_division;
            if division_changed {
                let target_scroll = self.apply_division_change(new_state);
                offset = target_scroll;
                self.zoom_scroll_target = Some(target_scroll);
            }

            let new_hour_height = self.zoom_state.current_value();

            if animated {
                // Re-anchor the animation FROM frame whenever:
                // * no prior animation was running (needs_jump), OR
                // * a division change just re-laid-out the list (the old FROM
                //   frame references a now-invalid coordinate system).
                if needs_jump || division_changed {
                    zoom_transition.jump_to(
                        ZoomFrame {
                            hour_height: old_hour_height,
                            scroll: self.scroll_offset(),
                        },
                        cx,
                    );
                }
                zoom_transition.update(cx, |frame, _cx| {
                    frame.hour_height = new_hour_height;
                    frame.scroll = offset;
                });
            } else {
                zoom_transition.jump_to(
                    ZoomFrame {
                        hour_height: new_hour_height,
                        scroll: offset,
                    },
                    cx,
                );
                self.scroll_to(offset);
            }
        }

        let frame = zoom_transition.evaluate(window, cx).clone();

        self.update_list_item_heights(frame.hour_height);
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

        // Chain zoom-reset: start the next step when the current animation
        // reaches 50% completion.  Balances speed against visual smoothness —
        // the easing curve is never cut short, and the next step picks up
        // smoothly from wherever the animation happens to be.
        if self.pending_zoom_reset.is_some()
            && self.pending_zoom_transition.is_none()
            && zoom_transition.evaluate_delta(cx) >= 0.5
        {
            self.step_zoom_reset(cx);
        }

        // Apply continuous edge-scroll while dragging near the top/bottom border.
        if let Some(speed) = self.edge_scroll_speed {
            let new_offset = self.scroll_offset() + px(speed);
            self.scroll_to(new_offset);
            cx.notify();
        }
    }

    pub(super) fn render_division_list(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entity = cx.entity();
        let hour_height = self.hour_height;

        // ── Buffer management ─────────────────────────────────────────────────
        // Runs BEFORE the scroll transition is evaluated so that any scroll
        // compensation from prepend/trim isn't overwritten on the same frame.
        //
        // Previously this ran inside the virtual list closure, which meant:
        //   1. Scroll transition evaluates → drives handle to position A
        //   2. Closure fires → buffer management calls scroll_to(A ± delta) → B
        //   3. Next frame: transition evaluates again → drives back to A
        //   4. Buffer fires again → compensates → oscillation every frame
        //
        // By running here first, we adjust the scroll handle BEFORE the
        // transition sees it, then immediately jump the transition to the new
        // position so it has no stale target to animate back to.
        //
        // self.visible_range is one frame stale (set inside the virtual list
        // closure on the previous frame) but that is fine: the prepend/trim
        // thresholds are coarse (measured in whole items), so one frame of
        // latency has no perceptible effect.
        let mut buffer_shifted_scroll = false;
        let mut buffer_shift_delta = px(0.);
        if self.zoom_scroll_target.is_none() && !self.visible_range.is_empty() {
            let visible_range = self.visible_range.clone();
            let division = self.current_division_state().base_division;
            let chunk = Self::division_buffer_size(division);
            let buffer = chunk;
            let loaded_range = (self.loaded_divisions.end - self.loaded_divisions.start) as usize;
            let scroll_before = self.scroll_offset();

            // Near the top: slide the window backward to load more past.
            if visible_range.start < buffer {
                let new_start = Self::step_back_n_divisions(division, self.start, chunk);
                let delta = self.hour_height * ((self.start - new_start).as_seconds_f32() / 3600.0);
                self.loaded_divisions.start += chunk as i64;
                self.loaded_divisions.end += chunk as i64;
                self.start = new_start;
                self.scroll_to(self.scroll_offset() - delta);
                self.rebuild_item_start_times();
                self.update_list_item_heights(self.hour_height);
                buffer_shifted_scroll = true;
            }

            // Far from the top: slide the window forward to trim past.
            if visible_range.start > 3 * buffer && self.item_start_times.len() > chunk {
                let new_start = self.item_start_times[chunk];
                let delta = self.hour_height * ((new_start - self.start).as_seconds_f32() / 3600.0);
                self.loaded_divisions.start -= chunk as i64;
                self.loaded_divisions.end -= chunk as i64;
                self.start = new_start;
                self.scroll_to(self.scroll_offset() + delta);
                self.rebuild_item_start_times();
                self.update_list_item_heights(self.hour_height);
                buffer_shifted_scroll = true;
            }

            if buffer_shifted_scroll {
                buffer_shift_delta = self.scroll_offset() - scroll_before;
            }

            // Near the bottom: grow the list to load more future items.
            // Append never adjusts the scroll handle so it needs no special treatment.
            if visible_range.end + buffer > loaded_range {
                self.loaded_divisions.end += chunk as i64;
                self.rebuild_item_start_times();
                self.update_list_item_heights(self.hour_height);
            }
        }

        // ── Zoom-active item count guard ──────────────────────────────────────
        // While a zoom transition is running the virtual list must not render
        // more items than the GPU can handle.  During normal incremental zooms
        // the buffer is sized correctly by `apply_division_change`; this guard
        // is a last line of defence for any remaining edge case (e.g. a pinch
        // gesture that crosses multiple boundaries in one frame).
        //
        // We cap total items at a generous but safe limit.  The trim is done
        // symmetrically around the viewport centre so the user never sees a
        // hard boundary.  Buffer management is already paused during zoom
        // (`zoom_scroll_target.is_some()` prevents the block above from
        // running), so trimming here can't interfere with scroll compensation.
        if self.zoom_scroll_target.is_some() {
            const MAX_ZOOM_ITEMS: usize = 600;
            let n = self.item_start_times.len();
            if n > MAX_ZOOM_ITEMS {
                // Keep a balanced window around the current visible range.
                let vis_mid = if self.visible_range.is_empty() {
                    n / 2
                } else {
                    (self.visible_range.start + self.visible_range.end) / 2
                };
                let half = MAX_ZOOM_ITEMS / 2;
                let trim_start = vis_mid.saturating_sub(half).min(n - MAX_ZOOM_ITEMS);
                let trim_end = (trim_start + MAX_ZOOM_ITEMS).min(n);

                if trim_start > 0 {
                    // Trim head: advance `self.start` and compensate scroll.
                    let new_start = self.item_start_times[trim_start];
                    let delta =
                        self.hour_height * ((new_start - self.start).as_seconds_f32() / 3600.0);
                    self.start = new_start;
                    self.scroll_to(self.scroll_offset() + delta);
                    // Keep zoom_scroll_target in same coordinate system.
                    if let Some(t) = self.zoom_scroll_target.as_mut() {
                        *t += delta;
                    }
                    self.loaded_divisions.start += trim_start as i64;
                }
                let tail_trim = n - trim_end;
                if tail_trim > 0 {
                    self.loaded_divisions.end -= tail_trim as i64;
                }
                self.rebuild_item_start_times();
                self.update_list_item_heights(self.hour_height);
            }
        }

        // ── Scroll transition ─────────────────────────────────────────────────
        let scroll_transition = window
            .use_keyed_transition("scroll-transition", cx, SCROLL_DURATION, |_, _| {
                self.scroll_offset()
            })
            .with_easing(ease_out_cubic);

        if buffer_shifted_scroll {
            // A buffer prepend/trim moved the list origin and compensated the
            // scroll handle.  Snap the transition completely to the new
            // position so the correction is always instantaneous — never
            // animated.  The previous goal was in a different coordinate
            // system and would produce a spurious ease-out if left in place.
            let new_pos = self.scroll_offset();
            scroll_transition.update(cx, |offset, _| {
                *offset = new_pos;
            });
            self.scroll_target = None;
            // Translate any queued intentional target into the new coordinate
            // system so it remains valid after the list-window shift.
            if let Some(pending) = self.pending_scroll_transition.as_mut() {
                *pending = *pending + buffer_shift_delta;
            }
        }
        // A pending target (from scroll_reset or a scroll command) is applied
        // after any buffer-shift correction so the jump uses the already-
        // compensated scroll position as the animation FROM point.
        if let Some(new_offset) = self.pending_scroll_transition.take() {
            scroll_transition.update(cx, |offset, _| {
                *offset = new_offset;
            });
        }
        // During edge-scrolling, keep the goal in sync with the handle so
        // it never drifts.  A stale goal would cause a spurious animation the
        // next time a buffer correction fires after edge-scrolling stops.
        if self.edge_scroll_speed.is_some() {
            let pos = self.scroll_offset();
            scroll_transition.update(cx, |offset, _| {
                *offset = pos;
            });
        }
        if scroll_transition.evaluate_delta(cx) != 1.0 {
            let scroll_offset = *scroll_transition.evaluate(window, cx);
            self.scroll_to(scroll_offset);
        } else {
            self.scroll_target = None;
        }

        // Pre-compute summary counts (actions, events) per division
        // boundary to avoid an O(n_items) scan inside the virtual-list closure for
        // every visible row.
        let pre_state = self.current_division_state();
        let pre_division = pre_state.base_division;
        let pre_show_summary = matches!(
            pre_division,
            BaseTimeDivision::Month | BaseTimeDivision::Year
        ) || (pre_division == BaseTimeDivision::Day
            && matches!(
                pre_state.zoom_level,
                TimeZoomLevel::ZoomedOut | TimeZoomLevel::ZoomedOutFar
            ));

        let summary_counts: std::collections::HashMap<DateTime<Local>, (usize, usize)> =
            if pre_show_summary {
                let mut map = std::collections::HashMap::new();
                for ti in &self.items {
                    if let Some(t) = ti.item.time_local() {
                        let boundary = pre_division.floor_boundary(t);
                        let entry = map.entry(boundary).or_insert((0usize, 0usize));
                        match &ti.item {
                            simple_core::AnyItem::Action(_) => entry.0 += 1,
                            simple_core::AnyItem::Event(_) => entry.1 += 1,
                            simple_core::AnyItem::Routine(_) => entry.0 += 1,
                        }
                    }
                }
                map
            } else {
                std::collections::HashMap::new()
            };

        v_virtual_list(
            cx.entity(),
            "timeline",
            self.hour_list_sizes.clone(),
            move |view, visible_range, _, cx| {
                view.visible_range = visible_range.clone();

                let state = view.current_division_state();
                let division = state.base_division;
                let subdivision = state.current_subdivision();

                // ── Sticky outer-boundary detection ────────────────────────────
                // Which item's outer label is currently claimed by the sticky
                // overlay? We suppress its in-list label to avoid duplication.
                let sticky_outer_start: Option<DateTime<Local>> =
                    division.outer_division().and_then(|outer_div| {
                        let scroll = view.scroll_offset();
                        let hh = view.hour_height;
                        let top_time =
                            view.start + ChronoDuration::seconds(((-scroll) / hh * 3600.0) as i64);
                        let outer_start = outer_div.floor_boundary(top_time);
                        let boundary_y =
                            hh * (outer_start - view.start).as_seconds_f32() / 3600.0 + scroll;
                        if boundary_y < px(0.) {
                            Some(outer_start)
                        } else {
                            None
                        }
                    });

                // ── Item rendering ─────────────────────────────────────────────
                visible_range
                    .map(|i| {
                        let item_time = view.item_start_times.get(i).copied().unwrap_or(view.start);
                        let item_height = view
                            .hour_list_sizes
                            .get(i)
                            .map(|s| s.height)
                            .unwrap_or(hour_height);
                        let is_outer = division.is_outer_boundary(item_time);
                        let is_sticky_suppressed =
                            sticky_outer_start.map_or(false, |s| item_time == s);

                        // Subdivision tick marks inside the item body.
                        let ticks: Vec<_> = if let Some(sub) = subdivision {
                            let item_end = division.next_boundary(item_time);
                            let mut tick_t = sub.next_boundary(item_time);
                            let mut result = vec![];
                            while tick_t < item_end {
                                let elapsed_secs = (tick_t - item_time).as_seconds_f32().round();
                                let tick_y = HOUR_DIVIDER_HEIGHT / 2.0
                                    + hour_height * (elapsed_secs / 3600.0);
                                let label = sub
                                    .style()
                                    .label(tick_t, true, cx)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground);
                                if tick_y < item_height {
                                    result.push(
                                        h_flex()
                                            .absolute()
                                            .top(tick_y - HOUR_DIVIDER_HEIGHT / 2.)
                                            .h(HOUR_DIVIDER_HEIGHT)
                                            .gap_2()
                                            .child(Divider::horizontal().w_2().stroke(px(1.)))
                                            .child(label),
                                    );
                                }
                                tick_t = sub.next_boundary(tick_t);
                            }
                            result
                        } else {
                            vec![]
                        };

                        // Header row: nub + base label + divider line (± outer label).
                        let header = h_flex()
                            .w_full()
                            .h(HOUR_DIVIDER_HEIGHT)
                            .gap_2()
                            .child(Divider::horizontal().w_2().stroke(px(2.)))
                            .child(division.base_label_style().label(item_time, false, cx))
                            // Outer boundary: solid line + outer label on the right.
                            .when(is_outer && !is_sticky_suppressed, |this| {
                                this.child(Divider::horizontal().flex_1().stroke(px(2.)))
                                    .when_some(division.outer_label(item_time), |this, label| {
                                        this.child(
                                            Label::new(label)
                                                .text_color(cx.theme().muted_foreground)
                                                .text_xl(),
                                        )
                                    })
                                    .child(div())
                            })
                            // Outer boundary claimed by sticky: solid line, no label.
                            .when(is_sticky_suppressed, |this| {
                                this.child(Divider::horizontal().flex_1().stroke(px(2.)))
                            })
                            // Normal item: dashed line.
                            .when(!is_outer && !is_sticky_suppressed, |this| {
                                this.child(Divider::horizontal().flex_1().dashed())
                            });

                        // Include the division variant in the element id so
                        // GPUI doesn’t try to diff/reuse elements from the
                        // previous division when the list is rebuilt.
                        let division_ord: usize = match division {
                            BaseTimeDivision::Minute => 0,
                            BaseTimeDivision::FiveMinutes => 0,
                            BaseTimeDivision::Hour => 1,
                            BaseTimeDivision::Day => 2,
                            BaseTimeDivision::Month => 3,
                            BaseTimeDivision::Year => 4,
                        };
                        // Summary label: count actions/events in this period for coarse zooms.
                        let show_summary =
                            matches!(division, BaseTimeDivision::Month | BaseTimeDivision::Year)
                                || (division == BaseTimeDivision::Day
                                    && matches!(
                                        state.zoom_level,
                                        TimeZoomLevel::ZoomedOut | TimeZoomLevel::ZoomedOutFar
                                    ));

                        let summary_label: Option<String> = if show_summary {
                            let (action_count, event_count) =
                                summary_counts.get(&item_time).copied().unwrap_or_default();
                            if action_count + event_count > 0 {
                                let mut parts = Vec::new();
                                if action_count > 0 {
                                    parts.push(format!(
                                        "{} action{}",
                                        action_count,
                                        if action_count == 1 { "" } else { "s" }
                                    ));
                                }
                                if event_count > 0 {
                                    parts.push(format!(
                                        "{} event{}",
                                        event_count,
                                        if event_count == 1 { "" } else { "s" }
                                    ));
                                }
                                Some(parts.join(" \u{00b7} "))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        // Double-click interaction — only at fine zoom levels.
                        let allow_create = view.is_item_create_enabled();
                        let entity_for_dbl = entity.clone();
                        let muted_fg = cx.theme().muted_foreground;

                        let item_div = div()
                            .id(("di", division_ord * 1_000_000 + i))
                            .size_full()
                            .relative()
                            .items_start()
                            .child(header)
                            .children(ticks)
                            .when_some(summary_label, |this, label| {
                                this.child(
                                    h_flex()
                                        .absolute()
                                        .top(HOUR_DIVIDER_HEIGHT + px(4.))
                                        .left(px(64.) + px(8.))
                                        .child(Label::new(label).text_xs().text_color(muted_fg)),
                                )
                            });

                        if allow_create {
                            item_div
                                .on_double_click(move |event: &ClickEvent, window, cx| {
                                    entity_for_dbl.update(cx, |view, cx| {
                                        let Some(bounds) = view.bounds else { return };
                                        let local_y = event.position().y - bounds.origin.y;
                                        let local_pos = Point::new(px(0.), local_y);
                                        let raw_time = view.position_to_time(local_pos);
                                        let floor_time = view.floor_to_slot(raw_time);
                                        view.add_draft_action_in_slot(floor_time, window, cx);
                                    });
                                })
                                .on_aux_click(cx.listener(
                                    move |view, event: &ClickEvent, window, cx| {
                                        if event.is_right_click() {
                                            // Clear any previous context menu.
                                            view.context_menu = None;
                                            view._context_menu_subscription = None;

                                            let local_pos =
                                                event.position() - view.bounds.unwrap().origin;
                                            let time = view.position_to_time(local_pos);
                                            let slot_time = view.floor_to_slot(time);

                                            view.context_menu_position = event.position();

                                            let view_entity = cx.entity().clone();
                                            let menu = PopupMenu::build(
                                                window,
                                                cx,
                                                timeline_context_menu(slot_time, view_entity),
                                            );

                                            // Focus for keyboard navigation.
                                            menu.focus_handle(cx).focus(window, cx);

                                            // Dismiss subscription.
                                            view._context_menu_subscription = Some(cx.subscribe(
                                                &menu,
                                                |this, _menu, _: &DismissEvent, cx| {
                                                    this.context_menu = None;
                                                    this._context_menu_subscription = None;
                                                    cx.notify();
                                                },
                                            ));

                                            view.context_menu = Some(menu);
                                            cx.notify();
                                        }
                                    },
                                ))
                                .into_any_element()
                        } else {
                            item_div.into_any_element()
                        }
                    })
                    .collect()
            },
        )
        .track_scroll(&self.scroll_handle)
    }

    pub(super) fn render_timeline_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let current_division = self.current_division_state().base_division;

        h_flex()
            .p_2()
            .gap_2()
            .block_mouse_except_scroll()
            .child(
                h_flex()
                    .child(
                        Button::new("zoom-in")
                            .icon(Icon::new(AppIcon::ZoomIn))
                            .size_8()
                            .rounded_l_full()
                            .rounded_r_none()
                            .on_click(cx.listener(|view, _event, _window, cx| {
                                view.zoom_in(cx);
                            }))
                            .disabled(!self.can_zoom_in()),
                    )
                    .child(
                        Button::new("zoom-reset")
                            .icon(Icon::new(AppIcon::ZoomReset))
                            .size_8()
                            .rounded_none()
                            .border_x_0()
                            .on_click(cx.listener(|view, _event, _window, cx| {
                                view.zoom_reset(cx);
                            }))
                            .disabled(!self.is_zoomed()),
                    )
                    .child(
                        Button::new("zoom-out")
                            .icon(Icon::new(AppIcon::ZoomOut))
                            .size_8()
                            .rounded_l_none()
                            .rounded_r_full()
                            .on_click(cx.listener(|view, _event, _window, cx| {
                                view.zoom_out(cx);
                            }))
                            .disabled(!self.can_zoom_out()),
                    ),
            )
            .child(
                h_flex()
                    .child(
                        Button::new("zoom-to-5m")
                            .child(Label::new("5m").text_sm())
                            .size_8()
                            .rounded_l_full()
                            .rounded_r_none()
                            .on_click(cx.listener(|view, _event, _window, cx| {
                                view.zoom_to(16.0, cx);
                            }))
                            .disabled(current_division == BaseTimeDivision::FiveMinutes),
                    )
                    .child(
                        Button::new("zoom-to-1h")
                            .child(Label::new("1h").text_sm())
                            .size_8()
                            .rounded_none()
                            .border_l_0()
                            .on_click(cx.listener(|view, _event, _window, cx| {
                                view.zoom_to(1.0, cx);
                            }))
                            .disabled(current_division == BaseTimeDivision::Hour),
                    )
                    .child(
                        Button::new("zoom-to-1d")
                            .child(Label::new("1d").text_sm())
                            .size_8()
                            .rounded_none()
                            .border_l_0()
                            .on_click(cx.listener(|view, _event, _window, cx| {
                                view.zoom_to(1.0 / 16.0, cx);
                            }))
                            .disabled(current_division == BaseTimeDivision::Day),
                    )
                    .child(
                        Button::new("zoom-to-1mo")
                            .child(Label::new("1mo").text_sm())
                            .text_sm()
                            .size_8()
                            .rounded_none()
                            .border_l_0()
                            .on_click(cx.listener(|view, _event, _window, cx| {
                                view.zoom_to(1.0 / 512.0, cx);
                            }))
                            .disabled(current_division == BaseTimeDivision::Month),
                    )
                    .child(
                        Button::new("zoom-to-1y")
                            .child(Label::new("1y").text_sm())
                            .text_sm()
                            .size_8()
                            .border_l_0()
                            .rounded_l_none()
                            .rounded_r_full()
                            .on_click(cx.listener(|view, _event, _window, cx| {
                                view.zoom_to(1.0 / 8192.0, cx);
                            }))
                            .disabled(current_division == BaseTimeDivision::Year),
                    ),
            )
            .child(
                Button::new("scroll-reset")
                    .icon(Icon::new(AppIcon::ListChevronsDownUp))
                    .size_8()
                    .rounded_full()
                    .on_click(cx.listener(|view, _event, _window, cx| {
                        view.scroll_reset(cx);
                    })),
            )
    }

    /// Renders a sticky label for the current outer division period (e.g. the date
    /// when viewing hours, the month when viewing days). The label sticks to the top
    /// of the viewport while the outer-boundary list item is scrolled above it, and
    /// slides up as the next outer boundary approaches from below.
    pub(super) fn render_sticky_outer_label(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let division = self.current_division_state().base_division;
        let outer_div = division.outer_division()?;

        let scroll = self.scroll_offset();
        let hour_height = self.hour_height;

        // Time at the top of the visible viewport.
        let top_time =
            self.start + ChronoDuration::seconds(((-scroll) / hour_height * 3600.0) as i64);

        // Start of the outer period that contains the viewport top.
        let outer_start = outer_div.floor_boundary(top_time);

        // Viewport-relative y of the outer-boundary list item.
        let boundary_y =
            hour_height * (outer_start - self.start).as_seconds_f32() / 3600.0 + scroll;

        // While the boundary item is still on screen the in-list label is visible—no sticky needed.
        if boundary_y >= px(0.) {
            return None;
        }

        let label = division.outer_label(outer_start)?;

        // Viewport y of the *next* outer boundary (drives the push-up animation).
        let next_outer = outer_div.next_boundary(outer_start);
        let next_boundary_y =
            hour_height * (next_outer - self.start).as_seconds_f32() / 3600.0 + scroll;
        let sticky_top = if next_boundary_y < HOUR_DIVIDER_HEIGHT {
            next_boundary_y - HOUR_DIVIDER_HEIGHT
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
                .child(
                    Label::new(label)
                        .text_color(cx.theme().muted_foreground)
                        .text_xl(),
                ),
        )
    }

    pub(super) fn render_now_cursor(&self, cx: &Context<Self>) -> impl IntoElement {
        let now = Local::now();
        // let color = cx.theme().red_light.mix_oklab(cx.theme().foreground, 0.8);
        let color = cx.theme().red.mix_oklab(cx.theme().foreground, 0.8);

        let time_label = match self.current_division_state().base_division {
            BaseTimeDivision::Minute => now.format("%-I:%M:%S").to_string(),
            BaseTimeDivision::FiveMinutes | BaseTimeDivision::Hour | BaseTimeDivision::Day => {
                now.format("%-I:%M").to_string()
            }
            BaseTimeDivision::Month => now.format("%b %-d").to_string(),
            BaseTimeDivision::Year => now.format("%B").to_string(),
        };

        // let upcoming = self
        //     .items
        //     .iter()
        //     .filter(|item| item.item.time_local().is_some())
        //     .find(|item| {
        //         let time = item.item.time_local().unwrap();
        //         time > now && time - now < UPCOMING_DURATION
        //     });
        // let upcoming_label = upcoming.map(|item| {
        //     let time = item.item.time_local().unwrap();
        //     let delta = time - now;
        //     let delta_seconds = delta.as_seconds_f32();
        //     let p = delta_seconds / UPCOMING_DURATION.as_seconds_f32();

        //     let delta_minutes = delta_seconds / 60.;
        //     let time_label = format!("in {}m", delta_minutes.round());
        //     let color = cx.theme().green.mix_oklab(cx.theme().red, p);
        //     let size = px(20. * (1. - p / 2.));
        //     h_flex()
        //         .items_center()
        //         .gap_1()
        //         .child(
        //             Label::new(time_label)
        //                 .text_color(color)
        //                 .text_size(size)
        //                 .line_height(size * 1.25),
        //         )
        //         .child(
        //             Label::new(item.item.title())
        //                 .text_sm()
        //                 .text_color(cx.theme().muted_foreground), // .line_height(size * 1.5),
        //         )
        // });
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
                    .child(Label::new(time_label).text_sm().text_color(color)),
            )
            .child(
                h_flex()
                    .w_full()
                    .child(Divider::horizontal().color(color).flex_1())
                    // .when_some(upcoming_label, |this, label| {
                    //     this.child(
                    //         div()
                    //             // .mx_1()
                    //             .px(px(7.))
                    //             .border_1()
                    //             .border_color(cx.theme().border)
                    //             .bg(cx.theme().background.alpha(0.8))
                    //             .rounded_xl()
                    //             .child(label),
                    //     )
                    //     .child(Divider::horizontal().color(color).w_2())
                    // })
                    .when(false, |this| this),
            )
    }

    // pub(super) fn render_upcoming_arrow(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
    //     let now = Local::now();
    //     // let color = cx.theme().red_light.mix_oklab(cx.theme().foreground, 0.8);
    //     let color = cx.theme().red.mix_oklab(cx.theme().foreground, 0.8);

    //     let upcoming = self
    //         .items
    //         .iter()
    //         .filter(|item| item.item.time_local().is_some())
    //         .find(|item| {
    //             let time = item.item.time_local().unwrap();
    //             time > now && time - now < UPCOMING_DURATION
    //         });
    //     upcoming.map(|item| {
    //         let time = item.item.time_local().unwrap();
    //         let delta = time - now;
    //         let delta_seconds = delta.as_seconds_f32();
    //         let height = self.duration_to_height(delta);
    //         let p = delta_seconds / UPCOMING_DURATION.as_seconds_f32();

    //         let color = cx.theme().green.mix_oklab(cx.theme().red, p);
    //         v_flex()
    //             .absolute()
    //             .h(height)
    //             .top(self.time_to_offset(now))
    //             .right_2()
    //             .items_center()
    //             .child(Divider::vertical_dashed().color(color).h_full())
    //         // .gap_1()
    //         // .child(
    //         //     Label::new(time_label)
    //         //         .text_color(color)
    //         //         .text_size(size)
    //         //         .line_height(size * 1.25),
    //         // )
    //         // .child(
    //         //     Label::new(item.item.title())
    //         //         .text_sm()
    //         //         .text_color(cx.theme().muted_foreground), // .line_height(size * 1.5),
    //         // )
    //     })
    // }
}
