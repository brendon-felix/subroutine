use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, SharedString, Window, div,
    prelude::*, px, rems,
};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    notification::NotificationType,
    switch::Switch,
    v_flex,
};
use simple_core::Action;

use crate::{
    components::popover::{CloseOverlay, popover},
    stores::DatabaseStore,
};

/// Whether the action being created will persist as a reusable template.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ActionKind {
    /// Creates only a concrete pipeline `Action` — no `SavedAction` template.
    Ephemeral,
    /// Creates a `SavedAction` template and immediately instantiates one `Action` from it.
    Saved,
}

pub struct ActionCreator {
    pub focus_handle: FocusHandle,
    database_store: Entity<DatabaseStore>,

    title_input: Entity<InputState>,
    content_input: Entity<InputState>,

    kind: ActionKind,
    details_expanded: bool,

    current_title: String,
    current_content: String,

    batch_mode: bool,

    _subscriptions: Vec<gpui::Subscription>,
}

impl ActionCreator {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title_input = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("Action name");
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

        // Submit on Enter in the title field — needs window for notifications,
        // so we use subscribe_in.
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
            kind: ActionKind::Saved,
            details_expanded: false,
            current_title: String::new(),
            current_content: String::new(),
            batch_mode: false,
            _subscriptions: subscriptions,
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.current_title.clone();
        if title.is_empty() {
            return;
        }

        let content = if self.current_content.is_empty() {
            None
        } else {
            Some(self.current_content.clone())
        };

        let mut action = match self.kind {
            ActionKind::Ephemeral => Action::new(title),
            ActionKind::Saved => Action::new_saved(title),
        };
        action.content = content;

        let warnings = self
            .database_store
            .update(cx, |store, cx| store.add_action_to_queue(action, cx));

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

        // cx.dispatch_action(&CloseOverlay);
        if !self.batch_mode {
            window.dispatch_action(Box::new(CloseOverlay), cx);
        } else {
            self.current_title.clear();
            self.title_input
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.current_content.clear();
            self.content_input
                .update(cx, |input, cx| input.set_value("", window, cx));
            cx.notify();
        }
    }

    fn render_kind_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let kind = self.kind;

        h_flex()
            .gap_1()
            .child(
                Button::new("kind-saved")
                    .label("Save as template")
                    .xsmall()
                    .map(|b| {
                        if kind == ActionKind::Saved {
                            b.primary()
                        } else {
                            b.ghost()
                        }
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.kind = ActionKind::Saved;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("kind-ephemeral")
                    .label("One-off")
                    .xsmall()
                    .map(|b| {
                        if kind == ActionKind::Ephemeral {
                            b.primary()
                        } else {
                            b.ghost()
                        }
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.kind = ActionKind::Ephemeral;
                        cx.notify();
                    })),
            )
            .child({
                let description = match kind {
                    ActionKind::Saved => "Saved as a reusable template and added to the pipeline.",
                    ActionKind::Ephemeral => "Added directly to the pipeline — no saved template.",
                };
                Label::new(description)
                    .text_xs()
                    .text_color(theme.muted_foreground)
            })
    }
}

impl Focusable for ActionCreator {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.title_input.focus_handle(cx)
    }
}

impl Render for ActionCreator {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let can_submit = !self.current_title.is_empty();

        let inner = v_flex().h_full().w_128().pt_8().child(
            v_flex()
                .pt_2()
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
                // .child(
                //     h_flex()
                //         .w_full()
                //         .items_center()
                //         .justify_between()
                //         .px_4()
                //         .py_3()
                //         .border_b_1()
                //         .border_color(theme.border)
                //         .child(
                //             Label::new("Create a new action")
                //                 .font_semibold()
                //                 .text_color(theme.foreground),
                //         )
                //         // Close button
                //         .child(
                //             Button::new("close")
                //                 // .label("×")
                //                 .icon(IconName::Close)
                //                 .ghost()
                //                 .cursor_pointer()
                //                 .on_click(|_, window, cx| {
                //                     window.dispatch_action(Box::new(CloseOverlay), cx);
                //                 }),
                //         ),
                // )
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
                    Input::new(&self.content_input)
                        .w_full()
                        .py_0()
                        .px_4()
                        .text_size(rems(0.75))
                        .line_height(rems(0.75))
                        .focus_bordered(true)
                        .appearance(false),
                )
                .child(
                    h_flex()
                        .w_full()
                        .gap_3()
                        .items_center()
                        .px(px(16.0))
                        .py(px(10.0))
                        .border_b_1()
                        .border_color(theme.border)
                        .child(
                            Label::new("Options")
                                .text_xs()
                                .font_semibold()
                                .text_color(theme.muted_foreground),
                        ),
                )
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
                                .child(
                                    Button::new("cancel")
                                        .small()
                                        .label("Cancel")
                                        .cursor_pointer()
                                        .on_click(|_, window, cx| {
                                            window.dispatch_action(Box::new(CloseOverlay), cx);
                                        }),
                                )
                                .child(
                                    Button::new("submit")
                                        .small()
                                        .primary()
                                        .label("Add action")
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
