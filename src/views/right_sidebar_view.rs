use gpui::prelude::*;
use gpui::{Context, EventEmitter, IntoElement, Render, Subscription, Window, div, rgb};
// use gpui_component::input::InputState;
use gpui_component::label::Label;
use gpui_component::v_flex;
// use ticks::tasks::TaskPriority;

// use crate::stores::ui_store::{TaskSelected, UiStateChanged, UiStateStore};

pub struct RightSidebarView {
    // title_input: Entity<InputState>,
    // desc_input: Entity<InputState>,
    // ui_store: Entity<UiStateStore>,
    collapsed: bool,
    // last_selected_task_id: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl RightSidebarView {
    pub fn new(
        // ui_store: Entity<UiStateStore>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Self {
        // let title_input = cx.new(|cx| InputState::new(window, cx));
        // let desc_input = cx.new(|cx| InputState::new(window, cx).multi_line(true));

        // let mut subscriptions = vec![
        //     cx.subscribe(
        //         &ui_store,
        //         |_this, _ui_store, _event: &UiStateChanged, cx| {
        //             cx.notify();
        //         },
        //     ),
        //     cx.subscribe(&ui_store, |_this, _ui_store, _event: &TaskSelected, cx| {
        //         cx.notify();
        //     }),
        // ];

        // let mut subscriptions = vec![];

        // Subscribe to title input events
        // subscriptions.push(cx.subscribe_in(
        //     &title_input,
        //     window,
        //     |this, _input, event: &InputEvent, _window, cx| match event {
        //         InputEvent::Change => {
        //             let new_title = this.title_input.read(cx).value();
        //             this.ui_store.update(cx, |store, cx| {
        //                 store.update_selected_task_content(Some(new_title.to_string()), None);
        //                 cx.emit(UiStateChanged);
        //                 cx.notify();
        //             });
        //         }
        //         _ => {}
        //     },
        // ));

        // Subscribe to description input events
        // subscriptions.push(cx.subscribe_in(
        //     &desc_input,
        //     window,
        //     |this, _input, event: &InputEvent, _window, cx| match event {
        //         InputEvent::Change => {
        //             let new_content = this.desc_input.read(cx).value();
        //             this.ui_store.update(cx, |store, cx| {
        //                 store.update_selected_task_content(None, Some(new_content.to_string()));
        //                 cx.emit(UiStateChanged);
        //                 cx.notify();
        //             });
        //         }
        //         _ => {}
        //     },
        // ));

        Self {
            // title_input,
            // desc_input,
            // ui_store,
            collapsed: false,
            // last_selected_task_id: None,
            _subscriptions: vec![],
        }
    }

    pub fn toggle_collapsed(&mut self, cx: &mut Context<Self>) {
        self.collapsed = !self.collapsed;
        cx.notify();
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    // pub fn set_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
    //     if self.collapsed != collapsed {
    //         self.collapsed = collapsed;
    //         cx.notify();
    //     }
    // }

    // fn update_selected_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    //     let selected_task = self.ui_store.read(cx).get_selected_task().clone();
    //     let current_task_id = selected_task
    //         .as_ref()
    //         .and_then(|t| t.task_id.as_ref())
    //         .map(|id| id.0.clone());

    //     // Only update input fields if the selected task has changed
    //     if self.last_selected_task_id != current_task_id {
    //         self.last_selected_task_id = current_task_id;

    //         if let Some(task) = selected_task {
    //             // Update input fields with task data - keep title sanitized, allow newlines in content
    //             let title = task
    //                 .title
    //                 .as_ref()
    //                 .unwrap_or(&"Untitled Task".to_string())
    //                 .replace('\n', " ")
    //                 .replace('\r', " ");
    //             let content = task.content.as_ref().unwrap_or(&"".to_string()).clone();

    //             self.title_input.update(cx, |input, cx| {
    //                 input.set_value(title, window, cx);
    //             });

    //             self.desc_input.update(cx, |input, cx| {
    //                 input.set_value(content, window, cx);
    //             });
    //         } else {
    //             // Clear input fields when no task is selected
    //             self.title_input.update(cx, |input, cx| {
    //                 input.set_value("".to_string(), window, cx);
    //             });

    //             self.desc_input.update(cx, |input, cx| {
    //                 input.set_value("".to_string(), window, cx);
    //             });
    //         }
    //     }
    // }
}

impl EventEmitter<()> for RightSidebarView {}

impl Render for RightSidebarView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Update input fields with selected task data during render
        // self.update_selected_task(window, cx);

