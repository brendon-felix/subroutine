use gpui::prelude::*;
use gpui::{
    App, Entity, FocusHandle, Focusable, IntoElement, Render, Styled, Subscription, Window, div,
    prelude::FluentBuilder,
};

use crate::views::{
    BacklogListView, DashboardView, FocusView, NavigateFromFocus, NavigateToView, PipelineView,
    RoutinesView, routines_view::NavigateFromRoutines, routines_view::StartRoutineEditor,
};

use crate::{stores::AppDatabaseStore, stores::DataChanged};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainViewMode {
    Dashboard,
    Focus,
    ActionList,
    Routines,
    // Completions,
}

pub struct MainView {
    pub focus_handle: FocusHandle,
    pub focus_view: Entity<FocusView>,
    pub action_list: Entity<BacklogListView>,
    pub routines_view: Entity<RoutinesView>,
    // pub completions_view: Entity<CompletionsView>,
    pub pipeline: Entity<PipelineView>,
    pub dashboard_view: Entity<DashboardView>,
    // database_store: Entity<DatabaseStore>,
    _subscriptions: Vec<Subscription>,
    pub mode: MainViewMode,
}

impl MainView {
    pub fn new(
        database_store: Entity<AppDatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_view = cx.new(|cx| FocusView::new(database_store.clone(), window, cx));
        let action_list = cx.new(|cx| BacklogListView::new(database_store.clone(), window, cx));
        let routines_view = cx.new(|cx| RoutinesView::new(database_store.clone(), window, cx));
        // let completions_view =
        //     cx.new(|cx| CompletionsView::new(database_store.clone(), window, cx));
        let pipeline = cx.new(|cx| PipelineView::new(database_store.clone(), window, cx));

        let dashboard_view = cx.new(|cx| DashboardView::new(database_store.clone(), window, cx));

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

        // subscriptions.push(cx.subscribe(
        //     &completions_view,
        //     |this, _completions_view, event: &NavigateFromCompletions, cx| {
        //         this.set_mode(event.mode, cx);
        //     },
        // ));

        subscriptions.push(cx.subscribe_in(
            &database_store,
            window,
            |this, _store, _event: &DataChanged, window, cx| {
                this.pipeline.update(cx, |pipeline, cx| {
                    pipeline.update_items(window, cx);
                    cx.notify();
                });
                cx.notify();
            },
        ));

        Self {
            focus_handle,
            focus_view,
            action_list,
            routines_view,
            // completions_view,
            pipeline,
            dashboard_view,
            // database_store,
            _subscriptions: subscriptions,
            mode: MainViewMode::Dashboard,
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

impl gpui::EventEmitter<StartRoutineEditor> for MainView {}

impl Render for MainView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().size_full().map(|this| match self.mode {
            MainViewMode::Dashboard => this.child(self.dashboard_view.clone()),
            MainViewMode::Focus => this.child(self.focus_view.clone()),
            MainViewMode::ActionList => this.child(self.action_list.clone()),
            MainViewMode::Routines => this.child(self.routines_view.clone()),
            // MainViewMode::Completions => this.child(self.completions_view.clone()),
        })
    }
}
