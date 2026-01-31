use crate::{
    stores::{DatabaseStore, DragDropStore},
    views::{ActionEditor, ActionListView, NavigateToView, action_list_view},
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
    // TaskList,
    // Test,
    ActionEditor,
    ActionList,
}

pub struct MainView {
    pub focus_handle: FocusHandle,
    // pub task_list: Entity<TaskListView>,
    // pub test_view: Entity<TestView>,
    pub action_editor: Entity<ActionEditor>,
    pub action_list: Entity<ActionListView>,
    _subscriptions: Vec<Subscription>,
    mode: MainViewMode,
}

impl MainView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        drag_drop_store: Entity<DragDropStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // let task_list =
        //     cx.new(|cx| TaskListView::new(task_store.clone(), drag_drop_store.clone(), window, cx));
        // let test_view = cx.new(|cx| TestView::new(task_store.clone(), cx));

        let action_editor = cx.new(|cx| ActionEditor::new(database_store.clone(), window, cx));

        let action_list =
            cx.new(|cx| ActionListView::new(database_store.clone(), drag_drop_store, window, cx));

        let focus_handle = cx.focus_handle();
        // window.focus(&focus_handle);
        cx.focus_self(window);
        let mut subscriptions = Vec::new();

        // subscriptions.push(cx.subscribe(
        //     &task_store,
        //     |_this, _task_store, event: &ApiError, cx| {
        //         eprintln!("API Error: {}", event.message);
        //         cx.notify();
        //     },
        // ));

        // subscriptions.push(cx.subscribe(
        //     &task_store,
        //     |_this, _task_store, _event: &TaskCreated, cx| {
        //         cx.notify();
        //     },
        // ));

        subscriptions.push(cx.subscribe(
            &action_list,
            |this, _task_list, event: &NavigateToView, cx| {
                this.set_mode(event.mode, cx);
            },
        ));

        Self {
            focus_handle,
            action_editor,
            action_list,
            // drag_drop_store,
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
            .child(
                div().flex().absolute().top_2().right_2().child(
                    Button::new("home-to-tasks-btn")
                        .w(px(112.0))
                        .icon(IconName::Inbox)
                        .ghost()
                        .label("Actions")
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.set_mode(MainViewMode::ActionList, cx);
                        })),
                ),
            )
            .child(
                v_flex()
                    .items_center()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                Divider::horizontal()
                                    .label("Welcome to")
                                    .font(gpui::font("Georgia"))
                                    .color(cx.theme().muted_foreground)
                                    .w_full(),
                            )
                            .child(
                                Label::new("Subroutine")
                                    .text_3xl()
                                    .content_stretch()
                                    // .font(gpui::font("Hoefler Text")),
                                    .font(gpui::font("Georgia")),
                            ),
                    )
                    .child(
                        Divider::horizontal()
                            .color(cx.theme().muted_foreground)
                            .w_full(),
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
                    ),
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
            // MainViewMode::TaskList => this.child(self.task_list.clone()),
            // MainViewMode::Test => this.child(self.test_view.clone()),
            MainViewMode::ActionEditor => this.child(self.action_editor.clone()),
            MainViewMode::ActionList => this.child(self.action_list.clone()),
        })
    }
}
