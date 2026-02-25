use app_core::{ActionContext, SavedAction, TimesOfDay};
use chrono::NaiveTime;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    progress::Progress,
    switch::Switch,
    v_flex,
};
use uuid::Uuid;

use crate::{components::popover::popover, stores::DatabaseStore};

const FIBONACCI_DURATIONS: &[u32] = &[1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144];

fn energy_label(value: i8) -> &'static str {
    match value {
        -2 => "Very draining",
        -1 => "Draining",
        0 => "Neutral",
        1 => "Energizing",
        2 => "Very energizing",
        _ => "Unknown",
    }
}

fn attention_label(value: u8) -> &'static str {
    match value {
        1 => "Autopilot",
        2 => "Low focus",
        3 => "Moderate focus",
        4 => "High focus",
        5 => "Deep focus",
        _ => "Unknown",
    }
}

fn transition_label(value: u8) -> &'static str {
    match value {
        1 => "Just do it",
        2 => "Easy start",
        3 => "Some effort",
        4 => "Needs setup",
        5 => "Hard to begin",
        _ => "Unknown",
    }
}

fn importance_label(value: u8) -> &'static str {
    match value {
        1 => "Nice to do",
        2 => "Somewhat important",
        3 => "Important",
        4 => "Very important",
        5 => "Critical",
        _ => "Unknown",
    }
}

