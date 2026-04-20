use gpui::{
    App, Context, DragMoveEvent, Entity, FocusHandle, Focusable, IntoElement, Render, Styled,
    Subscription, Window, div, prelude::FluentBuilder, px,
};
use gpui::{Pixels, Point, prelude::*};
use gpui_component::button::ButtonVariants;
use gpui_component::{ActiveTheme, button::Button, h_flex, label::Label, v_flex};
use gpui_component::{IconName, Sizable};
use simple_core::{Action, Event};

use crate::components::drag_drop::{DragData, DropZone};

use crate::views::{StartActionCreator, StartEventCreator};
use crate::{
    stores::DatabaseStore,
    stores::database_store::PipelineChanged,
    views::{
        BacklogListView, FocusView, NavigateFromFocus, NavigateToView, Pipeline, RoutinesView,
        routines_view::NavigateFromRoutines, routines_view::StartRoutineEditor,
    },
};

// pub enum DragItem {
//     PipelineAction(Action),
//     PipelineEvent(Event),
//     BacklogAction(Action),
//     SavedAction(Action),
//     SavedEvent(Event),
// }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainViewMode {
    Home,
    Focus,
    ActionList,
    Routines,
    // Completions,
}

#[derive(Clone, Copy, PartialEq)]
enum ActiveDrop {
    Pipeline,
    Backlog,
}

