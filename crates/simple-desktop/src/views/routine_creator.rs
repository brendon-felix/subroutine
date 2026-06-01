use std::{rc::Rc, time::Duration};

use chrono::Local;
use gpui::{
    App, AppContext as _, Context, DefiniteLength, Entity, FocusHandle, Focusable, KeyBinding,
    Pixels, Point, ScrollHandle, SharedString, Size, Window, actions, div, prelude::*, px, rems,
};
use gpui_component::{
    ActiveTheme, Colorize, Icon, IconName, Sizable, VirtualListScrollHandle, WindowExt,
    animation::ease_out_cubic,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    notification::NotificationType,
    scroll::{ScrollableElement, ScrollbarHandle},
    v_flex, v_virtual_list,
};
use simple_core::{Routine, RoutineStep};
use simple_parser::{
    BuildTarget, BuiltEntity, ParseDraft, RecurrenceSpec, WeekdaySet, build_entity,
    parse::parse_routine_step_input, parse_action_input,
};

use gpui_transitions::WindowUseTransition;

use crate::{
    AppIcon,
    components::{CloseOverlay, OverlayPosition, overlay},
    stores::AppDatabaseStore,
};

const SCROLL_DURATION: Duration = Duration::from_millis(200);

const ITEM_HEIGHT: Pixels = px(12. * 4.);

