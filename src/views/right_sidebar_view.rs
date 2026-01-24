use gpui::prelude::*;
use gpui::{
    Bounds, BoxShadow, Context, CursorStyle, ElementId, Entity, EventEmitter, FontWeight,
    IntoElement, Pixels, Point, Render, Window, div, font, hsla, point, px,
};
use gpui_component::label::Label;
use gpui_component::{ActiveTheme, Sizable, v_flex};

use crate::app::ResultExt;
use crate::components::checkbox::Checkbox;
use crate::components::drag_drop::{DragData, Draggable, DropIndicator, DropPosition, DropZone};
use crate::stores::TaskStore;
use crate::stores::drag_drop_store::DragDropStore;
use crate::stores::task_store::{ApiError, TaskLocation, TasksUpdated};
use crate::tasks::TaskData;

struct Pipeline {
    task_store: Entity<TaskStore>,
    task_data: Vec<TaskData>,
    drag_drop_store: Entity<DragDropStore>,
    drag_active_here: bool,
    item_height: Pixels,
    gap: Pixels,
}

impl Pipeline {
    pub fn new(
        task_store: Entity<TaskStore>,
        drag_drop_store: Entity<DragDropStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(&task_store, |this, _store, _event: &TasksUpdated, cx| {
            this.update_tasks(cx);
            cx.notify();
        })
        .detach();

        Self {
            task_store,
            task_data: vec![],
            drag_drop_store,
            drag_active_here: false,
            item_height: px(80.0),
            gap: px(12.0),
        }
    }

    pub fn update_tasks(&mut self, cx: &mut Context<Self>) {
        self.task_data = self.task_store.read(cx).pipeline_data();
    }

    fn calculate_drop_index(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> usize {
        // let item_count = self.task_data.len().min(5);
        let item_count = self.task_data.len();
        if item_count == 0 {
            return 0;
        }

        let interval = self.item_height + self.gap;

        let relative_y = (position.y - bounds.origin.y).clamp(px(0.0), bounds.size.height);
        let item_index = (relative_y / interval).floor() as usize;
        // item_index.min(item_count)
        item_index
    }

    fn handle_drag_move(
        &mut self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if bounds.contains(&window.mouse_position()) {
            let drop_index = self.calculate_drop_index(position, bounds, window, cx);
            // let drop_index = self.calculate_drop_index();
            self.drag_drop_store.update(cx, |store, cx| {
                let location = Some(TaskLocation::Pipeline(drop_index));
                store.set_target(location, cx);
            });
            self.drag_active_here = true;
        } else if self.drag_active_here {
            // drag moved out of bounds, clear target
            self.drag_drop_store.update(cx, |store, cx| {
                // only clear if current target is within this component
                if let Some(TaskLocation::Pipeline(_)) = store.get_target() {
                    store.clear_target(cx);
                }
            });
            self.drag_active_here = false;
        }
    }
}

impl Render for Pipeline {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let target_index = self
            .drag_drop_store
            .read(cx)
            .get_target()
            .and_then(|loc| match loc {
                TaskLocation::Pipeline(ix) => Some(*ix),
                _ => None,
            });

