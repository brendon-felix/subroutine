use chrono::{DateTime, Local, Utc};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, Render,
    SharedString, Subscription, Window, div, prelude::FluentBuilder, px,
};
use gpui::{InteractiveElement, ParentElement, StatefulInteractiveElement, Styled};
use gpui_component::WindowExt as _;
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    clipboard::Clipboard,
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::NotificationType,
    v_flex,
};
use simple_core::Event;
use uuid::Uuid;

use crate::components::popover::popover;
use crate::stores::AppDatabaseStore;
// use crate::stores::database_store::EventsLoaded;
use crate::utils::{
    format_datetime_local, format_duration, format_recurrence, parse_datetime_local, parse_duration,
};

pub struct StartEventEditor {
    pub event_id: Option<Uuid>,
}

pub struct EventEditor {
    pub focus_handle: FocusHandle,
    database_store: Entity<AppDatabaseStore>,

    /// The event being edited, if any.
    event: Option<Event>,
    /// True when `event` was loaded from the live queue rather than the
    /// saved-events template store. Determines which save path is used.
    is_queue_item: bool,

    title_input: Entity<InputState>,
    content_input: Entity<InputState>,
    duration_input: Entity<InputState>,
    scheduled_time_input: Entity<InputState>,

    /// Deferred values applied on the first render (requires &mut Window).
    pending_title: Option<String>,
    pending_content: Option<String>,
    pending_duration: Option<String>,
    pending_scheduled_time: Option<String>,

    _subscriptions: Vec<Subscription>,
}

