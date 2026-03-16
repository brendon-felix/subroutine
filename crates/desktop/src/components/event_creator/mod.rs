use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, SharedString, Window, div,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    notification::NotificationType,
    v_flex,
};
use simple_core::Event;

use crate::{
    components::popover::{CloseOverlay, popover},
    stores::DatabaseStore,
    utils::{format_datetime_local, format_duration, parse_datetime_local, parse_duration},
};

pub struct EventCreator {
    pub focus_handle: FocusHandle,
    database_store: Entity<DatabaseStore>,

    title_input: Entity<InputState>,
    content_input: Entity<InputState>,
    time_input: Entity<InputState>,
    duration_input: Entity<InputState>,

    details_expanded: bool,
    current_title: String,

    _subscriptions: Vec<gpui::Subscription>,
}

impl EventCreator {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title_input = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("What's happening?");
            state.focus(window, cx);
            state
        });
        let content_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Any notes? (optional)"));
        let time_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. 3pm, 2026-03-01 14:00"));
        let duration_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. 30m, 1h (default 1h)"));

        let mut subscriptions = Vec::new();

        subscriptions.push(
            cx.subscribe(&title_input, |this, _, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    this.current_title = this.title_input.read(cx).value().trim().to_string();
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
            time_input,
            duration_input,
            details_expanded: false,
            current_title: String::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Pre-populates the creator from an existing event (for editing).
    pub fn load_event(&mut self, event: &Event, window: &mut Window, cx: &mut Context<Self>) {
        let title = event.title.clone();
        let time_str = format_datetime_local(event.time);
        let duration_str = event.duration.map(format_duration).unwrap_or_default();
        let content_str = event.content.clone().unwrap_or_default();

        self.title_input.update(cx, |input, cx| {
            input.set_value(title.clone(), window, cx);
        });
        self.time_input.update(cx, |input, cx| {
            input.set_value(time_str, window, cx);
        });
        self.duration_input.update(cx, |input, cx| {
            input.set_value(duration_str, window, cx);
        });
        self.content_input.update(cx, |input, cx| {
            input.set_value(content_str, window, cx);
        });
        self.current_title = title;
        cx.notify();
    }

    fn read_time(&self, cx: &App) -> Option<Result<chrono::DateTime<chrono::Utc>, String>> {
        let text = self.time_input.read(cx).value().to_string();
        let text = text.trim().to_string();
        if text.is_empty() {
            return None;
        }
        Some(parse_datetime_local(&text).map_err(|e| e.to_string()))
    }

    fn read_duration(&self, cx: &App) -> Option<chrono::Duration> {
        let text = self.duration_input.read(cx).value().to_string();
        let text = text.trim().to_string();
        if text.is_empty() {
            return None;
        }
        parse_duration(&text).ok()
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.current_title.clone();
        if title.is_empty() {
            return;
        }

        let time = match self.read_time(cx) {
            None => {
                window.push_notification(
                    (NotificationType::Warning, "Enter a time for the event"),
                    cx,
                );
                return;
            }
            Some(Err(ref error)) => {
                window.push_notification(
                    (
                        NotificationType::Error,
                        SharedString::from(format!("Invalid time: {error}")),
                    ),
                    cx,
                );
                return;
            }
            Some(Ok(time)) => time,
        };

        let content = {
            let value = self.content_input.read(cx).value().to_string();
            if value.trim().is_empty() {
                None
            } else {
                Some(value)
            }
        };

        // Default duration of 1 hour if not specified.
        let duration = self
            .read_duration(cx)
            .unwrap_or_else(|| chrono::Duration::hours(1));

        let mut event = Event::saved(title, time);
        event.content = content;
        event.duration = Some(duration);

        let warnings = self
            .database_store
            .update(cx, |store, cx| store.add_event_to_queue(event, cx));

        for warning in warnings {
            window.push_notification(
                (
                    NotificationType::Warning,
                    SharedString::from(format!(
                        "\"{}\" overlaps with \"{}\"",
                        warning.inserted_title, warning.conflicting_title
                    )),
                ),
                cx,
            );
        }

        window.push_notification((NotificationType::Success, "Event added to queue"), cx);
        cx.dispatch_action(&CloseOverlay);
    }
}

impl Focusable for EventCreator {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.title_input.focus_handle(cx)
    }
}

impl Render for EventCreator {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let can_submit = !self.current_title.is_empty();

        let time_parse_error = match self.read_time(cx) {
            Some(Err(ref error)) => Some(error.clone()),
            _ => None,
        };

        let inner = v_flex().h_full().w(px(480.0)).pt_8().child(
            v_flex()
                .track_focus(&self.focus_handle)
                .bg(theme.group_box)
                .text_color(theme.group_box_foreground)
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .shadow_xl()
                .on_any_mouse_down(|_event, _window, cx| {
                    cx.stop_propagation();
                })
                // Title row
                .child(
                    div()
                        .flex_none()
                        .px(px(16.0))
                        .py(px(12.0))
                        .border_b_1()
                        .border_color(theme.border)
                        .child(Input::new(&self.title_input).size_full()),
                )
                // Time + duration fields
                .child(
                    v_flex()
                        .w_full()
                        .px(px(16.0))
                        .py(px(10.0))
                        .gap_3()
                        .child(render_field_row(
                            "Time",
                            Input::new(&self.time_input).small(),
                            &theme,
                        ))
                        .when_some(time_parse_error, |this, error| {
                            this.child(
                                h_flex().pl(px(100.0)).child(
                                    Label::new(format!("Invalid time: {error}"))
                                        .text_xs()
                                        .text_color(theme.danger),
                                ),
                            )
                        })
                        .child(render_field_row(
                            "Duration",
                            Input::new(&self.duration_input).small(),
                            &theme,
                        ))
                        .child(
                            Button::new("toggle-details")
                                .label(if self.details_expanded {
                                    "Fewer options ↑"
                                } else {
                                    "More options ↓"
                                })
                                .ghost()
                                .xsmall()
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.details_expanded = !this.details_expanded;
                                    cx.notify();
                                })),
                        )
                        .when(self.details_expanded, |this| {
                            this.child(render_field_row(
                                "Notes",
                                Input::new(&self.content_input).small(),
                                &theme,
                            ))
                        }),
                )
                // Footer
                .child(
                    div()
                        .flex_none()
                        .px(px(16.0))
                        .py(px(10.0))
                        .border_t_1()
                        .border_color(theme.border)
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .items_center()
                                .child(
                                    Label::new("Press ↵ to add, Esc to close")
                                        .text_xs()
                                        .text_color(theme.muted_foreground),
                                )
                                .child(
                                    Button::new("submit")
                                        .label("Add event")
                                        .primary()
                                        .small()
                                        .cursor_pointer()
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

fn render_field_row(
    label: impl Into<SharedString>,
    input: Input,
    theme: &gpui_component::theme::Theme,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .child(
            Label::new(label)
                .text_xs()
                .text_color(theme.muted_foreground)
                .w(px(80.0)),
        )
        .child(input.w_full())
}
