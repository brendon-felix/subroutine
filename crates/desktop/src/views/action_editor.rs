use chrono::{DateTime, Utc};
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    clipboard::Clipboard,
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::NotificationType,
    v_flex,
};
use simple_core::Action;
use uuid::Uuid;

use crate::{
    components::popover::popover,
    stores::DatabaseStore,
    utils::{format_datetime_local, format_duration, parse_datetime_local, parse_duration},
};

pub struct StartActionEditor {
    pub action_id: Option<Uuid>,
}

pub struct ActionEditor {
    pub focus_handle: FocusHandle,
    database_store: Entity<DatabaseStore>,

    /// The action being edited, if any.
    action: Option<Action>,

    title_input: Entity<InputState>,
    content_input: Entity<InputState>,
    duration_input: Entity<InputState>,
    target_time_input: Entity<InputState>,
    recurrence_input: Entity<InputState>,

    /// Deferred values applied on the first render (requires &mut Window).
    pending_title: Option<String>,
    pending_content: Option<String>,
    pending_duration: Option<String>,
    pending_target_time: Option<String>,
    pending_recurrence: Option<String>,

    _subscriptions: Vec<Subscription>,
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
        let duration_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. 30m, 1h, 2h"));
        let target_time_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("e.g. 3pm, 2026-03-01 14:00 (leave blank for backlog)")
        });
        let recurrence_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("e.g. 1d, 7d, 4h (leave blank for none)")
        });

        Self {
            focus_handle,
            database_store,
            action: None,
            title_input,
            content_input,
            duration_input,
            target_time_input,
            recurrence_input,
            pending_title: None,
            pending_content: None,
            pending_duration: None,
            pending_target_time: None,
            pending_recurrence: None,
            _subscriptions: Vec::new(),
        }
    }

    /// Load a saved action into the editor by ID.
    pub fn load_action(&mut self, action_id: Uuid, cx: &mut Context<Self>) {
        let store = self.database_store.read(cx);
        // Look in the saved-actions table first, then fall back to the live
        // queue and backlog for actions that were never persisted separately.
        let action = store
            .get_action(action_id)
            .or_else(|| store.get_queue_action(action_id))
            .or_else(|| store.get_backlog_action(action_id))
            .cloned();

        if let Some(action) = action {
            self.pending_title = Some(action.title.clone());
            self.pending_content = action.content.clone();
            self.pending_duration = action.duration.map(format_duration);
            self.pending_target_time = action.target.map(format_datetime_local);
            self.pending_recurrence = action.recurrence.map(format_duration);
            self.action = Some(action);
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
        if let Some(target_time) = self.pending_target_time.take() {
            self.target_time_input.update(cx, |input, cx| {
                input.set_value(target_time, window, cx);
            });
        }
        if let Some(recurrence) = self.pending_recurrence.take() {
            self.recurrence_input.update(cx, |input, cx| {
                input.set_value(recurrence, window, cx);
            });
        }
    }

    fn read_target_time(&self, cx: &App) -> Option<Result<DateTime<Utc>, String>> {
        let text = self.target_time_input.read(cx).value().to_string();
        let text = text.trim().to_string();
        if text.is_empty() {
            return None;
        }
        Some(parse_datetime_local(&text).map_err(|e| e.to_string()))
    }

    fn build_action(&self, cx: &App) -> Action {
        let title = self.title_input.read(cx).value().to_string();
        let content = {
            let value = self.content_input.read(cx).value().to_string();
            if value.trim().is_empty() {
                None
            } else {
                Some(value)
            }
        };
        let duration = {
            let text = self.duration_input.read(cx).value().to_string();
            if text.trim().is_empty() {
                None
            } else {
                parse_duration(text.trim()).ok()
            }
        };
        let target = self.read_target_time(cx).and_then(|r| r.ok());
        // A manually entered target time is always treated as static so the
        // scheduler does not move it automatically.
        let target_static = target.is_some();
        let recurrence = {
            let text = self.recurrence_input.read(cx).value().to_string();
            if text.trim().is_empty() {
                None
            } else {
                parse_duration(text.trim()).ok()
            }
        };

        let base = match &self.action {
            Some(existing) => existing.clone(),
            None => Action::new_saved(title.clone()),
        };

        Action {
            title,
            content,
            duration,
            target,
            target_static,
            recurrence,
            ..base
        }
    }

    fn save_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let action = self.build_action(cx);
        if action.title.trim().is_empty() {
            return;
        }

        let action_id = action.id;
        let is_in_queue = self
            .database_store
            .read(cx)
            .get_queue_action(action_id)
            .is_some();

        let warnings = if is_in_queue {
            self.database_store
                .update(cx, |store, cx| store.update_queue_action(action, cx))
        } else {
            // Not in the queue — just persist the saved-action record.
            // The pipeline will pick it up on the next refresh if needed.
            self.database_store.update(cx, |store, cx| {
                store.upsert_action(action, cx);
            });
            Vec::new()
        };

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
    }

    fn delete_action(&mut self, cx: &mut Context<Self>) {
        if let Some(action) = &self.action {
            let id = action.id;
            self.database_store.update(cx, |store, cx| {
                store.delete_action(id, cx);
            });
        }
    }

    fn render_debug_box(&self, cx: &App) -> impl IntoElement + use<> {
        let theme = cx.theme();

        let rows: Vec<(&'static str, String)> = match &self.action {
            Some(action) => vec![
                ("ID", action.id.to_string()),
                ("Lineage ID", action.lineage_id.to_string()),
                (
                    "Origin Routine",
                    action
                        .origin_routine_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "—".to_string()),
                ),
                ("Ephemeral", action.ephemeral.to_string()),
                ("Target static", action.target_static.to_string()),
                (
                    "Target",
                    action
                        .target
                        .map(format_datetime_local)
                        .unwrap_or_else(|| "—".to_string()),
                ),
                (
                    "Duration",
                    action
                        .duration
                        .map(format_duration)
                        .unwrap_or_else(|| "—".to_string()),
                ),
                (
                    "Recurrence",
                    action
                        .recurrence
                        .map(format_duration)
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
                            .w(px(160.0)),
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

    fn render_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .child(
                Button::new("save")
                    .label("Save")
                    .primary()
                    .on_click(cx.listener(|this, _event, window, cx| {
                        this.save_action(window, cx);
                    })),
            )
            .when(self.action.is_some(), |this| {
                this.child(
                    Button::new("delete")
                        .label("Delete")
                        .danger()
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.delete_action(cx);
                        })),
                )
            })
    }
}

