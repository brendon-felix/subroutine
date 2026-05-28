use chrono::Local;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, SharedString, Window,
    prelude::*, px, rems,
};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    notification::NotificationType,
    switch::Switch,
    v_flex,
};
use simple_parser::{
    BuildTarget, BuiltEntity, ParseDraft, RecurrenceSpec, WeekdaySet, build_entity,
    parse_event_input,
};

use crate::{
    components::popover::{CloseOverlay, popover},
    stores::AppDatabaseStore,
};

/// Format a [`RecurrenceSpec`] as a short human-readable string.
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

    if *set == WeekdaySet::every_day() {
        return "daily".into();
    }
    if *set == WeekdaySet::weekdays() {
        return "weekdays".into();
    }
    if *set == WeekdaySet::weekends() {
        return "weekends".into();
    }

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

pub struct EventCreator {
    pub focus_handle: FocusHandle,
    database_store: Entity<AppDatabaseStore>,

    title_input: Entity<InputState>,
    content_input: Entity<InputState>,

    save: bool,

    current_title: String,
    current_content: String,

    /// The most recent successful parse of `current_title`. Kept up to date
    /// on every change so we can show a live preview without re-parsing on
    /// submit.
    current_draft: Option<ParseDraft>,

    batch_mode: bool,

    _subscriptions: Vec<gpui::Subscription>,
}

impl EventCreator {
    pub fn new(
        database_store: Entity<AppDatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title_input = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("Event name");
            state.focus(window, cx);
            state
        });
        let content_input = cx.new(|cx| InputState::new(window, cx).placeholder("Description"));

        let mut subscriptions = Vec::new();

        subscriptions.push(
            cx.subscribe(&title_input, |this, _, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    let value = this.title_input.read(cx).value().to_string();
                    this.current_title = value.trim().to_string();
                    // Re-parse eagerly so the draft stays fresh.
                    this.current_draft = parse_event_input(&this.current_title).ok();
                    cx.notify();
                }
            }),
        );

        subscriptions.push(
            cx.subscribe(&content_input, |this, _, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    let value = this.content_input.read(cx).value().to_string();
                    this.current_content = value.trim().to_string();
                    cx.notify();
                }
            }),
        );

        // Submit on Enter in the title field.
        subscriptions.push(cx.subscribe_in(
            &title_input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.submit(window, cx);
                }
            },
        ));

        Self {
            focus_handle: cx.focus_handle(),
            database_store,
            title_input,
            content_input,
            save: false,
            current_title: String::new(),
            current_content: String::new(),
            current_draft: None,
            batch_mode: false,
            _subscriptions: subscriptions,
        }
    }

    /// Build an `Event` from the current input state.
    ///
    /// Strategy:
    ///   1. Try to parse the title with `simple_parser`.
    ///   2. On success, use `build_entity` to get a fully-populated `Event`.
    ///   3. Override `content` with whatever the user typed in the description
    ///      field (if non-empty), so explicit descriptions always win.
    ///   4. Apply the `save` toggle (ephemeral vs. saved).
    ///   5. Return the event together with any parser warnings so the caller
    ///      can surface them as notifications.
    fn build_event(&self) -> Result<(simple_core::Event, Vec<String>), String> {
        let (draft, warnings) = match self
            .current_draft
            .clone()
            .or_else(|| parse_event_input(&self.current_title).ok())
        {
            Some(draft) => {
                let w = draft.warnings.clone();
                (Some(draft), w)
            }
            None => (None, Vec::new()),
        };

        let draft = match draft {
            Some(d) => d,
            None => return Err("Could not parse event input.".into()),
        };

        let mut event = match build_entity(&draft, BuildTarget::Event) {
            Ok(BuiltEntity::Event(e)) => e,
            Ok(BuiltEntity::Action(_)) => {
                return Err("Parser produced an unexpected entity type.".into());
            }
            Err(e) => return Err(e.to_string()),
        };

        // The `save` checkbox controls ephemerality regardless of what the
        // parser inferred.
        event.saved = self.save;

        // An explicit description always takes precedence over the parsed one.
        if !self.current_content.is_empty() {
            event.content = Some(self.current_content.clone());
        }

        Ok((event, warnings))
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.current_title.is_empty() {
            return;
        }

        let (event, parse_warnings) = match self.build_event() {
            Ok(result) => result,
            Err(e) => {
                window.push_notification(
                    (
                        NotificationType::Error,
                        SharedString::from(format!("Could not create event: {e}")),
                    ),
                    cx,
                );
                return;
            }
        };

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

        // Add to the pipeline and show overlap warnings.
        self.database_store
            .update(cx, |store, cx| store.upsert_event(event, cx));

        // for warning in overlap_warnings {
        //     window.push_notification(
        //         (
        //             NotificationType::Warning,
        //             SharedString::from(format!(
        //                 "\"{}\" overlaps with \"{}\"",
        //                 warning.inserted_title, warning.conflicting_title
        //             )),
        //         ),
        //         cx,
        //     );
        // }

        if !self.batch_mode {
            window.dispatch_action(Box::new(CloseOverlay), cx);
        } else {
            self.reset(window, cx);
        }
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.current_title.clear();
        self.current_draft = None;
        self.title_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.current_content.clear();
        self.content_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }
}

impl Focusable for EventCreator {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.title_input.focus_handle(cx)
    }
}