impl Focusable for EventEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEditor {
    pub fn new(
        database_store: Entity<AppDatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        cx.focus_self(window);

        let title_input = cx.new(|cx| InputState::new(window, cx).placeholder("Event title"));
        let content_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Notes (optional)"));
        let duration_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. 30m, 1h, 90m"));
        let scheduled_time_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("e.g. 3pm, 2026-03-01 14:00 (required to add to queue)")
        });

        let subscriptions = Vec::new();

        Self {
            focus_handle,
            database_store,
            event: None,
            is_queue_item: false,
            title_input,
            content_input,
            duration_input,
            scheduled_time_input,
            pending_title: None,
            pending_content: None,
            pending_duration: None,
            pending_scheduled_time: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn load_event(&mut self, event_id: Uuid, cx: &mut Context<Self>) {
        let event = self.database_store.read(cx).get_event(event_id);

        if let Some(event) = event {
            self.pending_title = Some(event.title.clone());
            self.pending_content = event.content.clone();
            self.pending_duration = event.duration.map(format_duration);
            self.pending_scheduled_time = Some(format_datetime_local(event.time));
            self.is_queue_item = false;
            self.event = Some(event);
            cx.notify();
        }
    }

    /// Load an event that is live in the queue. The save button will call
    /// `update_queue_event` rather than adding a new entry.
    pub fn load_queue_event(&mut self, event_id: Uuid, cx: &mut Context<Self>) {
        let event = self.database_store.read(cx).get_event(event_id);

        if let Some(event) = event {
            self.pending_title = Some(event.title.clone());
            self.pending_content = event.content.clone();
            self.pending_duration = event.duration.map(format_duration);
            self.pending_scheduled_time = Some(format_datetime_local(event.time));
            self.is_queue_item = true;
            self.event = Some(event);
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
        if let Some(duration) = self.pending_duration.take() {
            self.duration_input.update(cx, |input, cx| {
                input.set_value(duration, window, cx);
            });
        }
        if let Some(time) = self.pending_scheduled_time.take() {
            self.scheduled_time_input.update(cx, |input, cx| {
                input.set_value(time, window, cx);
            });
        }
    }

    fn read_duration(&self, cx: &App) -> Option<chrono::Duration> {
        let text = self.duration_input.read(cx).value().to_string();
        if text.trim().is_empty() {
            return None;
        }
        parse_duration(text.trim()).ok()
    }

    fn read_scheduled_time(&self, cx: &App) -> Option<Result<DateTime<Utc>, String>> {
        let text = self.scheduled_time_input.read(cx).value().to_string();
        let text = text.trim().to_string();
        if text.is_empty() {
            return None;
        }
        Some(parse_datetime_local(&text).map_err(|e| e.to_string()))
    }

    fn build_event(&self, cx: &App) -> Option<Event> {
        let title = self.title_input.read(cx).value().to_string();
        if title.trim().is_empty() {
            return None;
        }
        let content = {
            let v = self.content_input.read(cx).value().to_string();
            if v.trim().is_empty() { None } else { Some(v) }
        };
        let duration = self.read_duration(cx);
        let time = match self.read_scheduled_time(cx) {
            Some(Ok(t)) => t,
            _ => Utc::now(),
        };

        let base = match &self.event {
            Some(existing) => existing.clone(),
            None => Event::new(title.clone(), time),
        };

        let mut updated = Event {
            title,
            content,
            time,
            duration,
            ..base
        };
        updated
            .duration
            .get_or_insert(chrono::Duration::minutes(60));
        Some(updated)
    }

    /// Primary save action. Routes to the right path depending on whether we
    /// are editing an existing queue item or creating/updating a template.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_queue_item {
            self.save_queue_update(window, cx);
        } else {
            self.save_and_queue(window, cx);
        }
    }

    /// Updates an event that already exists in the live queue.
    fn save_queue_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.read_scheduled_time(cx) {
            None => {
                window.push_notification(
                    (
                        NotificationType::Warning,
                        "Enter a scheduled time for the event",
                    ),
                    cx,
                );
                return;
            }
            Some(Err(ref e)) => {
                window.push_notification(
                    (
                        NotificationType::Error,
                        SharedString::from(format!("Invalid time: {e}")),
                    ),
                    cx,
                );
                return;
            }
            Some(Ok(_)) => {}
        }

        let Some(event) = self.build_event(cx) else {
            return;
        };

        let _warnings = self
            .database_store
            .update(cx, |store, cx| store.upsert_event(event, cx));

        // self.push_overlap_warnings(warnings, window, cx);
        window.push_notification((NotificationType::Success, "Event updated"), cx);
    }

    /// Adds the event as a new queue entry (used when not yet in the queue).
    fn save_and_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.read_scheduled_time(cx) {
            None => {
                window.push_notification(
                    (
                        NotificationType::Warning,
                        "Enter a scheduled time to add to queue",
                    ),
                    cx,
                );
                return;
            }
            Some(Err(ref e)) => {
                window.push_notification(
                    (
                        NotificationType::Error,
                        SharedString::from(format!("Invalid time: {e}")),
                    ),
                    cx,
                );
                return;
            }
            Some(Ok(_)) => {}
        }

        let Some(event) = self.build_event(cx) else {
            return;
        };

        let _warnings = self
            .database_store
            .update(cx, |store, cx| store.upsert_event(event, cx));

        // self.push_overlap_warnings(warnings, window, cx);
        window.push_notification((NotificationType::Success, "Event added to queue"), cx);
    }

    // fn push_overlap_warnings(
    //     &self,
    //     warnings: Vec<simple_core::OverlapWarning>,
    //     window: &mut Window,
    //     cx: &mut Context<Self>,
    // ) {
    //     for warning in warnings {
    //         window.push_notification(
    //             (
    //                 NotificationType::Warning,
    //                 SharedString::from(format!(
    //                     "\"{}\" overlaps with \"{}\"",
    //                     warning.inserted_title, warning.conflicting_title
    //                 )),
    //             ),
    //             cx,
    //         );
    //     }
    // }

    fn save_template(&mut self, cx: &mut Context<Self>) {
        let Some(event) = self.build_event(cx) else {
            return;
        };
        self.database_store.update(cx, |store, cx| {
            store.upsert_event(event, cx);
        });
    }

    fn delete_event(&mut self, cx: &mut Context<Self>) {
        if let Some(event) = &self.event {
            let id = event.id;
            self.database_store.update(cx, |store, cx| {
                store.delete_event(id, cx);
            });
        }
    }

    fn render_mode_badge(&self, cx: &App) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let (label, color) = if self.event.is_some() {
            ("Saved Event", theme.info)
        } else {
            ("New Event", theme.muted_foreground)
        };

        div()
            .px_2()
            .py(px(2.0))
            .rounded_full()
            .bg(color.alpha(0.15))
            .border_1()
            .border_color(color.alpha(0.4))
            .child(Label::new(label).text_xs().text_color(color))
    }

    fn render_scheduling_section(&self, cx: &App) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();

        let current_hint = self.event.as_ref().map(|e| {
            let local = e.time.with_timezone(&Local);
            format!("Currently: {}", local.format("%a %-d %b, %-I:%M%P"))
        });

        let parse_error = match self.read_scheduled_time(cx) {
            Some(Err(ref e)) => Some(e.clone()),
            _ => None,
        };

        v_flex()
            .w_full()
            .gap_2()
            .child(
                Label::new("Scheduling")
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(
                        h_flex()
                            .w_full()
                            .gap_3()
                            .items_center()
                            .child(
                                Label::new("Scheduled time")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .w(px(140.0)),
                            )
                            .child(Input::new(&self.scheduled_time_input).flex_1().small()),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .gap_3()
                            .items_center()
                            .child(
                                Label::new("Duration")
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .w(px(140.0)),
                            )
                            .child(Input::new(&self.duration_input).flex_1().small()),
                    )
                    .when_some(current_hint, |this, hint| {
                        this.child(
                            h_flex()
                                .pl(px(143.0))
                                .child(Label::new(hint).text_xs().text_color(theme.success)),
                        )
                    })
                    .when_some(parse_error, |this, error| {
                        this.child(
                            h_flex().pl(px(143.0)).child(
                                Label::new(format!("Invalid time: {}", error))
                                    .text_xs()
                                    .text_color(theme.danger_foreground),
                            ),
                        )
                    }),
            )
    }

    fn render_debug_box(&self, cx: &App) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();

        let rows: Vec<(&'static str, String)> = match &self.event {
            Some(e) => vec![
                ("id", e.id.to_string()),
                ("lineage_id", e.lineage_id.to_string()),
                ("saved", e.saved.to_string()),
                (
                    "recurrence",
                    e.recurrence
                        .map(|d| format_recurrence(d))
                        .unwrap_or_else(|| "—".to_string()),
                ),
            ],
            None => vec![],
        };

        v_flex()
            .w_full()
            .gap_1()
            .p_3()
            .rounded_md()
            .bg(theme.secondary)
            .border_1()
            .border_color(theme.border)
            .child(
                Label::new("Debug info")
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
            .children(rows.into_iter().enumerate().map(|(ix, (key, value))| {
                let copy_value = SharedString::from(value.clone());
                h_flex()
                    .gap_2()
                    .w_full()
                    .items_center()
                    .child(
                        Label::new(key)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .w(px(140.0)),
                    )
                    .child(
                        Label::new(SharedString::from(value))
                            .text_xs()
                            .text_color(theme.foreground)
                            .truncate()
                            .flex_1(),
                    )
                    .child(
                        Clipboard::new(SharedString::from(format!("debug-copy-{}", ix)))
                            .value(copy_value),
                    )
            }))
    }

    fn render_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let save_label = if self.is_queue_item {
            "Save changes"
        } else {
            "Add to queue"
        };
        h_flex()
            .gap_2()
            .child(
                Button::new("save-queue")
                    .label(save_label)
                    .primary()
                    .on_click(cx.listener(|this, _event, window, cx| {
                        this.save(window, cx);
                    })),
            )
            .child(
                Button::new("save-template")
                    .label("Save template")
                    .outline()
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.save_template(cx);
                    })),
            )
            .when(self.event.is_some(), |row| {
                row.child(
                    Button::new("delete")
                        .label("Delete")
                        .danger()
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.delete_event(cx);
                        })),
                )
            })
            .into_any_element()
    }
}

impl Render for EventEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_pending_values(window, cx);

        let theme = cx.theme().clone();

        let badge = self.render_mode_badge(cx);
        let scheduling_section = self.render_scheduling_section(cx);
        let debug_box = self.render_debug_box(cx);
        let actions = self.render_actions(cx);

        let inner = v_flex()
            .w(px(560.0))
            .h(px(560.0))
            .flex_initial()
            .bg(theme.group_box)
            .text_color(theme.group_box_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded_xl()
            .shadow_xl()
            .track_focus(&self.focus_handle)
            .on_any_mouse_down(|_event, _window, cx| {
                cx.stop_propagation();
            })
            .p_4()
            .gap_4()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        Label::new("Event Editor")
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .child(badge),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        Input::new(&self.title_input)
                            .w_full()
                            .large()
                            .focus_bordered(true)
                            .appearance(false),
                    )
                    .child(
                        Input::new(&self.content_input)
                            .w_full()
                            .focus_bordered(true)
                            .appearance(false),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .id("event-editor-scroll")
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .gap_4()
                            .w_full()
                            .child(scheduling_section)
                            .child(debug_box)
                            .child(actions),
                    ),
            );

        popover(inner, cx)
    }
}