pub struct MainView {
    pub focus_handle: FocusHandle,
    pub focus_view: Entity<FocusView>,
    pub action_list: Entity<BacklogListView>,
    pub routines_view: Entity<RoutinesView>,
    // pub completions_view: Entity<CompletionsView>,
    pub pipeline: Entity<Pipeline>,
    database_store: Entity<DatabaseStore>,
    _subscriptions: Vec<Subscription>,
    pub mode: MainViewMode,
    active_action_drop: Option<ActiveDrop>,
    active_event_drop: Option<ActiveDrop>,
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
        // let completions_view =
        //     cx.new(|cx| CompletionsView::new(database_store.clone(), window, cx));
        let pipeline = cx.new(|cx| Pipeline::new(database_store.clone(), window, cx));

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
            |this, _store, _event: &PipelineChanged, window, cx| {
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
            database_store,
            _subscriptions: subscriptions,
            mode: MainViewMode::Home,
            active_action_drop: None,
            active_event_drop: None,
        }
    }

    pub fn set_mode(&mut self, mode: MainViewMode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    pub fn render_pipeline(&self, cx: &Context<Self>) -> impl IntoElement + Styled {
        let pipeline = self.pipeline.clone();
        let active_pipeline_drop = self.active_action_drop == Some(ActiveDrop::Pipeline)
            || self.active_event_drop == Some(ActiveDrop::Pipeline);

        v_flex()
            .id("home-pipeline-scroll")
            .size_full()
            .overflow_y_scroll()
            .bg(cx.theme().group_box)
            // .border_1()
            // .border_color(if active_pipeline_drop {
            //     cx.theme().primary
            // } else {
            //     cx.theme().border
            // })
            .when(active_pipeline_drop, |this| {
                this.bg(cx.theme().primary.opacity(0.05))
            })
            .rounded_md()
            .child(pipeline)
    }

    pub fn render_backlog(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .id("home-backlog")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .w_full()
            .rounded_lg()
            .bg(cx.theme().group_box)
            // .border_1()
            // .border_color(if active_pipeline_drop {
            //     cx.theme().primary
            // } else {
            //     cx.theme().border
            // })
            .when(
                self.active_action_drop == Some(ActiveDrop::Backlog),
                |this| this.bg(cx.theme().primary.opacity(0.05)),
            )
            .items_center()
            .justify_center()
            .child(Label::new("Backlog").text_color(cx.theme().muted_foreground))
    }

    pub fn render_pipeline_droppable(
        &self,
        mouse_position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        // Outer drop zone — accepts Action drags
        DropZone::<DragData<Action>>::new("home-pipeline-action-drop-zone")
            .min_h_32()
            .flex_1()
            .w_full()
            .rounded_lg()
            .border_1()
            .when(
                self.active_action_drop != Some(ActiveDrop::Pipeline),
                |this| this.border_color(cx.theme().border).border_dashed(),
            )
            .active(self.active_action_drop == Some(ActiveDrop::Pipeline))
            .on_drop(cx.listener(|this, data: &DragData<Action>, _window, cx| {
                let id = data.data.id;
                this.database_store.update(cx, |store, cx| {
                    store.promote_action(id, cx);
                });
                this.active_action_drop = None;
                cx.notify();
            }))
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DragData<Action>>, _window, cx| {
                    let is_over = event.bounds.contains(&mouse_position);
                    if is_over != (this.active_action_drop == Some(ActiveDrop::Pipeline)) {
                        this.active_action_drop = if is_over {
                            Some(ActiveDrop::Pipeline)
                        } else {
                            None
                        };
                        cx.notify();
                    }
                },
            ))
            // Inner drop zone — accepts Event drags
            .child(
                DropZone::<DragData<Event>>::new("home-pipeline-event-drop-zone")
                    .size_full()
                    .rounded_lg()
                    .active(self.active_event_drop == Some(ActiveDrop::Pipeline))
                    .on_drop(cx.listener(|this, data: &DragData<Event>, _window, cx| {
                        let event = data.data.clone();
                        this.database_store.update(cx, |store, cx| {
                            store.add_event_to_queue(event, cx);
                        });
                        this.active_event_drop = None;
                        cx.notify();
                    }))
                    .on_drag_move(cx.listener(
                        move |this, event: &DragMoveEvent<DragData<Event>>, _window, cx| {
                            let is_over = event.bounds.contains(&mouse_position);
                            if is_over != (this.active_event_drop == Some(ActiveDrop::Pipeline)) {
                                this.active_event_drop = if is_over {
                                    Some(ActiveDrop::Pipeline)
                                } else {
                                    None
                                };
                                cx.notify();
                            }
                        },
                    ))
                    .child(self.render_pipeline(cx)),
            )
    }

    pub fn render_backlog_droppable(
        &self,
        mouse_position: Point<Pixels>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let active_backlog_drop = self.active_action_drop == Some(ActiveDrop::Backlog)
            || self.active_event_drop == Some(ActiveDrop::Backlog);

        DropZone::<DragData<Action>>::new("home-backlog-action-drop-zone")
            .flex_basis(px(100.))
            .rounded_lg()
            .h_full()
            .when(
                self.active_action_drop != Some(ActiveDrop::Backlog),
                |this| this.border_color(cx.theme().border).border_dashed(),
            )
            .active(active_backlog_drop)
            .on_drop(cx.listener(|this, data: &DragData<Action>, _window, cx| {
                let id = data.data.id;
                this.database_store.update(cx, |store, cx| {
                    // store.add_action_to_queue(action, cx);
                    store.demote_action(id, cx);
                });
                this.active_action_drop = None;
                cx.notify();
            }))
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DragData<Action>>, _window, cx| {
                    let is_over = event.bounds.contains(&mouse_position);
                    if is_over != (this.active_action_drop == Some(ActiveDrop::Backlog)) {
                        this.active_action_drop = if is_over {
                            Some(ActiveDrop::Backlog)
                        } else {
                            None
                        };
                        cx.notify();
                    }
                },
            ))
            .child(self.render_backlog(cx))
    }

    pub fn render_home(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + Styled {
        // let pipeline = self.pipeline.clone();
        let mouse_position = window.mouse_position();
        v_flex()
            // .debug_red()
            .absolute()
            .inset_0()
            .items_center()
            // Add-item buttons pinned at the top
            .child(
                h_flex()
                    .w_full()
                    .max_w(px(860.0))
                    .px_4()
                    .pt_4()
                    .pb_3()
                    .gap_2()
                    .child(
                        Button::new("new-action-plus")
                            .size_8()
                            .rounded_full()
                            .outline()
                            .icon(IconName::Plus)
                            .on_click(|_event, window, cx| {
                                window.dispatch_action(Box::new(StartActionCreator), cx);
                            }),
                    )
                    .child(
                        Button::new("new-action")
                            .rounded_full()
                            .outline()
                            .label("New action")
                            .on_click(|_event, window, cx| {
                                window.dispatch_action(Box::new(StartActionCreator), cx);
                            }),
                    )
                    .child(
                        Button::new("new-event")
                            .rounded_full()
                            .outline()
                            .label("New event")
                            .on_click(|_event, window, cx| {
                                window.dispatch_action(Box::new(StartEventCreator), cx);
                            }),
                    ),
            )
            .child(
                v_flex()
                    // .debug_green()
                    .max_w(px(860.0))
                    .w_full()
                    .px_4()
                    .pb_4()
                    .gap_4()
                    .child(
                        h_flex()
                            .w_full()
                            .pb_2()
                            .justify_between()
                            .items_center()
                            .child(
                                Label::new("Pipeline")
                                    .text_sm()
                                    .text_color(cx.theme().foreground),
                            )
                            .child(
                                Button::new("home-refresh-pipeline")
                                    .icon(IconName::Replace)
                                    .ghost()
                                    .xsmall()
                                    .tooltip("Refresh pipeline")
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.database_store.update(cx, |store, cx| {
                                            store.refresh_pipeline(cx);
                                        });
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .size_full()
                            .gap_4()
                            .child(self.render_pipeline_droppable(mouse_position, cx))
                            .child(
                                v_flex().child(self.render_backlog_droppable(mouse_position, cx)),
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().size_full().map(|this| match self.mode {
            MainViewMode::Home => this.child(self.render_home(window, cx)),
            MainViewMode::Focus => this.child(self.focus_view.clone()),
            MainViewMode::ActionList => this.child(self.action_list.clone()),
            MainViewMode::Routines => this.child(self.routines_view.clone()),
            // MainViewMode::Completions => this.child(self.completions_view.clone()),
        })
    }
}
