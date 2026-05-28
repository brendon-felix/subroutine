use gpui::prelude::*;
use gpui::{
    Context, DragMoveEvent, Entity, EventEmitter, IntoElement, Render, Subscription, Window, div,
    px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::label::Label;
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};
use simple_core::Action;

use crate::components::drag_drop::{DragData, DropZone};
use crate::icons::AppIcon;
use crate::stores::AppDatabaseStore;
use crate::stores::database_store::DataChanged;

use crate::views::PipelineView;

pub struct LeftSidebarView {
    collapsed: bool,
    pub pipeline: Entity<PipelineView>,
    database_store: Entity<AppDatabaseStore>,
    drop_active: bool,
    _subscriptions: Vec<Subscription>,
}

impl LeftSidebarView {
    pub fn new(
        database_store: Entity<AppDatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let pipeline = cx.new(|cx| PipelineView::new(database_store.clone(), window, cx));

        let mut subscriptions = Vec::new();

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
            collapsed: false,
            pipeline,
            database_store,
            drop_active: false,
            _subscriptions: subscriptions,
        }
    }

    // pub fn toggle_collapsed(&mut self, cx: &mut Context<Self>) -> bool {
    //     self.collapsed = !self.collapsed;
    //     cx.notify();
    //     self.collapsed
    // }

    // pub fn is_collapsed(&self) -> bool {
    //     self.collapsed
    // }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let queue_len = self.database_store.read(cx).sorted_queue().len();
        let backlog_len = self.database_store.read(cx).backlogged_actions().len();

        h_flex()
            .w_full()
            .px_3()
            .py_2()
            .justify_between()
            .items_center()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Label::new("Pipeline")
                            .text_sm()
                            .text_color(theme.foreground),
                    )
                    .child(
                        div()
                            .px_2()
                            .py(px(1.0))
                            .rounded_full()
                            .bg(theme.accent.opacity(0.15))
                            .child(
                                Label::new(format!("{}", queue_len))
                                    .text_xs()
                                    .text_color(theme.accent_foreground),
                            ),
                    ),
            )
            .when(!self.collapsed, |this| {
                this.child(
                    Label::new(format!("{} backlogged", backlog_len))
                        .text_xs()
                        .text_color(theme.muted_foreground),
                )
            })
    }
}

impl EventEmitter<()> for LeftSidebarView {}

impl Render for LeftSidebarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drop_active = self.drop_active;
        let pipeline = self.pipeline.clone();
        let mouse_position = window.mouse_position();

        v_flex()
            .size_full()
            .child(self.render_header(cx))
            .when(!self.collapsed, |this| {
                // Outer drop zone fills remaining space so the whole sidebar accepts drops.
                this.child(
                    DropZone::<DragData<Action>>::new("pipeline-queue-drop-zone")
                        .bg(cx.theme().secondary)
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .items_stretch()
                        .active(drop_active)
                        .on_drop(cx.listener(|this, data: &DragData<Action>, _window, cx| {
                            let action_id = data.data.id;
                            this.database_store.update(cx, |store, cx| {
                                store.auto_queue_action(action_id, cx);
                            });
                            this.drop_active = false;
                            cx.notify();
                        }))
                        .on_drag_move(cx.listener(
                            move |this, event: &DragMoveEvent<DragData<Action>>, _window, cx| {
                                let is_over = event.bounds.contains(&mouse_position);
                                if is_over != this.drop_active {
                                    this.drop_active = is_over;
                                    cx.notify();
                                }
                            },
                        ))
                        // Center the pipeline items vertically with a constrained scrollable box.
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .size_full()
                                .items_center()
                                .justify_start()
                                .pt_8()
                                .child(
                                    h_flex().w_full().px_2().pb_2().justify_end().child(
                                        Button::new("refresh-pipeline")
                                            .icon(AppIcon::RefreshCcw)
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
                                    div()
                                        .id("pipeline-scroll")
                                        .bg(cx.theme().group_box)
                                        .overflow_y_scroll()
                                        .size_full()
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .rounded_xl()
                                        .child(pipeline),
                                ),
                        ),
                )
            })
    }
}