/// Format a day number as an ordinal string: 1 → "1st", 2 → "2nd", etc.
fn ordinal(n: u32) -> String {
    let suffix = match n % 100 {
        11 | 12 | 13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{n}{suffix}")
}

/// Format a [`WeekdaySet`] as a short human-readable string.
fn format_weekday_set(set: &WeekdaySet) -> String {
    use chrono::Weekday::*;

    // Named combinations first.
    if *set == WeekdaySet::every_day() {
        return "daily".into();
    }
    if *set == WeekdaySet::weekdays() {
        return "weekdays".into();
    }
    if *set == WeekdaySet::weekends() {
        return "weekends".into();
    }

    // Fall back to comma-joined abbreviated names in Mon→Sun order.
    let names: Vec<&str> = [Mon, Tue, Wed, Thu, Fri, Sat, Sun]
        .iter()
        .filter(|&&d| set.contains(d))
        .map(|d| match d {
            Mon => "Mon",
            Tue => "Tue",
            Wed => "Wed",
            Thu => "Thu",
            Fri => "Fri",
            Sat => "Sat",
            Sun => "Sun",
        })
        .collect();

    names.join(", ")
}

fn format_recurrence(spec: &RecurrenceSpec) -> String {
    match spec {
        RecurrenceSpec::EveryDays(1) => "daily".into(),
        RecurrenceSpec::EveryDays(7) => "weekly".into(),
        RecurrenceSpec::EveryDays(n) => format!("every {n} days"),
        RecurrenceSpec::EveryWeeks(1) => "weekly".into(),
        RecurrenceSpec::EveryWeeks(n) => format!("every {n} weeks"),
        RecurrenceSpec::EveryMonths(1) => "monthly".into(),
        RecurrenceSpec::EveryMonths(3) => "quarterly".into(),
        RecurrenceSpec::EveryMonths(n) => format!("every {n} months"),
        RecurrenceSpec::EveryYears(1) => "yearly".into(),
        RecurrenceSpec::EveryYears(n) => format!("every {n} years"),
        RecurrenceSpec::OnMonthDay(day) => format!("the {}", ordinal(*day)),
        RecurrenceSpec::OnWeekdays(set) => format_weekday_set(set),
    }
}

pub struct RoutineCreator {
    pub focus_handle: FocusHandle,
    scroll_handle: VirtualListScrollHandle,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    db_store: Entity<AppDatabaseStore>,

    title_input: Entity<InputState>,
    step_input: Entity<InputState>,

    save: bool,

    current_title: String,
    current_step_input: String,
    current_step_draft: Option<ParseDraft>,
    current_steps: Vec<RoutineStep>,

    batch_mode: bool,

    pending_scroll_item: Option<usize>,

    _subscriptions: Vec<gpui::Subscription>,
}

impl RoutineCreator {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scroll_handle = VirtualListScrollHandle::new();
        let item_sizes = Rc::new(vec![]);
        let db_store = AppDatabaseStore::global(cx);

        let title_input = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("Routine Name");
            state.focus(window, cx);
            state
        });
        let step_input = cx.new(|cx| InputState::new(window, cx).placeholder("Routine Step"));

        // cx.bind_keys([KeyBinding::new("cmd-b", ToggleBatchMode, None)]);

        let mut subscriptions = Vec::new();

        // subscriptions.push(
        //     cx.subscribe(&title_input, |this, _, event: &InputEvent, cx| {
        //         if let InputEvent::Change = event {
        //             let value = this.title_input.read(cx).value().to_string();
        //             this.current_title = value.trim().to_string();
        //             // Re-parse eagerly so the draft stays fresh.
        //             cx.notify();
        //         }
        //     }),
        // );
        subscriptions.push(cx.subscribe_in(
            &title_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.submit_title(window, cx);
                }
            },
        ));

        subscriptions.push(
            cx.subscribe(&step_input, |this, _, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    let value = this.step_input.read(cx).value().to_string();
                    this.current_step_input = value.trim().to_string();
                    this.current_step_draft =
                        parse_routine_step_input(&this.current_step_input).ok();
                    cx.notify();
                }
            }),
        );
        subscriptions.push(cx.subscribe_in(
            &step_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.submit_step(window, cx);
                }
            },
        ));

        Self {
            focus_handle: cx.focus_handle(),
            scroll_handle,
            item_sizes,
            db_store,
            title_input,
            step_input,
            save: false,
            current_title: String::new(),
            current_steps: Vec::new(),
            current_step_input: String::new(),
            current_step_draft: None,
            batch_mode: false,
            pending_scroll_item: None,
            _subscriptions: subscriptions,
        }
    }

    fn refresh_item_sizes(&mut self, height: Pixels) {
        if let Some(vec) = Rc::get_mut(&mut self.item_sizes) {
            vec.clear();
            vec.extend((0..self.current_steps.len()).map(|_| Size::new(Pixels::default(), height)));
        } else {
            let num_items = self.current_steps.len();
            self.item_sizes = Rc::new(
                (0..num_items)
                    .map(|_| Size::new(Pixels::default(), height))
                    .collect(),
            );
        }
    }

    fn build_step(&self) -> (RoutineStep, Vec<String>) {
        // Fast path: if we already have a fresh draft use it, otherwise
        // attempt a parse right now (covers the edge case where the draft
        // was never set, e.g. the user pasted text without triggering Change).
        let (draft, mut warnings) = match self
            .current_step_draft
            .clone()
            .or_else(|| parse_routine_step_input(&self.current_step_input).ok())
        {
            Some(draft) => {
                let w = draft.warnings.clone();
                (Some(draft), w)
            }
            None => (None, Vec::new()),
        };

        let step = if let Some(ref draft) = draft {
            // build_entity can only fail for events (requires a `when`), so
            // unwrap_or_else here is safe for the RoutineStep path.
            match build_entity(draft, BuildTarget::RoutineStep) {
                Ok(BuiltEntity::RoutineStep(s)) => s,
                _ => {
                    // Shouldn't happen for RoutineStep targets, but fall back
                    // gracefully.
                    warnings.push(
                        "Parser produced an unexpected entity type; using bare title.".into(),
                    );
                    RoutineStep::new(&self.current_step_input)
                }
            }
        } else {
            // If parsing failed, fall back to a bare title step so the user
            // doesn't lose their input.
            RoutineStep::new(&self.current_step_input)
        };

        (step, warnings)
    }

    fn build(&self) -> Routine {
        let (steps, _) = if !self.current_step_input.is_empty() {
            let (step, warnings) = self.build_step();
            (vec![step], warnings)
        } else {
            (self.current_steps.clone(), Vec::new())
        };
        let routine = Routine::new(&self.current_title).with_steps(steps);
        routine
    }

    fn submit_title(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.title_input.read(cx).value().to_string();
        self.current_title = value.trim().to_string();
        // Re-parse eagerly so the draft stays fresh.
        if self.current_title.is_empty() {
            return;
        }
        // self.step_input.focus_handle(cx).focus(window, cx);
        cx.focus_view(&self.step_input, window);
        // if !self.current_step_input.is_empty() {
        //     self.submit_step(window, cx);
        // } else {
        //     self.submit(window, cx);
        // }
        cx.notify();
    }

    fn submit_step(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.current_step_input.is_empty() {
            return;
        }

        let (step, parse_warnings) = self.build_step();

        // Surface any parse warnings first.
        for warning in &parse_warnings {
            window.push_notification(
                (
                    NotificationType::Warning,
                    SharedString::from(warning.clone()),
                ),
                cx,
            );
        }

        self.current_steps.push(step);
        self.refresh_item_sizes(ITEM_HEIGHT);
        self.pending_scroll_item = Some(self.current_steps.len() - 1);
        self.reset_step_input(window, cx);
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.current_title.is_empty() {
            return;
        }

        let routine = self.build();

        // add routine
        self.db_store.update(cx, |store, cx| {
            store.upsert_routine(routine, cx);
        });

        // if !self.batch_mode {
        //     window.dispatch_action(Box::new(CloseOverlay), cx);
        // } else {
        //     self.reset(window, cx);
        // }
        window.dispatch_action(Box::new(CloseOverlay), cx);
    }

    fn reset_step_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.current_step_draft = None;
        self.current_step_input.clear();
        self.step_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.current_title.clear();
        self.current_step_draft = None;
        self.title_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    fn render_title_input(&self, cx: &Context<Self>) -> impl IntoElement {
        Input::new(&self.title_input)
            .h_12()
            .w_full()
            .text_size(rems(1.5))
            .line_height(rems(2.0))
            .focus_bordered(true)
            .appearance(false)
    }

    fn render_step_input(&self, cx: &Context<Self>) -> impl IntoElement {
        Input::new(&self.step_input)
            .px_0()
            // .pr_0()
            // .pl_6()
            .w_full()
            .h_12()
            .items_center()
            .text_size(rems(1.))
            .line_height(rems(1.25))
            .focus_bordered(true)
            .appearance(false)
    }

    fn render_steps(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scroll_transition = window
            .use_keyed_transition("step-scroll", cx, SCROLL_DURATION, |_, _| {
                self.scroll_handle.offset().y
            })
            .with_easing(ease_out_cubic);

        if let Some(item_ix) = self.pending_scroll_item.take() {
            let viewport_height = self.scroll_handle.bounds().size.height;
            let n = item_ix + 1;
            let total_height = ITEM_HEIGHT * n as f32;
            let target = -(total_height - viewport_height).max(px(0.));
            scroll_transition.jump_to(self.scroll_handle.offset().y, cx);
            scroll_transition.update(cx, |offset, _| *offset = target);
        }

        if scroll_transition.evaluate_delta(cx) != 1.0 {
            let offset = *scroll_transition.evaluate(window, cx);
            self.scroll_handle.set_offset(Point::new(px(0.), offset));
        }

        v_virtual_list(
            cx.entity(),
            "timeline",
            self.item_sizes.clone(),
            move |view, visible_range, _, cx| {
                visible_range
                    .filter_map(|i| {
                        view.current_steps.get(i).cloned().map(|step| {
                            h_flex()
                                .size_full()
                                .when(i != view.current_steps.len() - 1, |this| {
                                    this.border_b_1().border_color(cx.theme().border)
                                })
                                .justify_between()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            Label::new(format!("{}", i + 1))
                                                .w_6()
                                                .text_color(cx.theme().muted_foreground),
                                        )
                                        .child(Label::new(&step.title).line_height(rems(1.25))),
                                )
                                .child(
                                    h_flex()
                                        .h_full()
                                        .gap_2()
                                        .when_some(step.duration, |this, duration| {
                                            this.child(
                                                Label::new(format!("{}m", duration.num_minutes()))
                                                    .line_height(rems(1.25))
                                                    .text_color(cx.theme().muted_foreground),
                                            )
                                        })
                                        .child(
                                            Button::new(("remove-step", i as u32))
                                                .ghost()
                                                .size_6()
                                                .child(Icon::new(IconName::CircleX).size_3())
                                                .on_click(cx.listener(
                                                    move |view, _, window, cx| {
                                                        view.current_steps.remove(i);
                                                        view.refresh_item_sizes(ITEM_HEIGHT);
                                                        cx.notify();
                                                    },
                                                )),
                                        ),
                                )
                        })
                    })
                    .collect()
            },
        )
        .track_scroll(&self.scroll_handle)
        .border_y_1()
        .border_color(cx.theme().border)
        // .max_h_48()
        .h_48()
    }
}

