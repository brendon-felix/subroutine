use database::Action;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
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

use crate::{components::popover::popover, stores::DatabaseStore};

const FIBONACCI_DURATIONS: &[i64] = &[1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144];

const ACTION_TYPES: &[&str] = &["task", "activity", "event", "habit"];

const TIMES_OF_DAY: &[&str] = &["morning", "afternoon", "evening", "night"];

const TIME_OF_DAY_ICONS: &[IconName] =
    &[IconName::Sun, IconName::Sun, IconName::Moon, IconName::Moon];

fn energy_label(value: i64) -> &'static str {
    match value {
        -5 => "Exhausting",
        -4 => "Very draining",
        -3 => "Draining",
        -2 => "Somewhat draining",
        -1 => "Slightly draining",
        0 => "Neutral",
        1 => "Slightly energizing",
        2 => "Somewhat energizing",
        3 => "Energizing",
        4 => "Very energizing",
        5 => "Invigorating",
        _ => "Unknown",
    }
}

fn attention_label(value: i64) -> &'static str {
    match value {
        1 => "Autopilot",
        2 => "Low focus",
        3 => "Moderate focus",
        4 => "High focus",
        5 => "Deep focus",
        _ => "Unknown",
    }
}

fn transition_label(value: i64) -> &'static str {
    match value {
        1 => "Just do it",
        2 => "Easy start",
        3 => "Some effort",
        4 => "Needs setup",
        5 => "Hard to begin",
        _ => "Unknown",
    }
}

fn enjoyment_label(value: i64) -> &'static str {
    match value {
        -5 => "Miserable",
        -4 => "Very unpleasant",
        -3 => "Unpleasant",
        -2 => "Somewhat unpleasant",
        -1 => "Slightly unpleasant",
        0 => "Neutral",
        1 => "Slightly pleasant",
        2 => "Somewhat enjoyable",
        3 => "Enjoyable",
        4 => "Very enjoyable",
        5 => "Delightful",
        _ => "Unknown",
    }
}

fn importance_label(value: i64) -> &'static str {
    match value {
        1 => "Nice to do",
        2 => "Somewhat important",
        3 => "Important",
        4 => "Very important",
        5 => "Critical",
        _ => "Unknown",
    }
}

