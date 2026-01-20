use std::thread::sleep;
use std::time::Duration;

use gpui::{
    AnyElement, App, Context, Div, ElementId, Entity, EventEmitter, Interactivity, IntoElement,
    Render, SharedString, Stateful, StyleRefinement, Window, div, font, px, size,
};
use gpui::{FocusHandle, prelude::*};
// use gpui_component::input::InputState;
use gpui_component::label::Label;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{ActiveTheme, Selectable, Sizable, StyledExt, h_flex, v_flex};
use smallvec::SmallVec;

use crate::components::checkbox::Checkbox;
use crate::components::pipeline::{NavigateDown, NavigateUp};
use crate::stores::TaskStore;
use crate::stores::task_store::{ApiError, TaskCreated, TaskDeleted, TasksUpdated};
use crate::tasks::{TaskData, task_data_compare};

struct Pipeline {
    task_store: Entity<TaskStore>,
    task_data: Vec<TaskData>,
}

impl Pipeline {
    pub fn new(task_store: Entity<TaskStore>, cx: &mut Context<Self>) -> Self {
        let mut pipeline = Self {
            task_store,
            task_data: vec![],
        };
        pipeline.update_tasks(cx);
        pipeline
    }

    pub fn update_tasks(&mut self, cx: &mut Context<Self>) {
        self.task_data = self
            .task_store
            .read(cx)
            // .get_filtered_tasks(&current_view)
            .get_all_tasks()
            .into_iter()
            .cloned()
            .collect();
        self.task_data.sort_by(task_data_compare);
    }
}

impl Render for Pipeline {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_col_reverse()
            .size_full()
            .overflow_hidden()
            // .overflow_y_scrollbar()
            .child(
                v_flex().p_3().gap_3().flex_col_reverse().children(
                    self.task_data
                        .iter()
                        .take(5)
                        .enumerate()
                        .map(|(i, task)| {
                            let title = task.title.clone().unwrap_or("Untitled".into());
                            // Label::new(title)
                            //     .font(font("Arial"))
                            //     .p_2()
                            //     .bg(cx.theme().background)
                            //     .rounded_md()
                            //     .border_1()
                            //     .border_color(cx.theme().border)
                            let opacity = 1.0 - (i as f32 * 0.2);
                            div()
                                .id(ElementId::NamedInteger("pipeline-item".into(), i as u64))
                                .h_32()
                                .p_2()
                                .bg(cx.theme().background)
                                .opacity(opacity)
                                .rounded_md()
                                .border_1()
                                .border_color(cx.theme().border)
                                .hover(|this| this.bg(cx.theme().list_hover).cursor_pointer())
                                .child(
                                    Checkbox::new(ElementId::NamedInteger(
                                        "pipeline-checkbox".into(),
                                        i as u64,
                                    ))
                                    .checked(false)
                                    .large()
                                    .on_click(cx.listener(move |_view, _checked, _window, cx| {})),
                                )
                                .child(
                                    Label::new(title)
                                        .font(font("Arial"))
                                        .text_color(cx.theme().foreground),
                                )
                        })
                        .collect::<Vec<_>>(),
                ),
            )
    }
}

pub struct RightSidebarView {
    collapsed: bool,
    pipeline: Entity<Pipeline>,
    task_store: Entity<TaskStore>,
    // _subscriptions: Vec<Subscription>,
}

