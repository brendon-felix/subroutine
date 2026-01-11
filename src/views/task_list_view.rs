use gpui::{
    App, Context, ElementId, Entity, EventEmitter, IntoElement, Render, Styled, Subscription,
    Window, div,
};
use gpui::{Edges, prelude::*, px};
// use gpui_component::checkbox::Checkbox;
use gpui_component::label::Label;
// use gpui_component::list::{List, ListDelegate, ListEvent, ListItem, ListState};
// use gpui_component::skeleton::Skeleton;
use gpui_component::{ActiveTheme, Selectable, StyledExt, h_flex};
// use ticks::tasks::TaskPriority;

use crate::components::checkbox::Checkbox;
use crate::components::custom_list::{List, ListDelegate, ListEvent, ListItem, ListState};
// use crate::list_rewrite::{List, ListDelegate, ListEvent, ListItem, ListState};
use crate::stores::TaskStore;
use crate::stores::task_store::{ApiError, TaskCreated, TaskDeleted, TasksUpdated};
// use crate::stores::ui_store::{
//     DetailsOpenRequested, TaskSelected, UiStateChanged, UiStateStore, ViewChanged,
// };
use crate::tasks::{TaskData, task_data_compare};

pub struct TaskListDelegate {
    tasks_data: Vec<TaskData>,
    filtered_tasks: Vec<TaskData>,
    is_searching: bool,
    search_query: String,
    selected_index: Option<usize>,
    task_store: Entity<TaskStore>,
    // ui_store: Entity<UiStateStore>,
}

impl TaskListDelegate {
    // pub fn new(task_store: Entity<TaskStore>, ui_store: Entity<UiStateStore>, cx: &App) -> Self {
    pub fn new(task_store: Entity<TaskStore>, cx: &App) -> Self {
        // let current_view = ui_store.read(cx).current_view.clone();
        let mut tasks_data: Vec<TaskData> = task_store
            .read(cx)
            // .get_filtered_tasks(&current_view)
            .get_all_tasks()
            .into_iter()
            .cloned()
            .collect();
        tasks_data.sort_by(task_data_compare);
        Self {
            tasks_data,
            filtered_tasks: Vec::new(),
            is_searching: false,
            search_query: String::new(),
            selected_index: None,
            task_store,
            // ui_store,
        }
    }

    pub fn update_tasks(&mut self, cx: &App) {
        // let current_view = self.ui_store.read(cx).current_view.clone();
        self.tasks_data = self
            .task_store
            .read(cx)
            // .get_filtered_tasks(&current_view)
            .get_all_tasks()
            .into_iter()
            .cloned()
            .collect();
        self.tasks_data.sort_by(task_data_compare);

        // If we're searching, reapply the filter to the updated tasks
        if self.is_searching && !self.search_query.is_empty() {
            self.filtered_tasks = self
                .tasks_data
                .iter()
                .filter(|task| {
                    let title = task
                        .title
                        .as_ref()
                        .map(|t| t.to_lowercase())
                        .unwrap_or_default();
                    title.contains(&self.search_query.to_lowercase())
                })
                .cloned()
                .collect();
        } else {
            self.is_searching = false;
            self.filtered_tasks.clear();
        }
    }

    fn current_tasks(&self) -> &[TaskData] {
        if self.is_searching {
            &self.filtered_tasks
        } else {
            &self.tasks_data
        }
    }

    // pub fn get_selected_task(&self) -> Option<&TaskData> {
    //     self.selected_index
    //         .and_then(|ix| self.tasks_data.get(ix.row))
    // }
}

impl ListDelegate for TaskListDelegate {
    fn items_count(&self, _cx: &App) -> usize {
        self.current_tasks().len()
    }

