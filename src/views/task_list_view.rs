use gpui::prelude::*;
use gpui::{
    Context, Entity, EventEmitter, IntoElement, Render, Styled, Subscription, Window, div, px, rgb,
};

use crate::stores::TaskStore;
use crate::stores::task_store::{ApiError, TaskCreated, TaskDeleted, TasksUpdated};
use crate::stores::ui_store::{UiStateChanged, UiStateStore};

pub struct TaskListView {
    // task_store: Entity<TaskStore>,
    ui_store: Entity<UiStateStore>,
    _subscriptions: Vec<Subscription>,
}

impl TaskListView {
    pub fn new(
        task_store: Entity<TaskStore>,
        ui_store: Entity<UiStateStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        // Subscribe to task store events
        subscriptions.push(cx.subscribe(
            &task_store,
            |_this, _task_store, _event: &TasksUpdated, cx| {
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &task_store,
            |_this, _task_store, _event: &TaskCreated, cx| {
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &task_store,
            |_this, _task_store, _event: &TaskDeleted, cx| {
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

        // Subscribe to UI state changes
        subscriptions.push(cx.subscribe(
            &ui_store,
            |_this, _ui_store, _event: &UiStateChanged, cx| {
                cx.notify();
            },
        ));

        Self {
            // task_store,
            ui_store,
            _subscriptions: subscriptions,
        }
    }

    fn render_task_item(&self, _task_id: u32, _cx: &Context<Self>) -> impl IntoElement {
        // let task_store = self.task_store.read(cx);
        div().child("Task")
    }
}

impl EventEmitter<()> for TaskListView {}

impl Render for TaskListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // let task_store = self.task_store.read(cx);
        let ui_state = self.ui_store.read(cx);

        // Filter tasks based on current view
        let filtered_tasks: Vec<u32> = match ui_state.current_view {
            _ => vec![],
        };

        // let view_title = match ui_state.current_view {
        //     ViewType::TaskList => "All Tasks",
        //     ViewType::Today => "Today & Overdue",
        //     ViewType::Upcoming => "Upcoming",
        //     ViewType::Completed => "Completed",
        // };

        div()
            .id("task-list")
            .size_full()
            .flex()
            .flex_col()
            .p_4()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .overflow_y_hidden()
                    .when(filtered_tasks.is_empty(), |element| {
                        element.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(200.0))
                                .text_color(rgb(0x888888))
                                .child("No tasks found"),
                        )
                    })
                    .when(!filtered_tasks.is_empty(), |element| {
                        let mut element = element;
                        for task_id in filtered_tasks {
                            element = element.child(self.render_task_item(task_id, cx));
                        }
                        element
                    }),
            )
    }
}