impl RightSidebarView {
    pub fn new(
        task_store: Entity<TaskStore>,
        // ui_store: Entity<UiStateStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
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

        // let mut subscriptions = Vec::new();

        // // Subscribe to task store events
        // subscriptions.push(cx.subscribe(
        //     &task_store,
        //     |this, _task_store, _event: &TasksUpdated, cx| {
        //         this.update_pipeline(cx);
        //         cx.notify();
        //     },
        // ));

        // subscriptions.push(cx.subscribe(
        //     &task_store,
        //     |this, _task_store, _event: &TaskCreated, cx| {
        //         this.update_pipeline(cx);
        //         cx.notify();
        //     },
        // ));

        // subscriptions.push(cx.subscribe(
        //     &task_store,
        //     |this, _task_store, _event: &TaskDeleted, cx| {
        //         this.update_pipeline(cx);
        //         cx.notify();
        //     },
        // ));

        // subscriptions.push(cx.subscribe(
        //     &task_store,
        //     |_this, _task_store, event: &ApiError, cx| {
        //         eprintln!("TaskListView: API Error: {}", event.message);
        //         cx.notify();
        //     },
        // ));

        // Subscribe to task store events
        cx.subscribe(
            &task_store,
            |this, _task_store, _event: &TasksUpdated, cx| {
                this.update_pipeline(cx);
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(
            &task_store,
            |this, _task_store, _event: &TaskCreated, cx| {
                this.update_pipeline(cx);
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(
            &task_store,
            |this, _task_store, _event: &TaskDeleted, cx| {
                this.update_pipeline(cx);
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(&task_store, |_this, _task_store, event: &ApiError, cx| {
            eprintln!("TaskListView: API Error: {}", event.message);
            cx.notify();
        })
        .detach();

        // let options = GalleryOptions {
        //     axis: gpui::Axis::Vertical,
        //     max_item_size: size(px(200.), px(100.)),
        //     transition_duration: Duration::from_millis(350),
        //     ..Default::default()
        // };

        // let pipeline_list = cx.new(|cx| Pipeline::new(task_store.clone(), cx));
        let pipeline_list = cx.new(|cx| Pipeline {
            task_store: task_store.clone(),
            task_data: vec![],
        });

        Self {
            // title_input,
            // desc_input,
            // ui_store,
            task_store,
            collapsed: false,
            pipeline: pipeline_list, // last_selected_task_id: None,
                                     // _subscriptions: vec![],
        }
    }

    // fn ensure_gallery_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    //     let delegate =
    //             // TaskListDelegate::new(self.task_store.clone(), self.ui_store.clone(), cx);
    //             Pipeline::new(self.task_store.clone(), cx);
    //     // let list_state = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));
    //     let gallery_state = cx.new(|cx| GalleryState::new(delegate, window, cx));

    //     // Subscribe to list events
    //     // self._subscriptions.push(cx.subscribe(
    //     //     &list_state,
    //     //     |this, _list_state, event: &ListEvent, cx| {
    //     //         match event {
    //     //             ListEvent::Select(_ix) => {
    //     //                 // Selection via keyboard - don't open details, just update
    //     //             }
    //     //             ListEvent::Confirm(ix) => {
    //     //                 // Handle Enter key - open details and select task
    //     //                 this.handle_task_click(*ix, cx);
    //     //             }
    //     //             ListEvent::Cancel => {
    //     //                 // Selection cancelled
    //     //             }
    //     //         }
    //     //         cx.notify();
    //     //     },
    //     // ));

    //     self.list_state = Some(list_state);
    // }

    fn update_pipeline(&mut self, cx: &mut Context<Self>) {
        self.pipeline.update(cx, |pipeline, cx| {
            pipeline.update_tasks(cx);
            cx.notify();
        });
    }

    pub fn toggle_collapsed(&mut self, cx: &mut Context<Self>) -> bool {
        self.collapsed = !self.collapsed;
        cx.notify();
        self.collapsed
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_2()
            .pl_1()
            .bg(cx.theme().secondary)
            .child(
                div()
                    .size_full()
                    .bg(cx.theme().background)
                    .rounded_lg()
                    .child(
                        // right sidebar content
                        v_flex()
                            .size_full()
                            .pt_4()
                            .gap_3()
                            .items_center()
                            // .child(
                            //     Label::new("Settings")
                            //         .text_lg()
                            //         .font(gpui::font("Georgia")),
                            // )
                            // .child(Divider::horizontal())
                            // .child(
                            //     Switch::new("dark-mode-switch")
                            //         .checked(cx.theme().is_dark())
                            //         .label("Dark mode")
                            //         .on_click(cx.listener(
                            //             |_view, _checked, _, cx| {
                            //                 // view.is_enabled = *checked;
                            //                 cx.notify();
                            //             },
                            //         )),
                            // )
                            // .child(
                            //     Switch::new("alerts-switch")
                            //         .checked(true)
                            //         .label("Enable alerts")
                            //         .on_click(cx.listener(
                            //             |_view, _checked, _, cx| {
                            //                 // view.is_enabled = *checked;
                            //                 cx.notify();
                            //             },
                            //         )),
                            // ), // .child(Divider::horizontal()),
                            .child(Label::new("Pipeline").text_lg().font(font("Georgia")))
                            .child(self.pipeline.clone()),
                    ),
            )
    }
}