fn duration_display(minutes: u32) -> String {
    if minutes < 60 {
        format!("{}m", minutes)
    } else {
        let hours = minutes / 60;
        let remaining = minutes % 60;
        if remaining == 0 {
            format!("{}h", hours)
        } else {
            format!("{}h{}m", hours, remaining)
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ActionEditorPage {
    General,
    Metadata,
    Preferences,
}

impl ActionEditorPage {
    fn title(&self) -> &'static str {
        match self {
            ActionEditorPage::General => "Basics",
            ActionEditorPage::Metadata => "Characteristics",
            ActionEditorPage::Preferences => "Preferences",
        }
    }

    fn progress(&self) -> f32 {
        match self {
            ActionEditorPage::General => 5.,
            ActionEditorPage::Metadata => 50.,
            ActionEditorPage::Preferences => 100.,
        }
    }
}

pub struct ActionEditor {
    pub focus_handle: FocusHandle,
    database_store: Entity<DatabaseStore>,
    current_page: ActionEditorPage,

    action_id: Option<Uuid>,
    title_input: Entity<InputState>,
    content_input: Entity<InputState>,
    pending_title: Option<String>,
    pending_content: Option<String>,

    // ActionContext fields
    energy_rate: Option<i8>,
    attention_level: Option<u8>,
    transition_difficulty: Option<u8>,
    importance: Option<u8>,

    // Preferred time of day
    preferred_times: TimesOfDay,
}

impl Focusable for ActionEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ActionEditor {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        cx.focus_self(window);

        let title_input = cx.new(|cx| InputState::new(window, cx).placeholder("What needs doing?"));
        let content_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Any details? (optional)"));

        Self {
            focus_handle,
            database_store,
            current_page: ActionEditorPage::General,

            action_id: None,
            title_input,
            content_input,
            pending_title: None,
            pending_content: None,

            energy_rate: None,
            attention_level: None,
            transition_difficulty: None,
            importance: None,

            preferred_times: TimesOfDay::empty(),
        }
    }

    pub fn load_action(&mut self, action_id: Uuid, cx: &mut Context<Self>) {
        let action = {
            let db_store = self.database_store.read(cx);
            db_store.get_saved_action(action_id).cloned()
        };

        if let Some(action) = action {
            self.action_id = Some(action.id);
            self.pending_title = Some(action.title.clone());
            self.pending_content = action.content.clone();

            self.energy_rate = action.context.energy_rate;
            self.attention_level = action.context.attention_level;
            self.transition_difficulty = action.context.transition_difficulty;
            self.importance = action.context.importance;

            if let Some(naive_time) = action.target_time {
                self.preferred_times = TimesOfDay::from(naive_time);
            } else {
                self.preferred_times = TimesOfDay::empty();
            }

            cx.notify();
        }
    }

    fn apply_pending_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(title) = self.pending_title.take() {
            self.title_input.update(cx, |input, cx| {
                input.set_value(title, window, cx);
            });
        }
        if let Some(content) = self.pending_content.take() {
            self.content_input.update(cx, |input, cx| {
                input.set_value(content, window, cx);
            });
        }
    }

    fn build_action(&self, cx: &App) -> SavedAction {
        let title = self.title_input.read(cx).value().to_string();
        let content = {
            let value = self.content_input.read(cx).value().to_string();
            if value.is_empty() { None } else { Some(value) }
        };

        // Resolve preferred_times to a single NaiveTime (the start of the first set period).
        let target_time = times_of_day_to_naive_time(self.preferred_times);

        let context = ActionContext {
            energy_rate: self.energy_rate,
            attention_level: self.attention_level,
            transition_difficulty: self.transition_difficulty,
            importance: self.importance,
        };

        let mut action = SavedAction::new(title);
        action.id = self.action_id.unwrap_or(action.id);
        action.content = content;
        action.target_time = target_time;
        action.context = context;
        action
    }

    fn save_action(&mut self, cx: &mut Context<Self>) {
        let action = self.build_action(cx);
        if action.title.trim().is_empty() {
            return;
        }
        self.database_store.update(cx, |store, cx| {
            store.upsert_saved_action(action, cx);
        });
    }

    fn delete_action(&mut self, cx: &mut Context<Self>) {
        if let Some(action_id) = self.action_id {
            self.database_store.update(cx, |store, cx| {
                store.delete_saved_action(action_id, cx);
            });
        }
    }

    fn toggle_time_of_day(&mut self, flag: TimesOfDay) {
        self.preferred_times.toggle(flag);
    }

    fn render_general_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().w_full().gap_6().child(
            v_flex()
                .gap_2()
                .child(Label::new("Notes").text_color(cx.theme().muted_foreground))
                .child(Input::new(&self.content_input).w_full().h(px(80.0))),
        )
    }

    fn render_metadata_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_5()
            .child(self.render_energy_selector(cx))
            .child(self.render_attention_selector(cx))
            .child(self.render_transition_selector(cx))
            .child(self.render_importance_selector(cx))
    }

    fn render_preferences_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_6()
            .child(self.render_time_of_day_selector(cx))
    }

    fn render_energy_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_label = self
            .energy_rate
            .map(|v| energy_label(v).to_string())
            .unwrap_or_else(|| "not set".to_string());

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Label::new("How draining is this?").text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(SharedString::from(current_label))
                            .text_color(cx.theme().foreground),
                    ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .children((-2i8..=2i8).map(|value| {
                        let is_selected = self.energy_rate == Some(value);
                        let display: SharedString = format!("{:+}", value).into();
                        let id: SharedString = format!("energy-{}", value).into();

                        Button::new(id)
                            .label(display)
                            .small()
                            .rounded_full()
                            .map(|button| {
                                if is_selected {
                                    button.primary()
                                } else {
                                    button.ghost()
                                }
                            })
                            .tooltip(SharedString::from(energy_label(value).to_string()))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                if this.energy_rate == Some(value) {
                                    this.energy_rate = None;
                                } else {
                                    this.energy_rate = Some(value);
                                }
                                cx.notify();
                            }))
                    })),
            )
    }

    fn render_attention_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_label = self
            .attention_level
            .map(|v| attention_label(v).to_string())
            .unwrap_or_else(|| "not set".to_string());

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Label::new("How much focus does this need?")
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(SharedString::from(current_label))
                            .text_color(cx.theme().foreground),
                    ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .children((1u8..=5u8).map(|value| {
                        let is_selected = self.attention_level == Some(value);
                        let display: SharedString = format!("{}", value).into();
                        let id: SharedString = format!("attention-{}", value).into();

                        Button::new(id)
                            .label(display)
                            .small()
                            .rounded_full()
                            .map(|button| {
                                if is_selected {
                                    button.primary()
                                } else {
                                    button.ghost()
                                }
                            })
                            .tooltip(SharedString::from(attention_label(value).to_string()))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                if this.attention_level == Some(value) {
                                    this.attention_level = None;
                                } else {
                                    this.attention_level = Some(value);
                                }
                                cx.notify();
                            }))
                    })),
            )
    }

    fn render_transition_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_label = self
            .transition_difficulty
            .map(|v| transition_label(v).to_string())
            .unwrap_or_else(|| "not set".to_string());

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Label::new("How hard is it to start?")
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(SharedString::from(current_label))
                            .text_color(cx.theme().foreground),
                    ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .children((1u8..=5u8).map(|value| {
                        let is_selected = self.transition_difficulty == Some(value);
                        let display: SharedString = format!("{}", value).into();
                        let id: SharedString = format!("transition-{}", value).into();

                        Button::new(id)
                            .label(display)
                            .small()
                            .rounded_full()
                            .map(|button| {
                                if is_selected {
                                    button.primary()
                                } else {
                                    button.ghost()
                                }
                            })
                            .tooltip(SharedString::from(transition_label(value).to_string()))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                if this.transition_difficulty == Some(value) {
                                    this.transition_difficulty = None;
                                } else {
                                    this.transition_difficulty = Some(value);
                                }
                                cx.notify();
                            }))
                    })),
            )
    }

    fn render_importance_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_label = self
            .importance
            .map(|v| importance_label(v).to_string())
            .unwrap_or_else(|| "not set".to_string());

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Label::new("How important is this?")
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(SharedString::from(current_label))
                            .text_color(cx.theme().foreground),
                    ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .children((1u8..=5u8).map(|value| {
                        let is_selected = self.importance == Some(value);
                        let display: SharedString = format!("{}", value).into();
                        let id: SharedString = format!("importance-{}", value).into();

                        Button::new(id)
                            .label(display)
                            .small()
                            .rounded_full()
                            .map(|button| {
                                if is_selected {
                                    button.primary()
                                } else {
                                    button.ghost()
                                }
                            })
                            .tooltip(SharedString::from(importance_label(value).to_string()))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                if this.importance == Some(value) {
                                    this.importance = None;
                                } else {
                                    this.importance = Some(value);
                                }
                                cx.notify();
                            }))
                    })),
            )
    }

    fn render_time_of_day_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let time_flags: &[(TimesOfDay, &str, IconName)] = &[
            (TimesOfDay::EARLY_MORNING, "Early morning", IconName::Sun),
            (TimesOfDay::MORNING, "Morning", IconName::Sun),
            (TimesOfDay::MIDDAY, "Midday", IconName::Sun),
            (TimesOfDay::AFTERNOON, "Afternoon", IconName::Sun),
            (TimesOfDay::EVENING, "Evening", IconName::Moon),
            (TimesOfDay::NIGHT, "Night", IconName::Moon),
            (TimesOfDay::LATE_NIGHT, "Late night", IconName::Moon),
        ];

        v_flex()
            .gap_3()
            .child(
                Label::new("When do you prefer to do this?")
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Label::new("Select all that apply")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                h_flex().gap_2().flex_wrap().children(
                    time_flags
                        .iter()
                        .map(|(flag, label, icon)| {
                            let flag = *flag;
                            let is_selected = self.preferred_times.contains(flag);
                            let label: SharedString = (*label).into();
                            let icon = icon.clone();
                            let id: SharedString = format!("time-{:?}", flag).into();

                            Button::new(id)
                                .icon(icon)
                                .label(label)
                                .map(|button| {
                                    if is_selected {
                                        button.primary()
                                    } else {
                                        button.ghost()
                                    }
                                })
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.toggle_time_of_day(flag);
                                    cx.notify();
                                }))
                        })
                        .collect::<Vec<_>>(),
                ),
            )
    }

    fn render_nav_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let on_first_page = self.current_page == ActionEditorPage::General;
        let on_last_page = self.current_page == ActionEditorPage::Preferences;
        let has_action_id = self.action_id.is_some();

        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .child(h_flex().gap_2().when(has_action_id, |this| {
                this.child(
                    Button::new("delete")
                        .icon(IconName::Delete)
                        .label("Delete")
                        .danger()
                        .ghost()
                        .on_click(cx.listener(|this, _event, window, cx| {
                            this.delete_action(cx);
                            window.dispatch_action(
                                Box::new(crate::components::popover::CloseOverlay),
                                cx,
                            );
                        })),
                )
            }))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("prev")
                            .icon(IconName::ArrowLeft)
                            .ghost()
                            .disabled(on_first_page)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                match this.current_page {
                                    ActionEditorPage::General => {}
                                    ActionEditorPage::Metadata => {
                                        this.current_page = ActionEditorPage::General;
                                    }
                                    ActionEditorPage::Preferences => {
                                        this.current_page = ActionEditorPage::Metadata;
                                    }
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Progress::new("editor-progress")
                            .w(px(120.0))
                            .value(self.current_page.progress()),
                    )
                    .child(if on_last_page {
                        Button::new("save")
                            .icon(IconName::Check)
                            .label("Save")
                            .primary()
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.save_action(cx);
                                window.dispatch_action(
                                    Box::new(crate::components::popover::CloseOverlay),
                                    cx,
                                );
                            }))
                    } else {
                        Button::new("next")
                            .icon(IconName::ArrowRight)
                            .ghost()
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                match this.current_page {
                                    ActionEditorPage::General => {
                                        this.current_page = ActionEditorPage::Metadata;
                                    }
                                    ActionEditorPage::Metadata => {
                                        this.current_page = ActionEditorPage::Preferences;
                                    }
                                    ActionEditorPage::Preferences => {}
                                }
                                cx.notify();
                            }))
                    }),
            )
    }
}

