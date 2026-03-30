use chrono::Utc;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};
use std::{sync::Arc, time::Duration};
use tachyonfx::{EffectManager, EffectTimer, Interpolation, RefRect, fx};
use ticks::tasks::{Task, TaskPriority};

use crate::{
    tasks::is_overdue,
    term::TICK_PERIOD_MS,
    ui::{
        animate::{Animation, AnimationDirection, AnimationType},
        focuslist::{
            FocusListItem,
            state::{FocusListPosition, FocusListState},
        },
        focusmode::N_TICKS,
        utils,
    },
};

const EXPLODE_MS: u64 = TICK_PERIOD_MS * N_TICKS;
const MOTION_DURATION_MS: u64 = TICK_PERIOD_MS * 2;

pub fn create_list_item(task: &Arc<Task>) -> FocusListItem<'static> {
    let now = Utc::now();

    let line1 = Line::from("");
    let line2 = Line::from(task.title.clone());
    // let line3 = if let Some(date_str) = format_date(&task.due_date, task.is_all_day, is_today) {
    let datetime_str = utils::format_datetime(task.due_date, task.is_all_day);

    // // Add repeat flag if present
    // let repeat_flag = if !task.repeat_flag.is_empty() {
    //     Some(task.repeat_flag.clone())
    // } else {
    //     None
    // };
    // let repeat_str = format_repeat_flag(&repeat_flag);

    let line3 = {
        // let mut spans = vec![Span::from(datetime_str)];
        let span = Span::from(datetime_str);
        // if let Some(repeat) = repeat_str {
        //     spans.push(Span::from(" • "));
        //     spans.push(Span::from(repeat));
        // }
        // let mut line = Line::from(spans);
        let mut line = Line::from(span);
        if is_overdue(now, task) {
            line = line.style(Style::default().fg(Color::Red).dim());
        } else {
            line = line.style(Style::default().dim());
        }
        line
    };
    let mut item = FocusListItem::new(vec![line1, line2, line3]);
    match task.priority {
        TaskPriority::High => item = item.with_border_color(Color::Red),
        TaskPriority::Medium => item = item.with_border_color(Color::Yellow),
        TaskPriority::Low => item = item.with_border_color(Color::Blue),
        TaskPriority::None => {}
    }
    item
}

pub fn render_no_tasks(f: &mut Frame, area: Rect) {
    let r = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Fill(1),
                Constraint::Length(5),
                Constraint::Fill(1),
            ]
            .as_ref(),
        )
        .split(area)[1];
    let r = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Fill(1),
                Constraint::Length(30),
                Constraint::Fill(1),
            ]
            .as_ref(),
        )
        .split(r)[1];
    f.render_widget(
        Paragraph::new("\nNo tasks available").centered().block(
            Block::new()
                .borders(Borders::ALL)
                .border_set(BorderType::Rounded.to_border_set()),
        ),
        r,
    );
}

fn translate_from_animation(translate_y: i32, duration: Duration) -> Animation {
    let anim = AnimationType::TranslateFrom {
        x: 0,
        y: translate_y,
    };
    Animation::new(anim, duration)
}

fn translate_to_animation(translate_y: i32, duration: Duration) -> Animation {
    let anim = AnimationType::TranslateTo {
        x: 0,
        y: translate_y,
    };
    Animation::new(anim, duration)
}

fn focus_from_animation(translate_y: i32, resize_horizontal: i32, duration: Duration) -> Animation {
    let anim = vec![
        AnimationType::TranslateFrom {
            x: 0,
            y: translate_y,
        },
        AnimationType::ResizeFrom {
            dir: AnimationDirection::Horizontal,
            amount: resize_horizontal,
        },
    ];
    Animation::new(AnimationType::Composite(anim), duration)
}

fn fade_in(effects: &mut EffectManager<()>, area_ref: RefRect, duration: Duration) {
    effects.add_effect(fx::dynamic_area(
        area_ref,
        fx::fade_from_fg(
            Color::Rgb(25, 25, 25),
            EffectTimer::new(duration.into(), Interpolation::SineOut),
        ),
    ));
}

fn fade_out(effects: &mut EffectManager<()>, area_ref: RefRect, duration: Duration) {
    effects.add_effect(fx::dynamic_area(
        area_ref,
        fx::fade_to_fg(
            Color::Rgb(25, 25, 25),
            EffectTimer::new(duration.into(), Interpolation::SineOut),
        ),
    ));
}

