use gpui::{
    Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{IconName, button::Button, h_flex, v_flex};

use crate::{
    // components::drag_drop::{DragData, DropZone},
    stores::AppDatabaseStore,
    views::{StartActionCreator, StartEventCreator},
};

// pub enum DragItem {
//     PipelineAction(Action),
//     PipelineEvent(Event),
//     BacklogAction(Action),
//     SavedAction(Action),
//     SavedEvent(Event),
// }

// #[derive(Clone, Copy, PartialEq)]
// enum ActiveDrop {
//     Pipeline,
//     Backlog,
// }

pub struct DashboardView {
    database_store: Entity<AppDatabaseStore>,
    // active_action_drop: Option<ActiveDrop>,
    // active_event_drop: Option<ActiveDrop>,
}

impl DashboardView {
    pub fn new(
        database_store: Entity<AppDatabaseStore>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self { database_store }
    }

    // pub fn render_pipeline(&self, cx: &Context<Self>) -> impl IntoElement + Styled {
    //     // let pipeline = self.pipeline.clone();
    //     // let active_pipeline_drop = self.active_action_drop == Some(ActiveDrop::Pipeline)
    //     //     || self.active_event_drop == Some(ActiveDrop::Pipeline);

    //     v_flex()
    //         .id("home-pipeline-scroll")
    //         .size_full()
    //         .overflow_y_scroll()
    //         .bg(cx.theme().group_box)
    //         // .border_1()
    //         // .border_color(if active_pipeline_drop {
    //         //     cx.theme().primary
    //         // } else {
    //         //     cx.theme().border
    //         // })
    //         // .when(active_pipeline_drop, |this| {
    //         //     this.bg(cx.theme().primary.opacity(0.05))
    //         // })
    //         .rounded_md()
    //     // .child(pipeline)
    // }

    // pub fn render_backlog(&self, cx: &Context<Self>) -> impl IntoElement {
    //     div()
    //         .id("home-backlog")
    //         .flex()
    //         .flex_col()
    //         .flex_1()
    //         .min_h_0()
    //         .w_full()
    //         .rounded_lg()
    //         .bg(cx.theme().group_box)
    //         // .border_1()
    //         // .border_color(if active_pipeline_drop {
    //         //     cx.theme().primary
    //         // } else {
    //         //     cx.theme().border
    //         // })
    //         // .when(
    //         //     self.active_action_drop == Some(ActiveDrop::Backlog),
    //         //     |this| this.bg(cx.theme().primary.opacity(0.05)),
    //         // )
    //         .items_center()
    //         .justify_center()
    //         .child(Label::new("Backlog").text_color(cx.theme().muted_foreground))
    // }

    // pub fn render_pipeline_droppable(
    //     &self,
    //     mouse_position: Point<Pixels>,
    //     cx: &Context<Self>,
    // ) -> impl IntoElement {
    //     // Outer drop zone — accepts Action drags
    //     DropZone::<DragData<Action>>::new("home-pipeline-action-drop-zone")
    //         .min_h_32()
    //         .flex_1()
    //         .w_full()
    //         .rounded_lg()
    //         .border_1()
    //     // .when(
    //     //     self.active_action_drop != Some(ActiveDrop::Pipeline),
    //     //     |this| this.border_color(cx.theme().border).border_dashed(),
    //     // )
    //     // .active(self.active_action_drop == Some(ActiveDrop::Pipeline))
    //     // .on_drop(cx.listener(|this, data: &DragData<Action>, _window, cx| {
    //     //     let id = data.data.id;
    //     //     this.database_store.update(cx, |store, cx| {
    //     //         store.promote_action(id, cx);
    //     //     });
    //     //     this.active_action_drop = None;
    //     //     cx.notify();
    //     // }))
    //     // .on_drag_move(cx.listener(
    //     //     move |this, event: &DragMoveEvent<DragData<Action>>, _window, cx| {
    //     //         let is_over = event.bounds.contains(&mouse_position);
    //     //         if is_over != (this.active_action_drop == Some(ActiveDrop::Pipeline)) {
    //     //             this.active_action_drop = if is_over {
    //     //                 Some(ActiveDrop::Pipeline)
    //     //             } else {
    //     //                 None
    //     //             };
    //     //             cx.notify();
    //     //         }
    //     //     },
    //     // ))
    //     // // Inner drop zone — accepts Event drags
    //     // .child(
    //     //     DropZone::<DragData<Event>>::new("home-pipeline-event-drop-zone")
    //     //         .size_full()
    //     //         .rounded_lg()
    //     //         .active(self.active_event_drop == Some(ActiveDrop::Pipeline))
    //     //         .on_drop(cx.listener(|this, data: &DragData<Event>, _window, cx| {
    //     //             let event = data.data.clone();
    //     //             this.database_store.update(cx, |store, cx| {
    //     //                 store.add_event_to_queue(event, cx);
    //     //             });
    //     //             this.active_event_drop = None;
    //     //             cx.notify();
    //     //         }))
    //     //         .on_drag_move(cx.listener(
    //     //             move |this, event: &DragMoveEvent<DragData<Event>>, _window, cx| {
    //     //                 let is_over = event.bounds.contains(&mouse_position);
    //     //                 if is_over != (this.active_event_drop == Some(ActiveDrop::Pipeline)) {
    //     //                     this.active_event_drop = if is_over {
    //     //                         Some(ActiveDrop::Pipeline)
    //     //                     } else {
    //     //                         None
    //     //                     };
    //     //                     cx.notify();
    //     //                 }
    //     //             },
    //     //         ))
    //     //         .child(self.render_pipeline(cx)),
    //     // )
    // }

    // pub fn render_backlog_droppable(
    //     &self,
    //     mouse_position: Point<Pixels>,
    //     cx: &Context<Self>,
    // ) -> impl IntoElement {
    //     // let active_backlog_drop = self.active_action_drop == Some(ActiveDrop::Backlog)
    //     //     || self.active_event_drop == Some(ActiveDrop::Backlog);

    //     DropZone::<DragData<Action>>::new("home-backlog-action-drop-zone")
    //         .flex_basis(px(100.))
    //         .rounded_lg()
    //         .h_full()
    //     // .when(
    //     //     self.active_action_drop != Some(ActiveDrop::Backlog),
    //     //     |this| this.border_color(cx.theme().border).border_dashed(),
    //     // )
    //     // .active(active_backlog_drop)
    //     // .on_drop(cx.listener(|this, data: &DragData<Action>, _window, cx| {
    //     //     let id = data.data.id;
    //     //     this.database_store.update(cx, |store, cx| {
    //     //         // store.add_action_to_queue(action, cx);
    //     //         store.demote_action(id, cx);
    //     //     });
    //     //     this.active_action_drop = None;
    //     //     cx.notify();
    //     // }))
    //     // .on_drag_move(cx.listener(
    //     //     move |this, event: &DragMoveEvent<DragData<Action>>, _window, cx| {
    //     //         let is_over = event.bounds.contains(&mouse_position);
    //     //         if is_over != (this.active_action_drop == Some(ActiveDrop::Backlog)) {
    //     //             this.active_action_drop = if is_over {
    //     //                 Some(ActiveDrop::Backlog)
    //     //             } else {
    //     //                 None
    //     //             };
    //     //             cx.notify();
    //     //         }
    //     //     },
    //     // ))
    //     // .child(self.render_backlog(cx))
    // }
}

impl Render for DashboardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // div().child("Dashboard")
        // let pipeline = self.pipeline.clone();

        let actions = self.database_store.read(cx).actions();
        let current_action = actions.first().cloned();

        // let mouse_position = window.mouse_position();
        v_flex()
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
            // .child(
            //     v_flex()
            //         .flex_1()
            //         // .debug_green()
            //         .max_w(px(860.0))
            //         // .w_full()
            //         // .px_4()
            //         // .pb_4()
            //         // .gap_4()
            //         // .child(
            //         //     h_flex()
            //         //         .w_full()
            //         //         .pb_2()
            //         //         .justify_between()
            //         //         .items_center()
            //         //         .child(
            //         //             Label::new("Pipeline")
            //         //                 .text_sm()
            //         //                 .text_color(cx.theme().foreground),
            //         //         )
            //         //         .child(
            //         //             Button::new("home-refresh-pipeline")
            //         //                 .icon(IconName::Replace)
            //         //                 .ghost()
            //         //                 .xsmall()
            //         //                 .tooltip("Refresh pipeline")
            //         //                 .on_click(cx.listener(|this, _event, _window, cx| {
            //         //                     this.database_store.update(cx, |store, cx| {
            //         //                         store.refresh_pipeline(cx);
            //         //                     });
            //         //                 })),
            //         //         ),
            //         // )
            //         // .child(
            //         //     h_flex()
            //         //         .size_full()
            //         //         .gap_4()
            //         //         // .child(self.render_pipeline_droppable(mouse_position, cx))
            //         //         // .child(
            //         //         //     v_flex().child(self.render_backlog_droppable(mouse_position, cx)),
            //         //         // ),
            //         //         .child("Dashboard"),
            //         // ),
            // )
            .child(
                div()
                    .size_full()
                    // .debug_red()
                    .when_some(current_action, |this, action| this.child(action.title)),
            )
    }
}