        div().size_full().p_2().child(
            div().size_full().overflow_hidden().child(
                DropZone::<DragData<TaskData>>::new("pipeline-drop-zone")
                    .active(self.drag_active_here)
                    .size_full()
                    .min_h(px(200.0))
                    .insertion_indicator(target_index.map(|index| DropIndicator {
                        index,
                        position: DropPosition::Before,
                    }))
                    .on_drop(
                        cx.listener(move |this, data: &DragData<TaskData>, _window, cx| {
                            if let Some(index) = target_index {
                                this.drag_drop_store.update(cx, |store, cx| {
                                    store.clear_target(cx);
                                });
                                this.task_store.update(cx, |store, cx| {
                                    if let Some(id) = &data.data.task_id {
                                        store
                                            .update_location(id, TaskLocation::Pipeline(index), cx)
                                            .log_err();
                                    }
                                });
                                this.drag_active_here = false;
                            }
                        }),
                    )
                    .on_drag_move(cx.listener(
                        move |this, event: &gpui::DragMoveEvent<DragData<TaskData>>, window, cx| {
                            // Start the drag in our store if not already started
                            // if let Some(dragged_item) = event
                            //     .dragged_item()
                            //     .downcast_ref::<DragData<TaskDragData>>()
                            // {
                            //     this.drag_drop_store.update(cx, |store, cx| {
                            //         if !store.has_active_drag() {
                            //             store.start_drag_from_data(&dragged_item.data, cx);
                            //         }
                            //     });
                            // }
                            this.handle_drag_move(event.event.position, event.bounds, window, cx);
                        },
                    ))
                    .when(!self.task_data.is_empty(), |this| {
                        this.children(
                            self.task_data
                                .iter()
                                // .take(5)
                                .take(10)
                                .enumerate()
                                .map(|(i, task)| {
                                    let title = task.title.clone().unwrap_or("Untitled".into());
                                    // let opacity = 1.0 - (i as f32 * 0.2);
                                    let opacity = 1.0;

                                    let drag_title = title.clone();
                                    let click_title = title.clone();

                                    let theme_clone = theme.clone();
                                    // let task_drag_data = TaskDragData {
                                    //     task: task_clone.clone(),
                                    //     source_view: "pipeline".to_string(),
                                    //     source_index: i,
                                    // };
                                    let drag_data = DragData::new(task.clone())
                                        // .with_label(SharedString::from(title_clone.clone()))
                                        .with_preview(move || {
                                            div()
                                                .px(px(12.0))
                                                .py(px(8.0))
                                                .bg(theme_clone.popover.opacity(0.95))
                                                .border_1()
                                                .border_color(theme_clone.border)
                                                .rounded(px(6.0))
                                                .shadow(vec![BoxShadow {
                                                    color: hsla(0.0, 0.0, 0.0, 0.25),
                                                    offset: point(px(0.0), px(4.0)),
                                                    blur_radius: px(12.0),
                                                    spread_radius: px(0.0),
                                                }])
                                                .text_size(px(13.0))
                                                .text_color(theme_clone.foreground)
                                                .font_weight(FontWeight::MEDIUM)
                                                .child(format!("Moving: {}", drag_title))
                                                .into_any_element()
                                        });

                                    Draggable::new(("pipeline-item", i), drag_data)
                                        .cursor_style(CursorStyle::PointingHand)
                                        .hover_bg(theme.list_hover.opacity(0.3))
                                        .w_full()
                                        .h(self.item_height)
                                        .p_2()
                                        .bg(theme.background)
                                        .opacity(opacity)
                                        .rounded_md()
                                        .border_1()
                                        .border_color(theme.border)
                                        .child(
                                            Checkbox::new(ElementId::NamedInteger(
                                                "pipeline-checkbox".into(),
                                                i as u64,
                                            ))
                                            .checked(false)
                                            .large()
                                            .on_click(cx.listener(
                                                move |_view, _checked, _window, _cx| {
                                                    println!(
                                                        "Pipeline item clicked: {}",
                                                        click_title
                                                    );
                                                },
                                            )),
                                        )
                                        .child(Label::new(title).text_sm())
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .when(self.task_data.is_empty(), |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(8.0))
                                .py(px(32.0))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .text_color(theme.muted_foreground)
                                        .child("Drop tasks here to add to pipeline"),
                                ),
                        )
                    }),
            ),
        )
    }
}

pub struct RightSidebarView {
    collapsed: bool,
    pipeline: Entity<Pipeline>,
}

impl RightSidebarView {
    pub fn new(
        task_store: Entity<TaskStore>,
        drag_drop_store: Entity<DragDropStore>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(
            &task_store,
            |this, _task_store, _event: &TasksUpdated, cx| {
                this.update_pipeline(cx);
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(&task_store, |_this, _task_store, event: &ApiError, cx| {
            eprintln!("TaskListView: API Error: {}", event.message);
            cx.notify();
        })
        .detach();

        let pipeline_list = cx.new(|cx| Pipeline::new(task_store, drag_drop_store, cx));

        Self {
            collapsed: false,
            pipeline: pipeline_list,
        }
    }

    fn update_pipeline(&mut self, cx: &mut Context<Self>) {
        self.pipeline.update(cx, |pipeline, cx| {
            pipeline.update_tasks(cx);
            cx.notify();
        });
    }

    pub fn toggle_collapsed(&mut self, cx: &mut Context<Self>) -> bool {
        self.collapsed = !self.collapsed;
        cx.notify();
        self.collapsed
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }
}

impl EventEmitter<()> for RightSidebarView {}

impl Render for RightSidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_2()
            .pl_1()
            .bg(cx.theme().secondary)
            .child(
                div()
                    .size_full()
                    .bg(cx.theme().background)
                    .rounded_lg()
                    .child(
                        // right sidebar content
                        v_flex()
                            .size_full()
                            .pt_4()
                            .gap_3()
                            .items_center()
                            .child(Label::new("Pipeline").text_lg().font(font("Georgia")))
                            .child(self.pipeline.clone()),
                    ),
            )
    }
}
