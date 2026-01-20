use crate::{
    stores::{
        TaskStore,
        task_store::{ApiError, TaskCreated},
    },
    tasks::TaskData,
    views::{TaskListView, TestView, task_list_view::NavigateToView},
};
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, IconName,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    v_flex,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainViewMode {
    Home,
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
        let task_list = cx.new(|cx| TaskListView::new(task_store.clone(), window, cx));
        let test_view = cx.new(|cx| TestView::new(task_store.clone(), cx));

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

        subscriptions.push(cx.subscribe(
            &task_list,
            |this, _task_list, event: &NavigateToView, cx| {
                this.set_mode(event.mode, cx);
            },
        ));

        Self {
            task_list,
            test_view,
            focus_handle,
            _subscriptions: subscriptions,
            mode: MainViewMode::Home,
        }
    }

    pub fn set_mode(&mut self, mode: MainViewMode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    pub fn render_home(&mut self, cx: &mut Context<Self>) -> impl IntoElement + Styled {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div().flex().absolute().top_2().right_2().child(
                    Button::new("home-to-tasks-btn")
                        .w(px(112.0))
                        .icon(IconName::Inbox)
                        .ghost()
                        .label("Task List")
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.set_mode(MainViewMode::TaskList, cx);
                        })),
                ),
            )
            .child(
                v_flex()
                    .child(
                        Label::new("Welcome to Subroutine")
                            .text_3xl()
                            .content_stretch()
                            // .font(gpui::font("Hoefler Text")),
                            .font(gpui::font("Georgia")),
                    )
                    .child(Divider::horizontal().color(cx.theme().accent).w_full()),
            )
            .child(
                Label::new("\"I'm feeling...\"")
                    .text_xl()
                    .text_color(cx.theme().muted_foreground)
                    .font(gpui::font("Georgia").italic()),
            )
            .child(
                v_flex()
                    // .font(gpui::font("Georgia").italic())
                    // .font(gpui::font("Monaco").italic())
                    .gap_4()
                    .children([
                        h_flex().gap_4().justify_between().children([
                            Button::new("home-btn-0")
                                .flex_1()
                                .outline()
                                .label("analysis paralysis")
                                .text_color(cx.theme().red_light),
                            Button::new("home-btn-1")
                                .flex_1()
                                .outline()
                                .label("overstimulated")
                                .text_color(cx.theme().yellow_light),
                        ]),
                        h_flex().gap_4().justify_between().children([
                            Button::new("home-btn-2")
                                .flex_1()
                                .outline()
                                .label("hyperfocused")
                                .text_color(cx.theme().green_light),
                            Button::new("home-btn-3")
                                .flex_1()
                                .outline()
                                .label("an instense emotion")
                                .text_color(cx.theme().magenta_light),
                        ]),
                    ]),
            )
    }
}

impl Focusable for MainView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().size_full().map(|this| match self.mode {
            MainViewMode::Home => this.child(self.render_home(cx)),
            MainViewMode::TaskList => this.child(self.task_list.clone()),
            MainViewMode::Test => this.child(self.test_view.clone()),
        })
    }
}