    fn render_item(
        &mut self,
        ix: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<TaskListDelegate>>,
    ) -> Option<ListItem> {
        self.current_tasks().get(ix).map(|task| {
            let default_title = "Untitled Task".to_string();
            let title = task
                .title
                .as_ref()
                .unwrap_or(&default_title)
                .replace('\n', " ")
                .replace('\r', " ");
            let is_selected = Some(ix) == self.selected_index;

            // let theme = cx.theme();

            // // Choose icon based on priority
            // let icon_color = match task.priority {
            //     Some(TaskPriority::High) => rgb(0xff4444),
            //     Some(TaskPriority::Medium) => rgb(0xffaa00),
            //     Some(TaskPriority::Low) => rgb(0x4444ff),
            //     _ => rgb(0x888888),
            // };

            ListItem::new(ix)
                .rounded_md()
                .child(
                    h_flex()
                        .items_center()
                        .py_2()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_3()
                                .min_w_0()
                                .flex_1()
                                // .child(Icon::new(IconName::CircleCheck).text_color(icon_color))
                                .child(
                                    Checkbox::new(ElementId::NamedInteger(
                                        "checkbox".into(),
                                        ix as u64,
                                    ))
                                    // .bg(theme.background)
                                    // .checked(task..is_some())
                                    // .border_color(gpui::rgb(0xFF0000))
                                    .on_click(
                                        |checked, _window, cx| {
                                            cx.stop_propagation();
                                            println!("Checkbox clicked: {}", checked);
                                        },
                                    ), // .on_hover(cx.listener(|this, e, window, cx| {
                                       //     cx.notify();
                                       // }))
                                       // .hover(|style| style.border_color(gpui::rgb(0xFF0000))),
                                )
                                .child(Label::new(title).truncate()),
                        )
                        // .child(div().flex_1())
                        // .when_some(task.due_date.as_ref(), |this, due_date| {
                        //     this.child(
                        //         Label::new(due_date.format("%m/%d").to_string())
                        //             .text_xs()
                        //             .text_color(rgb(0x888888)),
                        //     )
                        // })
                        .child(div().w_2()),
                )
                .selected(is_selected)
                .on_click({
                    // let task = task.clone();
                    // let ui_store = self.ui_store.clone();
                    cx.listener(move |list_state, _event, _window, cx| {
                        // Update delegate selection
                        list_state.delegate_mut().selected_index = Some(ix);
                        // Update UI store with selected task and request details pane to open
                        // ui_store.update(cx, |ui_store, cx| {
                        //     ui_store.set_selected_task(Some(task.clone()));
                        //     cx.emit(TaskSelected);
                        //     cx.emit(DetailsOpenRequested);
                        // });
                        cx.notify();
                    })
                })
        })
    }

    fn set_selected_index(
        &mut self,
        ix: Option<usize>,
        // _window: &mut Window,
        // cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
    }

    // fn render_empty(
    //     &mut self,
    //     _window: &mut Window,
    //     cx: &mut gpui::Context<gpui_component::list::ListState<TaskListDelegate>>,
    // ) -> impl IntoElement {
    //     v_flex()
    //         .size_full()
    //         .justify_center()
    //         .items_center()
    //         .gap_2()
    //         .child(Icon::new(IconName::Search).text_color(rgb(0x888888)))
    //         .child(Label::new("No tasks found").text_color(rgb(0x888888)))
    //         .child(
    //             Label::new("Your tasks will appear here")
    //                 .text_sm()
    //                 .text_color(rgb(0x666666)),
    //         )
    // }
}

pub struct TaskListView {
    task_store: Entity<TaskStore>,
    // ui_store: Entity<UiStateStore>,
    list_state: Option<Entity<ListState<TaskListDelegate>>>,
    _subscriptions: Vec<Subscription>,
}