impl Render for ActionEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_pending_values(window, cx);

        let theme = cx.theme().clone();
        let page_content: AnyElement = match self.current_page {
            ActionEditorPage::General => self.render_general_page(cx).into_any_element(),
            ActionEditorPage::Metadata => self.render_metadata_page(cx).into_any_element(),
            ActionEditorPage::Preferences => self.render_preferences_page(cx).into_any_element(),
        };

        v_flex()
            .size_full()
            .p_4()
            .gap_4()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        Input::new(&self.title_input)
                            .w_full()
                            .text_2xl()
                            .appearance(false),
                    )
                    .child(
                        Label::new(SharedString::from(self.current_page.title().to_string()))
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .id("action-editor-scroll")
                    .overflow_y_scroll()
                    .child(page_content),
            )
            .child(self.render_nav_footer(cx))
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Converts a `TimesOfDay` bitflag set to a representative `NaiveTime` (the start of
/// the earliest period that is set). Returns `None` if no flags are set.
fn times_of_day_to_naive_time(times: TimesOfDay) -> Option<NaiveTime> {
    if times.contains(TimesOfDay::EARLY_MORNING) {
        NaiveTime::from_hms_opt(4, 0, 0)
    } else if times.contains(TimesOfDay::MORNING) {
        NaiveTime::from_hms_opt(7, 0, 0)
    } else if times.contains(TimesOfDay::MIDDAY) {
        NaiveTime::from_hms_opt(11, 0, 0)
    } else if times.contains(TimesOfDay::AFTERNOON) {
        NaiveTime::from_hms_opt(13, 0, 0)
    } else if times.contains(TimesOfDay::EVENING) {
        NaiveTime::from_hms_opt(16, 0, 0)
    } else if times.contains(TimesOfDay::NIGHT) {
        NaiveTime::from_hms_opt(20, 0, 0)
    } else if times.contains(TimesOfDay::LATE_NIGHT) {
        NaiveTime::from_hms_opt(0, 0, 0)
    } else {
        None
    }
}
