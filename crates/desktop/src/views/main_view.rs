use app_core::starter_states;

use crate::{
    stores::{DatabaseStore, DragDropStore},
    views::{
        ActionListView, FocusView, NavigateFromFocus, NavigateToView, RoutinesView,
        routines_view::NavigateFromRoutines, routines_view::StartRoutineEditor,
        test_view::TestView,
    },
};
use gpui::AppContext as _;
use gpui::prelude::*;
use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Styled, Subscription,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, IconName, WindowExt,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    v_flex,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainViewMode {
    Home,
    Focus,
    // TaskList,
    Test,
    // ActionEditor,
    ActionList,
    Routines,
}

pub struct MainView {
    pub focus_handle: FocusHandle,
    // pub task_list: Entity<TaskListView>,
    pub test_view: Entity<TestView>,
    pub focus_view: Entity<FocusView>,
    // pub action_editor: Entity<ActionEditor>,
    pub action_list: Entity<ActionListView>,
    pub routines_view: Entity<RoutinesView>,
    database_store: Entity<DatabaseStore>,
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
        let test_view = cx.new(|cx| TestView::new(cx));

        let focus_view = cx.new(|cx| FocusView::new(database_store.clone(), window, cx));

        let action_list =
            cx.new(|cx| ActionListView::new(database_store.clone(), drag_drop_store, window, cx));

        let routines_view = cx.new(|cx| RoutinesView::new(database_store.clone(), window, cx));

        let focus_handle = cx.focus_handle();
        cx.focus_self(window);
        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &action_list,
            |this, _task_list, event: &NavigateToView, cx| {
                this.set_mode(event.mode, cx);
            },
        ));

        subscriptions.push(cx.subscribe(
            &focus_view,
            |this, _focus_view, event: &NavigateFromFocus, cx| {
                this.set_mode(event.mode, cx);
            },
        ));

        subscriptions.push(cx.subscribe(
            &routines_view,
            |this, _routines_view, event: &NavigateFromRoutines, cx| {
                this.set_mode(event.mode, cx);
            },
        ));

        subscriptions.push(cx.subscribe(
            &routines_view,
            |_this, _routines_view, event: &StartRoutineEditor, cx| {
                cx.emit(StartRoutineEditor {
                    routine_id: event.routine_id.clone(),
                });
            },
        ));

        Self {
            focus_handle,
            test_view,
            focus_view,
            action_list,
            routines_view,
            database_store,
            _subscriptions: subscriptions,
            mode: MainViewMode::Home,
        }
    }

    pub fn set_mode(&mut self, mode: MainViewMode, cx: &mut Context<Self>) {
        self.mode = mode;
        if mode == MainViewMode::Focus {
            let focus_view = self.focus_view.clone();
            cx.update_entity(&focus_view, |focus_view, cx| {
                focus_view.refresh_entries(cx);
            });
        }
        cx.notify();
    }

    pub fn is_focus_mode(&self) -> bool {
        self.mode == MainViewMode::Focus
    }

    pub fn render_home(&mut self, cx: &mut Context<Self>) -> impl IntoElement + Styled {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .child(
                div().flex().absolute().top_2().right_2().child(
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new("home-to-focus-btn")
                                .w(px(112.0))
                                .icon(IconName::Star)
                                .ghost()
                                .label("Focus")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.set_mode(MainViewMode::Focus, cx);
                                })),
                        )
                        .child(
                            Button::new("home-to-routines-btn")
                                .w(px(112.0))
                                .icon(IconName::Palette)
                                .ghost()
                                .label("Routines")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.set_mode(MainViewMode::Routines, cx);
                                })),
                        )
                        .child(
                            Button::new("home-to-tasks-btn")
                                .w(px(112.0))
                                .icon(IconName::Inbox)
                                .ghost()
                                .label("Actions")
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.set_mode(MainViewMode::ActionList, cx);
                                })),
                        ),
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
                        v_flex().gap_4().children([
                            h_flex().gap_4().justify_between().children([
                                Button::new("home-btn-0")
                                    .flex_1()
                                    .outline()
                                    .label("analysis paralysis")
                                    .text_color(cx.theme().red_light)
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.database_store.update(cx, |store, cx| {
                                            store.declare_mental_state(
                                                starter_states::SCATTERED_ID,
                                                cx,
                                            );
                                        });
                                        this.set_mode(MainViewMode::Focus, cx);
                                    })),
                                Button::new("home-btn-1")
                                    .flex_1()
                                    .outline()
                                    .label("overstimulated")
                                    .text_color(cx.theme().yellow_light)
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.database_store.update(cx, |store, cx| {
                                            store.declare_mental_state(
                                                starter_states::OVERWHELMED_ID,
                                                cx,
                                            );
                                        });
                                        this.set_mode(MainViewMode::Focus, cx);
                                    })),
                            ]),
                            h_flex().gap_4().justify_between().children([
                                Button::new("home-btn-2")
                                    .flex_1()
                                    .outline()
                                    .label("hyperfocused")
                                    .text_color(cx.theme().green_light)
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.database_store.update(cx, |store, cx| {
                                            store.declare_mental_state(
                                                starter_states::FOCUSED_ID,
                                                cx,
                                            );
                                        });
                                        this.set_mode(MainViewMode::Focus, cx);
                                    })),
                                Button::new("home-btn-3")
                                    .flex_1()
                                    .outline()
                                    .label("an intense emotion")
                                    .text_color(cx.theme().magenta_light)
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.database_store.update(cx, |store, cx| {
                                            store
                                                .declare_mental_state(starter_states::FRIED_ID, cx);
                                        });
                                        this.set_mode(MainViewMode::Focus, cx);
                                    })),
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

impl gpui::EventEmitter<StartRoutineEditor> for MainView {}

impl Render for MainView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().size_full().map(|this| match self.mode {
            MainViewMode::Home => this.child(self.render_home(cx)),
            MainViewMode::Focus => this.child(self.focus_view.clone()),
            // MainViewMode::TaskList => this.child(self.task_list.clone()),
            MainViewMode::Test => this.child(self.test_view.clone()),
            // MainViewMode::ActionEditor => this.child(self.action_editor.clone()),
            MainViewMode::ActionList => this.child(self.action_list.clone()),
            MainViewMode::Routines => this.child(self.routines_view.clone()),
        })
    }
}
