use gpui::{App, Entity, FocusHandle, Focusable, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme,
    input::{Input, InputEvent, InputState},
    label::Label,
    v_flex,
};

use crate::{
    components::popover::{CloseOverlay, popover},
    stores::DatabaseStore,
};

pub struct TaskCreator {
    pub focus_handle: FocusHandle,
    pub input_state: Entity<InputState>,
    pub database_store: Entity<DatabaseStore>,
    pending_title: Option<String>,
}

impl TaskCreator {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input_state = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("Create a new task...");
            state.focus(window, cx);
            state
        });

        cx.subscribe(&input_state, |this, _input, event, cx| {
            match event {
                InputEvent::Change => {
                    let value = this.input_state.read(cx).value().to_string();
                    if value.trim().is_empty() {
                        this.pending_title = None;
                    } else {
                        this.pending_title = Some(value);
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    if let Some(title) = this.pending_title.take() {
                        let title = title.trim().to_string();
                        if !title.is_empty() {
                            this.database_store.update(cx, |store, cx| {
                                store.create_action(title, cx);
                            });
                        }
                        cx.dispatch_action(&CloseOverlay);
                        this.pending_title = None;
                    }
                }
                _ => {}
            }
            cx.notify();
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            input_state,
            database_store,
            pending_title: None,
        }
    }
}

impl Focusable for TaskCreator {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input_state.focus_handle(cx)
    }
}

impl Render for TaskCreator {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        let inner = v_flex().h_full().w(px(600.0)).pt_8().child(
            v_flex()
                .track_focus(&self.focus_handle)
                .max_h(px(500.0))
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
                    div()
                        .flex_none()
                        .items_center()
                        .px(px(16.0))
                        .py(px(12.0))
                        .border_b_1()
                        .border_color(theme.border)
                        .child(Input::new(&self.input_state).size_full()),
                )
                .when_some(self.pending_title.as_deref(), |this, title| {
                    this.child(
                        div()
                            .flex_1()
                            .p_4()
                            .child(Label::new(format!("New action: {}", title))),
                    )
                }),
        );

        popover(inner, cx)
    }
}