fn duration_display(minutes: i64) -> String {
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

    action_id: Option<String>,
    title_input: Entity<InputState>,
    description_input: Entity<InputState>,
    pending_title: Option<String>,
    pending_description: Option<String>,

    action_type: String,
    duration_bucket: Option<i64>,
    energy_rate: Option<i64>,
    attention_level: Option<i64>,
    transition_difficulty: Option<i64>,
    enjoyment_after_start: Option<i64>,
    importance: Option<i64>,
    urgency_growth: bool,
    preferred_times: Vec<String>,
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

        let description_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Any details? (optional)"));

        Self {
            focus_handle,
            database_store,
            current_page: ActionEditorPage::General,

            action_id: None,
            title_input,
            description_input,
            pending_title: None,
            pending_description: None,

            action_type: "task".to_string(),
            duration_bucket: None,
            energy_rate: None,
            attention_level: None,
            transition_difficulty: None,
            enjoyment_after_start: None,
            importance: None,
            urgency_growth: false,
            preferred_times: Vec::new(),
        }
    }

    pub fn load_action(&mut self, action_id: &str, cx: &mut Context<Self>) {
        let action = {
            let db_store = self.database_store.read(cx);
            db_store.get_action(action_id).cloned()
        };

        if let Some(action) = action {
            self.action_id = Some(action.id.clone());
            self.pending_title = Some(action.title.clone());
            self.pending_description = action.description.clone();

            self.action_type = action.action_type.clone();
            self.duration_bucket = action.duration_bucket;
            self.energy_rate = action.energy_rate;
            self.attention_level = action.attention_level;
            self.transition_difficulty = action.transition_difficulty;
            self.enjoyment_after_start = action.enjoyment_after_start;
            self.importance = action.importance;
            self.urgency_growth = action.urgency_growth.unwrap_or(false);

            if let Some(ref times_json) = action.preferred_time_of_day {
                if let Ok(times) = serde_json::from_str::<Vec<String>>(times_json) {
                    self.preferred_times = times;
                }
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
        if let Some(description) = self.pending_description.take() {
            self.description_input.update(cx, |input, cx| {
                input.set_value(description, window, cx);
            });
        }
    }

    fn build_action(&self, cx: &App) -> Action {
        let title = self.title_input.read(cx).value().to_string();
        let description = {
            let value = self.description_input.read(cx).value().to_string();
            if value.is_empty() { None } else { Some(value) }
        };

        let preferred_time_of_day = if self.preferred_times.is_empty() {
            None
        } else {
            serde_json::to_string(&self.preferred_times).ok()
        };

        Action {
            id: self
                .action_id
                .clone()
                .unwrap_or_else(|| Action::default().id),
            action_type: self.action_type.clone(),
            title,
            description,
            duration_bucket: self.duration_bucket,
            energy_rate: self.energy_rate,
            attention_level: self.attention_level,
            transition_difficulty: self.transition_difficulty,
            enjoyment_after_start: self.enjoyment_after_start,
            importance: self.importance,
            urgency_growth: Some(self.urgency_growth),
            created_at: None,
            preferred_time_of_day,
            metadata: None,
        }
    }

    fn save_action(&mut self, cx: &mut Context<Self>) {
        let action = self.build_action(cx);
        if action.title.trim().is_empty() {
            return;
        }
        self.database_store.update(cx, |store, cx| {
            store.insert_action(action, cx);
        });
    }

    fn delete_action(&mut self, cx: &mut Context<Self>) {
        if let Some(action_id) = self.action_id.clone() {
            self.database_store.update(cx, |store, cx| {
                store.delete_action(action_id, cx);
            });
        }
    }

    fn toggle_preferred_time(&mut self, time: &str) {
        if let Some(index) = self.preferred_times.iter().position(|t| t == time) {
            self.preferred_times.remove(index);
        } else {
            self.preferred_times.push(time.to_string());
        }
    }

    fn render_general_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_6()
            .child(
                v_flex()
                    .gap_2()
                    .child(Label::new("Description").text_color(cx.theme().muted_foreground))
                    .child(Input::new(&self.description_input).w_full().h(px(80.0))),
            )
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        Label::new("What kind of action is this?")
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .children(ACTION_TYPES.iter().map(|action_type| {
                                let is_selected = self.action_type == *action_type;
                                let action_type_string = action_type.to_string();
                                let label: SharedString = capitalize(action_type).into();

                                let icon = match *action_type {
                                    "task" => IconName::Check,
                                    "activity" => IconName::Play,
                                    "event" => IconName::Calendar,
                                    "habit" => IconName::Redo,
                                    _ => IconName::Dash,
                                };

                                Button::new(SharedString::from(format!("type-{}", action_type)))
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
                                        this.action_type = action_type_string.clone();
                                        cx.notify();
                                    }))
                            })),
                    ),
            )
    }

    fn render_metadata_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_5()
            .child(self.render_duration_selector(cx))
            .child(self.render_scale_selector(
                "How draining is this?",
                "energy",
                -5..=5,
                self.energy_rate,
                energy_label,
                cx,
                |this, value| this.energy_rate = Some(value),
            ))
            .child(self.render_scale_selector(
                "How much focus does this need?",
                "attention",
                1..=5,
                self.attention_level,
                attention_label,
                cx,
                |this, value| this.attention_level = Some(value),
            ))
            .child(self.render_scale_selector(
                "How hard is it to start?",
                "transition",
                1..=5,
                self.transition_difficulty,
                transition_label,
                cx,
                |this, value| this.transition_difficulty = Some(value),
            ))
            .child(self.render_scale_selector(
                "How enjoyable once you begin?",
                "enjoyment",
                -5..=5,
                self.enjoyment_after_start,
                enjoyment_label,
                cx,
                |this, value| this.enjoyment_after_start = Some(value),
            ))
            .child(self.render_scale_selector(
                "How important is this?",
                "importance",
                1..=5,
                self.importance,
                importance_label,
                cx,
                |this, value| this.importance = Some(value),
            ))
            .child(self.render_urgency_toggle(cx))
    }

    fn render_preferences_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_6()
            .child(self.render_time_of_day_selector(cx))
    }

    fn render_duration_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_label = self
            .duration_bucket
            .map(|d| format!("~{}", duration_display(d)))
            .unwrap_or_else(|| "not set".to_string());

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Label::new("About how long will this take?")
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(SharedString::from(selected_label))
                            .text_color(cx.theme().foreground),
                    ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .flex_wrap()
                    .children(FIBONACCI_DURATIONS.iter().map(|&duration| {
                        let is_selected = self.duration_bucket == Some(duration);
                        let label: SharedString = duration_display(duration).into();
                        let id: SharedString = format!("dur-{}", duration).into();

                        Button::new(id)
                            .label(label)
                            .small()
                            .rounded_full()
                            .map(|button| {
                                if is_selected {
                                    button.primary()
                                } else {
                                    button.ghost()
                                }
                            })
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                if this.duration_bucket == Some(duration) {
                                    this.duration_bucket = None;
                                } else {
                                    this.duration_bucket = Some(duration);
                                }
                                cx.notify();
                            }))
                    })),
            )
    }

    fn render_scale_selector(
        &self,
        question: &str,
        id_prefix: &str,
        range: std::ops::RangeInclusive<i64>,
        current_value: Option<i64>,
        label_fn: fn(i64) -> &'static str,
        cx: &mut Context<Self>,
        setter: impl Fn(&mut Self, i64) + 'static + Clone,
    ) -> impl IntoElement {
        let current_label = current_value
            .map(|v| label_fn(v).to_string())
            .unwrap_or_else(|| "not set".to_string());

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Label::new(SharedString::from(question.to_string()))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(SharedString::from(current_label))
                            .text_color(cx.theme().foreground),
                    ),
            )
            .child(h_flex().gap_1().flex_wrap().children(range.map(|value| {
                let is_selected = current_value == Some(value);
                let display: SharedString = format!("{}", value).into();
                let id: SharedString = format!("{}-{}", id_prefix, value).into();
                let setter = setter.clone();

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
                    .tooltip(SharedString::from(label_fn(value).to_string()))
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        setter(this, value);
                        cx.notify();
                    }))
            })))
    }

    fn render_urgency_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .items_center()
            .justify_between()
            .child(
                v_flex()
                    .child(
                        Label::new("Does this get more urgent over time?")
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(if self.urgency_growth {
                            "Yes — importance increases the longer it waits"
                        } else {
                            "No — same priority whenever you do it"
                        })
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                Switch::new("urgency-growth")
                    .checked(self.urgency_growth)
                    .on_click(cx.listener(|this, checked, _window, cx| {
                        this.urgency_growth = *checked;
                        cx.notify();
                    })),
            )
    }

    fn render_time_of_day_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                    TIMES_OF_DAY
                        .iter()
                        .zip(TIME_OF_DAY_ICONS.iter())
                        .map(|(time, icon)| {
                            let icon = icon.clone();
                            let is_selected = self.preferred_times.iter().any(|t| t == *time);
                            let label: SharedString = capitalize(time).into();
                            let time_string = time.to_string();

                            Button::new(SharedString::from(format!("time-{}", time)))
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
                                    this.toggle_preferred_time(&time_string);
                                    cx.notify();
                                }))
                        }),
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
        let theme = cx.theme();
        let page_title: SharedString = self.current_page.title().into();

        let inner = v_flex().h_full().w(px(700.0)).pt_8().child(
            v_flex()
                .track_focus(&self.focus_handle)
                .min_h(px(480.0))
                .max_h(px(600.0))
                .bg(theme.group_box)
                .text_color(theme.group_box_foreground)
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .shadow_xl()
                .on_any_mouse_down(|_event, _window, cx| {
                    cx.stop_propagation();
                })
                .child(
                    v_flex()
                        .size_full()
                        .occlude()
                        .py_6()
                        .px_6()
                        .gap_4()
                        .child(
                            Input::new(&self.title_input)
                                .w_full()
                                .text_2xl()
                                .appearance(false),
                        )
                        .child(
                            h_flex().items_center().gap_2().child(
                                Label::new(page_title)
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                            ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .id("editor-page-scroll")
                                .overflow_y_scroll()
                                .map(|container| match self.current_page {
                                    ActionEditorPage::General => {
                                        container.child(self.render_general_page(cx))
                                    }
                                    ActionEditorPage::Metadata => {
                                        container.child(self.render_metadata_page(cx))
                                    }
                                    ActionEditorPage::Preferences => {
                                        container.child(self.render_preferences_page(cx))
                                    }
                                }),
                        )
                        .child(self.render_nav_footer(cx)),
                ),
        );

        popover(inner, cx)
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            upper + chars.as_str()
        }
    }
}
