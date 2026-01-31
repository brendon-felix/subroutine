use database::Action;
use gpui::{App, Entity, FocusHandle, Focusable, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme,
    input::{Input, InputEvent, InputState},
    label::Label,
    v_flex,
};

use crate::{components::overlay::CloseOverlay, stores::DatabaseStore};

// actions!(
//     task_creator,
//     [Select]
// );

// pub fn init(cx: &mut App) {
//     let context: Option<&str> = Some("CommandPalette");
//     cx.bind_keys([
//         KeyBinding::new("escape", CloseCommandPalette, context),
//         KeyBinding::new("enter", SelectCommand, context),
//         KeyBinding::new("up", NavigateUp, context),
//         KeyBinding::new("down", NavigateDown, context),
//     ]);
// }

#[allow(unused)]
pub struct TaskCreator {
    pub focus_handle: FocusHandle,
    pub input_state: Entity<InputState>,
    pub database_store: Entity<DatabaseStore>,
    // pub task_data: Option<TaskData>,
    pub action_data: Option<Action>,
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
                    if let Some(task_entry) = Some(this.input_state.read(cx).value().to_string()) {
                        if task_entry.trim().is_empty() {
                            // this.task_data = None;
                            this.action_data = None;
                            cx.notify();
                        } else {
                            this.parse_entry(task_entry, cx);
                        }
                    }
                }
                InputEvent::PressEnter { .. } => {
                    if let Some(action) = this.action_data.take() {
                        // Use the combined method that inserts the action, creates an instance,
                        // and enqueues it in a single spawned task to avoid races.
                        this.database_store.update(cx, |db_store, cx| {
                            let id = action.id.clone();
                            db_store.insert_action(action, cx);
                            db_store.create_instance_for_action(id, cx);
                        });
                        // Clear the input field after adding the task.
                        // this.input_state
                        //     .update(cx, |input, cx| {
                        //         input.set_value("".to_string(), cx);
                        //     })
                        //     .unwrap();
                        cx.dispatch_action(&CloseOverlay);
                        // Clear the parsed data.
                        // this.task_data = None;
                        this.action_data = None;
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
            // task_data: None,
            action_data: None,
        }
    }

    pub fn parse_entry(&mut self, entry: String, _cx: &mut Context<Self>) {
        self.action_data = Some(Action::new_task(entry));
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

        // Build the inner dialog card and let the shared overlay shell handle backdrop,
        // occlusion, centering and shared key context/close behaviour.
        let inner = v_flex().h_full().w(px(600.0)).pt_8().child(
            v_flex() // dialog container (inner)
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
                // .when_some(self.task_data.as_ref(), |this, task_data| {
                //     this.child(div().flex_1().p_4().child(Label::new(format!(
                //         "Task Title: {}",
                //         task_data.title.as_deref().unwrap_or("")
                //     ))))
                // }),
                .when_some(self.action_data.as_ref(), |this, task| {
                    this.child(
                        div()
                            .flex_1()
                            .p_4()
                            .child(Label::new(format!("New Action: {}", &task.title))),
                    )
                }),
        );

        // Reuse the centralized overlay shell for consistent overlay chrome and behaviour.
        crate::components::overlay::shell(theme, inner)
    }
}