impl Focusable for RoutineCreator {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.title_input.focus_handle(cx)
    }
}

impl Render for RoutineCreator {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        // let can_submit = !self.current_title.is_empty();

        // Build a one-line preview string from the current draft so the user
        // can see at a glance what the parser understood.
        let preview_text: Option<SharedString> = self.current_step_draft.as_ref().map(|draft| {
            let mut parts: Vec<String> = Vec::new();

            if let Some(when) = &draft.when {
                use simple_parser::ast::WhenSpec;
                match when {
                    WhenSpec::DateTime(dt) => {
                        let local = dt.with_timezone(&Local);
                        // e.g. "Fri Dec 20 3:00pm" or "Fri Dec 20 12:00pm"
                        let time_str = local.format("%-I:%M%P").to_string();
                        // Trim ":00" from whole-hour times → "3pm" instead of "3:00pm"
                        let time_str = if time_str.contains(":00") {
                            time_str.replace(":00", "")
                        } else {
                            time_str
                        };
                        parts.push(local.format(&format!("%a %b %-d {time_str}")).to_string());
                    }
                    WhenSpec::NaiveDate(date) => {
                        // Date-only — no time component yet (floating backlog date).
                        parts.push(date.format("%a %b %-d").to_string());
                    }
                }
            }
            if let Some(dur) = draft.duration {
                let total_mins = dur.num_minutes();
                if total_mins % 60 == 0 {
                    parts.push(format!("{}h", total_mins / 60));
                } else if total_mins >= 60 {
                    parts.push(format!("{}h {}m", total_mins / 60, total_mins % 60));
                } else {
                    parts.push(format!("{}m", total_mins));
                }
            }
            if let Some(rec) = &draft.recurrence {
                parts.push(format_recurrence(rec));
            }
            if let Some(loc) = &draft.location {
                parts.push(loc.clone());
            }
            if !draft.tags.is_empty() {
                parts.push(format!("{}", draft.tags.join(", ")));
            }
            if !draft.people.is_empty() {
                parts.push(format!("{}", draft.people.join(", ")));
            }
            if let Some(pri) = &draft.priority {
                parts.push(format!("{pri:?}"));
            }

            if parts.is_empty() {
                SharedString::from("")
            } else {
                // SharedString::from(parts.join("  ·  "))
                SharedString::from(parts.join(" "))
            }
        });

        let inner = v_flex().w(px(144. * 4.)).child(
            v_flex()
                .track_focus(&self.focus_handle)
                .bg(theme.background.mix_oklab(gpui::black(), 0.95).alpha(0.9))
                .text_color(theme.foreground)
                .border_1()
                .border_color(theme.border)
                // .rounded_full()
                .rounded(px(28.))
                .shadow_md()
                .on_any_mouse_down(|_event, _window, cx| {
                    cx.stop_propagation();
                })
                .px_4()
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .child(Icon::new(AppIcon::Repeat).large())
                        .child(self.render_title_input(cx)),
                )
                .when(!self.current_title.is_empty(), |this| {
                    this.child(self.render_steps(window, cx)).child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .child(div().w_6())
                            .child(self.render_step_input(cx)),
                    )
                })
                .when(false, |this| this),
        );

        overlay(
            inner,
            OverlayPosition::Top(DefiniteLength::Fraction(0.25).into()),
            // OverlayPosition::Center,
            cx,
        )
    }
}
