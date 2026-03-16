use gpui::prelude::*;
use gpui::{
    App, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Styled, Subscription,
    Window, div, prelude::FluentBuilder,
};
use gpui_component::button::DropdownButton;
use gpui_component::{ActiveTheme, button::Button, divider::Divider, h_flex, label::Label, v_flex};
use gpui_component::{IconName, Sizable};

use crate::views::{StartActionCreator, StartEventCreator};
use crate::{
    stores::DatabaseStore,
    views::{
        BacklogListView, CompletionsView, FocusView, NavigateFromCompletions, NavigateFromFocus,
        NavigateToView, RoutinesView, routines_view::NavigateFromRoutines,
        routines_view::StartRoutineEditor,
    },
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainViewMode {
    Home,
    Focus,
    ActionList,
    Routines,
    Completions,
}

pub struct MainView {
    pub focus_handle: FocusHandle,
    pub focus_view: Entity<FocusView>,
    pub action_list: Entity<BacklogListView>,
    pub routines_view: Entity<RoutinesView>,
    pub completions_view: Entity<CompletionsView>,
    database_store: Entity<DatabaseStore>,
    _subscriptions: Vec<Subscription>,
    pub mode: MainViewMode,
}

impl MainView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_view = cx.new(|cx| FocusView::new(database_store.clone(), window, cx));
        let action_list = cx.new(|cx| BacklogListView::new(database_store.clone(), window, cx));
        let routines_view = cx.new(|cx| RoutinesView::new(database_store.clone(), window, cx));
        let completions_view =
            cx.new(|cx| CompletionsView::new(database_store.clone(), window, cx));

        let focus_handle = cx.focus_handle();
        cx.focus_self(window);
        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &action_list,
            |this, _action_list, event: &NavigateToView, cx| {
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
                    routine_id: event.routine_id,
                });
            },
        ));

        subscriptions.push(cx.subscribe(
            &completions_view,
            |this, _completions_view, event: &NavigateFromCompletions, cx| {
                this.set_mode(event.mode, cx);
            },
        ));

        Self {
            focus_handle,
            focus_view,
            action_list,
            routines_view,
            completions_view,
            database_store,
            _subscriptions: subscriptions,
            mode: MainViewMode::Home,
        }
    }

    pub fn set_mode(&mut self, mode: MainViewMode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    pub fn is_focus_mode(&self) -> bool {
        self.mode == MainViewMode::Focus
    }

    pub fn render_home(&mut self, cx: &mut Context<Self>) -> impl IntoElement + Styled {
        let theme = cx.theme().clone();
        let queue_len = self.database_store.read(cx).pipeline.queue.len();
        let backlog_len = self.database_store.read(cx).pipeline.backlog.len();

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .absolute()
                    .left_3()
                    .top_3()
                    .gap_3()
                    .child(
                        Button::new("new-action-plus")
                            .large()
                            .size_12()
                            .rounded_full()
                            .outline()
                            .icon(IconName::Plus)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _event, window, cx| {
                                window.dispatch_action(Box::new(StartActionCreator), cx);
                            })),
                    )
                    .child(
                        Button::new("new-action")
                            .rounded_full()
                            .outline()
                            .label("New action")
                            // .icon(IconName::Plus)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _event, window, cx| {
                                window.dispatch_action(Box::new(StartActionCreator), cx);
                            })),
                    )
                    .child(
                        Button::new("new-event")
                            .rounded_full()
                            .outline()
                            .label("New event")
                            // .icon(IconName::Plus)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _event, window, cx| {
                                window.dispatch_action(Box::new(StartEventCreator), cx);
                            })),
                    ),
            )
            .child(
                v_flex().size_full().items_center().justify_center().child(
                    v_flex()
                        .items_center()
                        .gap_6()
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    Divider::horizontal()
                                        .label("Welcome to")
                                        .font(gpui::font("Georgia"))
                                        .color(theme.muted_foreground)
                                        .w_full(),
                                )
                                .child(
                                    Label::new("Subroutine")
                                        .text_3xl()
                                        .content_stretch()
                                        .font(gpui::font("Georgia")),
                                ),
                        )
                        .child(Divider::horizontal().color(theme.muted_foreground).w_full())
                        .child(
                            h_flex().gap_4().child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Label::new(format!("{}", queue_len))
                                            .text_2xl()
                                            .font(gpui::font("Georgia")),
                                    )
                                    .child(
                                        Label::new("in queue")
                                            .text_sm()
                                            .text_color(theme.muted_foreground),
                                    ),
                            ),
                        )
                        .child(
                            v_flex().gap_3().children([
                                Button::new("home-focus")
                                    .label("Focus Mode")
                                    .outline()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.set_mode(MainViewMode::Focus, cx);
                                    })),
                                Button::new("home-actions")
                                    .label("All Actions")
                                    .outline()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.set_mode(MainViewMode::ActionList, cx);
                                    })),
                                Button::new("home-routines")
                                    .label("Routines")
                                    .outline()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.set_mode(MainViewMode::Routines, cx);
                                    })),
                                Button::new("home-completions")
                                    .label("Completions")
                                    .outline()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.set_mode(MainViewMode::Completions, cx);
                                    })),
                            ]),
                        ),
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
            MainViewMode::ActionList => this.child(self.action_list.clone()),
            MainViewMode::Routines => this.child(self.routines_view.clone()),
            MainViewMode::Completions => this.child(self.completions_view.clone()),
        })
    }
}