fn explode(effects: &mut EffectManager<()>, area_ref: RefRect, duration: Duration) {
    let timer = EffectTimer::new(duration.into(), Interpolation::SineOut);
    let c = Color::Rgb(25, 25, 25);
    let fx = fx::dynamic_area(
        area_ref,
        fx::parallel(&[fx::explode(2.0, 2.0, timer), fx::paint_bg(c, timer)]),
    );
    effects.add_effect(fx);
}

pub fn animate_scroll_down(list_state: &mut FocusListState, effects: &mut EffectManager<()>) {
    let duration = Duration::from_millis(MOTION_DURATION_MS);

    // animate the focused (center) item coming from next position (bottom)
    let translate_from_and_grow = focus_from_animation(10, -10, duration);
    list_state.start_animation(translate_from_and_grow, FocusListPosition::Focused);

    // animate the previous (top) item coming from focused position (center)
    let translate_from_and_shrink = focus_from_animation(10, 10, duration);
    list_state.start_animation(translate_from_and_shrink, FocusListPosition::Prev);

    // animate the next (bottom) item coming from the bottom edge and fade in
    let translate_from = translate_from_animation(10, duration);
    list_state.start_animation(translate_from, FocusListPosition::Next);
    if let Some(area_ref) = list_state.get_area_ref(FocusListPosition::Next) {
        fade_in(effects, area_ref, duration);
    }

    // animate the previous (top) item going to top edge and fade out
    let translate_to = translate_to_animation(-10, duration);
    list_state.start_animation(translate_to, FocusListPosition::PrevPrev);
    if let Some(area_ref) = list_state.get_area_ref(FocusListPosition::PrevPrev) {
        fade_out(effects, area_ref, duration);
    }
}

pub fn animate_scroll_up(list_state: &mut FocusListState, effects: &mut EffectManager<()>) {
    let duration = Duration::from_millis(MOTION_DURATION_MS);

    // animate the focused (center) item coming from previous position (top)
    let translate_from_and_grow = focus_from_animation(-10, -10, duration);
    list_state.start_animation(translate_from_and_grow, FocusListPosition::Focused);

    // animate the next (bottom) item coming from focused position (center)
    let translate_from_and_shrink = focus_from_animation(-10, 10, duration);
    list_state.start_animation(translate_from_and_shrink, FocusListPosition::Next);

    // animate the previous (top) item coming from top edge and fade in
    let translate_from = translate_from_animation(-10, duration);
    list_state.start_animation(translate_from, FocusListPosition::Prev);
    if let Some(area_ref) = list_state.get_area_ref(FocusListPosition::Prev) {
        fade_in(effects, area_ref, duration);
    }

    // animate the next (bottom) item going to bottom edge and fade out
    let translate_to = translate_to_animation(10, duration);
    list_state.start_animation(translate_to, FocusListPosition::NextNext);
    if let Some(area_ref) = list_state.get_area_ref(FocusListPosition::NextNext) {
        fade_out(effects, area_ref, duration);
    }
}

pub fn animate_shift_down(list_state: &mut FocusListState, effects: &mut EffectManager<()>) {
    let duration = Duration::from_millis(MOTION_DURATION_MS);

    // animate the focused (center) item coming from previous position (top)
    let translate_from_and_grow = focus_from_animation(-10, -10, duration);
    list_state.start_animation(translate_from_and_grow, FocusListPosition::Focused);

    // animate the previous (top) item coming from the top edge and fade in
    let translate_from = translate_from_animation(-10, duration);
    list_state.start_animation(translate_from, FocusListPosition::Prev);
    if let Some(area_ref) = list_state.get_area_ref(FocusListPosition::Prev) {
        fade_in(effects, area_ref, duration);
    }
}

pub fn animate_shift_up(list_state: &mut FocusListState, effects: &mut EffectManager<()>) {
    let duration = Duration::from_millis(MOTION_DURATION_MS);

    // animate the focused (center) item coming from next position (bottom)
    let translate_from_and_grow = focus_from_animation(10, -10, duration);
    list_state.start_animation(translate_from_and_grow, FocusListPosition::Focused);

    // animate the next (bottom) item coming from the bottom edge and fade in
    let translate_from = translate_from_animation(10, duration);
    list_state.start_animation(translate_from, FocusListPosition::Next);
    if let Some(area_ref) = list_state.get_area_ref(FocusListPosition::Next) {
        fade_in(effects, area_ref, duration);
    }
}

pub fn animate_completion(list_state: &mut FocusListState, effects: &mut EffectManager<()>) {
    // animate explosion of focused item
    let duration = Duration::from_millis(EXPLODE_MS);
    if let Some(area_ref) = list_state.get_area_ref(FocusListPosition::Focused) {
        explode(effects, area_ref, duration);
    }
}