impl Render for EventCreator {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let can_submit = self
            .current_draft
            .as_ref()
            .map_or(false, |d| d.when.is_some())
            && !self.current_title.is_empty();

        // Build a one-line preview string from the current draft so the user
        // can see at a glance what the parser understood.
        let preview_text: Option<SharedString> = self.current_draft.as_ref().map(|draft| {
            let mut parts: Vec<String> = Vec::new();

            if let Some(when) = &draft.when {
                use simple_parser::ast::WhenSpec;
                match when {
                    WhenSpec::DateTime(dt) => {
                        let local = dt.with_timezone(&Local);
                        let time_str = local.format("%-I:%M%P").to_string();
                        let time_str = if time_str.contains(":00") {
                            time_str.replace(":00", "")
                        } else {
                            time_str
                        };
                        parts.push(format!(
                            "🕐 {}",
                            local.format(&format!("%a %b %-d {time_str}"))
                        ));
                    }
                    WhenSpec::NaiveDate(date) => {
                        parts.push(format!("📅 {}", date.format("%a %b %-d")));
                    }
                }
            }
            if let Some(dur) = draft.duration {
                let total_mins = dur.num_minutes();
                if total_mins % 60 == 0 {
                    parts.push(format!("⏱ {}h", total_mins / 60));
                } else if total_mins >= 60 {
                    parts.push(format!("⏱ {}h {}m", total_mins / 60, total_mins % 60));
                } else {
                    parts.push(format!("⏱ {}m", total_mins));
                }
            }
            if let Some(rec) = &draft.recurrence {
                parts.push(format!("🔁 {}", format_recurrence(rec)));
            }
            if let Some(loc) = &draft.location {
                parts.push(format!("📍 {loc}"));
            }
            if !draft.tags.is_empty() {
                parts.push(format!("🏷 {}", draft.tags.join(", ")));
            }
            if !draft.people.is_empty() {
                parts.push(format!("👤 {}", draft.people.join(", ")));
            }
            if let Some(pri) = &draft.priority {
                parts.push(format!("❗ {pri:?}"));
            }

            if parts.is_empty() {
                SharedString::from("")
            } else {
                SharedString::from(parts.join("  ·  "))
            }
        });

        let inner = v_flex().h_full().w_128().pt_8().child(
            v_flex()
                .pt_2()
                .track_focus(&self.focus_handle)
                .bg(theme.group_box)
                .text_color(theme.group_box_foreground)
                .border_1()
                .border_color(theme.border)
                .rounded_xl()
                .shadow_xl()
                .on_any_mouse_down(|_event, _window, cx| {
                    cx.stop_propagation();
                })
                // Title input row
                .child(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .items_center()
                        .pr_4()
                        .child(
                            Input::new(&self.title_input)
                                .w_full()
                                .py_0()
                                .px_4()
                                .text_size(rems(1.5))
                                .line_height(rems(1.75))
                                .focus_bordered(true)
                                .appearance(false),
                        )
                        .child(
                            Checkbox::new("save")
                                .label("Save event")
                                .checked(self.save)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.save = !this.save;
                                    cx.notify();
                                })),
                        ),
                )
                // Live parse preview (only shown when there is something to show)
                .when(
                    preview_text
                        .as_ref()
                        .map(|s| !s.is_empty())
                        .unwrap_or(false),
                    |el| {
                        el.child(
                            h_flex()
                                .w_full()
                                .px_4()
                                .py(px(4.0))
                                .border_t_1()
                                .border_color(theme.border)
                                .child(
                                    Label::new(preview_text.unwrap_or_default())
                                        .text_xs()
                                        .text_color(theme.muted_foreground),
                                ),
                        )
                    },
                )
                // Description input
                .child(
                    Input::new(&self.content_input)
                        .w_full()
                        .py_0()
                        .px_4()
                        .text_size(rems(0.75))
                        .line_height(rems(0.75))
                        .focus_bordered(true)
                        .appearance(false),
                )
                // Options label row
                .child(
                    h_flex()
                        .w_full()
                        .gap_3()
                        .items_center()
                        .px(px(16.0))
                        .py(px(10.0))
                        .border_t_1()
                        .border_color(theme.border)
                        .child(
                            Label::new("Options")
                                .text_xs()
                                .font_semibold()
                                .text_color(theme.muted_foreground),
                        ),
                )
                // Footer row: batch mode + buttons
                .child(
                    h_flex()
                        .w_full()
                        .p_4()
                        .border_t_1()
                        .border_color(theme.border)
                        .justify_between()
                        .items_center()
                        .child(
                            Switch::new("batch-mode")
                                .label("Batch mode")
                                .checked(self.batch_mode)
                                .on_click(cx.listener(|this, checked, _, cx| {
                                    this.batch_mode = *checked;
                                    cx.notify();
                                })),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(Button::new("cancel").small().label("Cancel").on_click(
                                    |_, window, cx| {
                                        window.dispatch_action(Box::new(CloseOverlay), cx);
                                    },
                                ))
                                .child(
                                    Button::new("submit")
                                        .small()
                                        .primary()
                                        .label("Add event")
                                        .disabled(!can_submit)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.submit(window, cx);
                                        })),
                                ),
                        ),
                ),
        );

        popover(inner, cx)
    }
}
