use crate::{
    stores::{
        TaskStore,
        task_store::{ApiError, TaskCreated},
    },
    views::{TaskListView, TestView},
};
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div, prelude::FluentBuilder,
};

pub enum MainViewMode {
    TaskList,
    Test,
}

pub struct MainView {
    pub focus_handle: FocusHandle,
    pub task_list: Entity<TaskListView>,
    pub test_view: Entity<TestView>,
    _subscriptions: Vec<Subscription>,
    mode: MainViewMode,
}

impl MainView {
    pub fn new(task_store: Entity<TaskStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let task_list = cx.new(|cx| TaskListView::new(task_store.clone(), cx));
        let test_view = cx.new(|cx| TestView::new(cx));

        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);
        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &task_store,
            |_this, _task_store, event: &ApiError, cx| {
                eprintln!("API Error: {}", event.message);
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &task_store,
            |_this, _task_store, _event: &TaskCreated, cx| {
                cx.notify();
            },
        ));

        Self {
            task_list,
            test_view,
            focus_handle,
            _subscriptions: subscriptions,
            mode: MainViewMode::TaskList,
        }
    }

    pub fn set_mode(&mut self, mode: MainViewMode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }
}

impl Focusable for MainView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().size_full().map(|this| match self.mode {
            MainViewMode::TaskList => this.child(self.task_list.clone()),
            MainViewMode::Test => this.child(self.test_view.clone()),
        })
    }
}