        // let selected_task = self.ui_store.read(cx).get_selected_task().clone();

        div()
            .size_full()
            // .bg(rgb(0x191919))
            .border_l_1()
            .border_color(rgb(0x303030))
            .child(
                v_flex()
                    .size_full()
                    .child(
                        // Header
                        div().p_4().border_b_1().border_color(rgb(0x303030)).child(
                            v_flex()
                                .gap_1()
                                .child(
                                    Label::new("Task Details")
                                        .text_lg()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(0xE0E0E0)),
                                )
                                .child(
                                    Label::new("Selected Task")
                                        .text_xs()
                                        .text_color(rgb(0x888888)),
                                ),
                        ),
                    )
                    .child(
                        // Content
                        div().flex_1().overflow_y_hidden().p_4().child(
                            v_flex().gap_4().size_full(), // .when_some(selected_task.as_ref(), |this, task| {
                                                          //     this.child(
                                                          //         v_flex()
                                                          //             .gap_3()
                                                          //             .child(
                                                          //                 v_flex()
                                                          //                     .gap_1()
                                                          //                     .child(
                                                          //                         Label::new("Priority")
                                                          //                             .text_sm()
                                                          //                             .text_color(rgb(0xA0A0A0)),
                                                          //                     )
                                                          //                     .child({
                                                          //                         let (priority_text, priority_color) =
                                                          //                             match &task.priority {
                                                          //                                 Some(TaskPriority::High) => {
                                                          //                                     ("High", rgb(0xff4444))
                                                          //                                 }
                                                          //                                 Some(TaskPriority::Medium) => {
                                                          //                                     ("Medium", rgb(0xffaa00))
                                                          //                                 }
                                                          //                                 Some(TaskPriority::Low) => {
                                                          //                                     ("Low", rgb(0x4444ff))
                                                          //                                 }
                                                          //                                 Some(TaskPriority::None) => {
                                                          //                                     ("None", rgb(0x888888))
                                                          //                                 }
                                                          //                                 None => ("None", rgb(0x888888)),
                                                          //                             };
                                                          //                         Label::new(priority_text)
                                                          //                             .text_color(priority_color)
                                                          //                     }),
                                                          //             )
                                                          //             .when_some(task.due_date.as_ref(), |this, due_date| {
                                                          //                 this.child(
                                                          //                     v_flex()
                                                          //                         .gap_1()
                                                          //                         .child(
                                                          //                             Label::new("Due Date")
                                                          //                                 .text_sm()
                                                          //                                 .text_color(rgb(0xA0A0A0)),
                                                          //                         )
                                                          //                         .child(
                                                          //                             Label::new(
                                                          //                                 due_date
                                                          //                                     .format("%B %d, %Y at %I:%M %p")
                                                          //                                     .to_string(),
                                                          //                             )
                                                          //                             .text_color(rgb(0xE0E0E0)),
                                                          //                         ),
                                                          //                 )
                                                          //             })
                                                          //             .child(
                                                          //                 v_form()
                                                          //                     .child(field().label("Title").child(
                                                          //                         Input::new(&self.title_input), // .bg(rgb(0x191919)),
                                                          //                     ))
                                                          //                     .child(field().label("Description").child(
                                                          //                         Input::new(&self.desc_input).h(px(200.0)), // .bg(rgb(0x191919)),
                                                          //                     )),
                                                          //             ),
                                                          //     )
                                                          // })
                                                          // .when(selected_task.is_none(), |this| {
                                                          //     this.child(
                                                          //         v_flex()
                                                          //             .items_center()
                                                          //             .justify_center()
                                                          //             .flex_1()
                                                          //             .gap_2()
                                                          //             .child(
                                                          //                 Icon::new(IconName::Info).text_color(rgb(0x888888)),
                                                          //             )
                                                          //             .child(
                                                          //                 Label::new("No task selected")
                                                          //                     .text_color(rgb(0x888888)),
                                                          //             )
                                                          //             .child(
                                                          //                 Label::new("Select a task to view details")
                                                          //                     .text_sm()
                                                          //                     .text_color(rgb(0x666666)),
                                                          //             ),
                                                          //     )
                                                          // }),
                        ),
                    ),
            )
    }
}