impl TaskListView {
    pub fn new(
        task_store: Entity<TaskStore>,
        // ui_store: Entity<UiStateStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        // Subscribe to task store events
        subscriptions.push(cx.subscribe(
            &task_store,
            |this, _task_store, _event: &TasksUpdated, cx| {
                this.update_task_list(cx);
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &task_store,
            |this, _task_store, _event: &TaskCreated, cx| {
                this.update_task_list(cx);
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &task_store,
            |this, _task_store, _event: &TaskDeleted, cx| {
                this.update_task_list(cx);
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &task_store,
            |_this, _task_store, event: &ApiError, cx| {
                eprintln!("TaskListView: API Error: {}", event.message);
                cx.notify();
            },
        ));

        Self {
            task_store,
            // ui_store,
            list_state: None,
            _subscriptions: subscriptions,
        }
    }

    fn update_task_list(&mut self, cx: &mut Context<Self>) {
        if let Some(list_state) = &self.list_state {
            list_state.update(cx, |list_state, cx| {
                list_state.delegate_mut().update_tasks(cx);
                cx.notify();
            });
        }
    }

    // fn clear_selection(&mut self, cx: &mut Context<Self>) {
    //     // Recreate the list state to reset all internal selection state
    //     self.list_state = None;
    //     cx.notify();
    // }

    fn ensure_list_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.list_state.is_none() {
            let delegate =
                // TaskListDelegate::new(self.task_store.clone(), self.ui_store.clone(), cx);
                TaskListDelegate::new(self.task_store.clone(), cx);
            // let list_state = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));
            let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

            // Subscribe to list events
            self._subscriptions.push(cx.subscribe(
                &list_state,
                |this, _list_state, event: &ListEvent, cx| {
                    match event {
                        ListEvent::Select(_ix) => {
                            // Selection via keyboard - don't open details, just update
                        }
                        ListEvent::Confirm(ix) => {
                            // Handle Enter key - open details and select task
                            this.handle_task_click(*ix, cx);
                        }
                        ListEvent::Cancel => {
                            // Selection cancelled
                        }
                    }
                    cx.notify();
                },
            ));

            self.list_state = Some(list_state);
        }
    }

    fn handle_task_click(&mut self, _ix: usize, _cx: &mut Context<Self>) {
        // Get the clicked task
        // if let Some(list_state) = &self.list_state {
        //     let task = list_state
        //         .read(cx)
        //         .delegate()
        //         .current_tasks()
        //         .get(ix)
        //         .cloned();

        //     if let Some(task) = task {
        //         // Update UI store with selected task and request details pane to open
        //         self.ui_store.update(cx, |ui_store, cx| {
        //             ui_store.set_selected_task(Some(task));
        //             cx.emit(TaskSelected);
        //             cx.emit(DetailsOpenRequested);
        //         });
        //     }
        // }
    }
}

impl EventEmitter<()> for TaskListView {}

impl Render for TaskListView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_list_state(window, cx);

        let list_state = self.list_state.as_ref().unwrap();
        div()
            .id("task-list")
            .size_full()
            .flex()
            .flex_col()
            // .p_4()
            .gap_3()
            .p_0p5()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .overflow_y_hidden()
                    .size_full()
                    // .child(List::new(list_state).scrollbar_visible(false)),
                    // .child(List::new(list_state).paddings(Edges::all(px(14.0)))),
                    .child(List::new(list_state)),
            )
    }
}

// #[derive(IntoElement)]
// pub struct Loading;

// #[derive(IntoElement)]
// struct LoadingItem;

// impl RenderOnce for LoadingItem {
//     fn render(self, _window: &mut gpui::Window, _cx: &mut gpui::App) -> impl IntoElement {
//         ListItem::new("skeleton")
//             .disabled(true)
//             .child(Skeleton::new().h_10().w_full())
//     }
// }

// impl RenderOnce for Loading {
//     fn render(self, _window: &mut gpui::Window, _cx: &mut gpui::App) -> impl IntoElement {
//         v_flex()
//             // .py_2p5()
//             // .gap_3()
//             // .child(LoadingItem)
//             .children((0..5).map(|_| LoadingItem))
//     }
// }