impl Render for ActionEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_pending_values(window, cx);

        let theme = cx.theme().clone();
        let is_new = self.action.is_none();

        let parse_error_target = match self.read_target_time(cx) {
            Some(Err(ref e)) => Some(e.clone()),
            _ => None,
        };

        let inner = v_flex()
            .w(px(480.0))
            .flex_initial()
            .bg(theme.group_box)
            .text_color(theme.group_box_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
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
                        Label::new("Action Editor")
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div()
                            .px_2()
                            .py(px(2.0))
                            .rounded_full()
                            .bg(theme.info.alpha(0.15))
                            .border_1()
                            .border_color(theme.info.alpha(0.4))
                            .child(
                                Label::new(if is_new { "New Action" } else { "Saved Action" })
                                    .text_xs()
                                    .text_color(theme.info),
                            ),
                    ),
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
                    .id("action-editor-scroll")
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .gap_4()
                            .w_full()
                            .child(
                                v_flex()
                                    .w_full()
                                    .gap_2()
                                    .child(
                                        Label::new("Scheduling")
                                            .text_xs()
                                            .text_color(theme.muted_foreground),
                                    )
                                    .child(render_field_row(
                                        "Duration",
                                        Input::new(&self.duration_input).small(),
                                        &theme,
                                    ))
                                    .child(
                                        v_flex()
                                            .w_full()
                                            .gap_1()
                                            .child(render_field_row(
                                                "Target time",
                                                Input::new(&self.target_time_input).small(),
                                                &theme,
                                            ))
                                            .when_some(parse_error_target, |this, error| {
                                                this.child(
                                                    h_flex().pl(px(163.0)).child(
                                                        Label::new(format!(
                                                            "Invalid time: {}",
                                                            error
                                                        ))
                                                        .text_xs()
                                                        .text_color(theme.danger),
                                                    ),
                                                )
                                            }),
                                    )
                                    .child(render_field_row(
                                        "Recurrence",
                                        Input::new(&self.recurrence_input).small(),
                                        &theme,
                                    )),
                            )
                            .child(self.render_debug_box(cx))
                            .child(self.render_actions(cx)),
                    ),
            );

        popover(inner, cx)
    }
}

fn render_field_row(
    label: impl Into<gpui::SharedString>,
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
                .w(px(120.0)),
        )
        .child(input.w_full())
}
