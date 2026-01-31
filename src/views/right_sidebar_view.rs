use gpui::{
    Bounds, BoxShadow, Context, CursorStyle, ElementId, Entity, EventEmitter, FontWeight,
    IntoElement, Pixels, Point, Render, Window, div, font, hsla, point, px,
};
use gpui::{DragMoveEvent, prelude::*};
use gpui_component::label::Label;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{ActiveTheme, Sizable, StyledExt, v_flex};
use std::collections::HashSet;

use crate::app::ResultExt;
use crate::components::checkbox::Checkbox;
use crate::components::drag_drop::{DragData, Draggable, DropIndicator, DropPosition, DropZone};
use crate::stores::DatabaseStore;
use crate::stores::database_store::{ActionsLoaded, PipelineLoaded};
use crate::stores::drag_drop_store::{ActionLocation, DragDropStore};
use database::{Action, Instance, PipelineItem};
// use crate::stores::task_store::{ApiError, TaskLocation, TasksUpdated};
// use crate::tasks::TaskData;

struct Pipeline {
    // task_store: Entity<TaskStore>,
    database_store: Entity<DatabaseStore>,
    // task_data: Vec<TaskData>,
    items: Vec<(PipelineItem, Instance)>,
    drag_drop_store: Entity<DragDropStore>,
    drag_active_here: bool,
    item_height: Pixels,
    gap: Pixels,
    pending_drops: Vec<(String, i64)>,
    in_progress_deletes: HashSet<String>,
    processing_drop: bool,
}

impl Pipeline {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        drag_drop_store: Entity<DragDropStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        // cx.subscribe(&task_store, |this, _store, _event: &TasksUpdated, cx| {
        //     this.update_tasks(cx);
        //     cx.notify();
        // })
        // .detach();

        cx.subscribe(
            &database_store,
            |this, _store, _event: &ActionsLoaded, cx| {
                println!("[SUBSCRIPTION] ActionsLoaded event received");
                this.update_items(cx);
                cx.notify();
                println!("[SUBSCRIPTION] ActionsLoaded handler completed");
            },
        )
        .detach();

        cx.subscribe(
            &database_store,
            |this, _store, _event: &PipelineLoaded, cx| {
                println!(
                    "[SUBSCRIPTION] PipelineLoaded event received, processing_drop={}",
                    this.processing_drop
                );
                this.update_items(cx);
                if this.processing_drop {
                    println!("Pipeline reload completed for drop, processing next");
                    this.processing_drop = false;
                    if !this.pending_drops.is_empty() {
                        println!(
                            "Processing queued drop, {} remaining",
                            this.pending_drops.len()
                        );
                        this.process_next_drop(cx);
                    }
                }
                cx.notify();
                println!("[SUBSCRIPTION] PipelineLoaded handler completed");
            },
        )
        .detach();

        Self {
            // task_store,
            database_store,
            // task_data: vec![],
            items: vec![],
            drag_drop_store,
            drag_active_here: false,
            item_height: px(80.0),
            gap: px(12.0),
            pending_drops: vec![],
            in_progress_deletes: HashSet::new(),
            processing_drop: false,
        }
    }

    // pub fn update_tasks(&mut self, cx: &mut Context<Self>) {
    //     self.task_data = self.task_store.read(cx).pipeline_data();
    // }

    pub fn update_items(&mut self, cx: &mut Context<Self>) {
        println!("[UPDATE_ITEMS] Starting update_items");
        // self.items = self.database_store.read(cx).get_pipeline_items().clone();
        let items = self.database_store.read(cx).get_pipeline_items().clone();

        self.items = items
            .into_iter()
            .filter_map(|item| {
                item.instance_id
                    .clone()
                    .map(|id| {
                        self.database_store
                            .read(cx)
                            .get_instance(&id)
                            .map(|instance| (item, instance.clone()))
                    })
                    .flatten()
            })
            .collect();
        self.in_progress_deletes
            .retain(|id| self.items.iter().any(|(_, instance)| &instance.id == id));
        println!("[UPDATE_ITEMS] Completed, {} items", self.items.len());
    }

    fn process_next_drop(&mut self, cx: &mut Context<Self>) {
        if self.processing_drop || self.pending_drops.is_empty() {
            return;
        }

        let (id, position) = self.pending_drops.remove(0);
        self.processing_drop = true;

        println!(
            "Processing queued drop: action {} at position {}",
            id, position
        );

        self.drag_drop_store.update(cx, |store, cx| {
            store.clear_drag(cx);
        });

        println!("Inserting action {} at position {}", id, position);

        self.database_store.update(cx, |store, cx| {
            store.insert_instance_at_position(id, position, cx);
        });

        self.drag_active_here = false;
    }

    fn calculate_drop_index(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> usize {
        // let item_count = self.task_data.len().min(5);
        // let item_count = self.task_data.len();
        let item_count = self.items.len();
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
        event: &DragMoveEvent<DragData<Action>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = event.bounds;
        let position = event.event.position;
        let data = event.drag(cx).clone();
        let item = data.data;
        let action_id = item.id.clone();
        if self.drag_drop_store.read(cx).is_dragging() == false {
            self.drag_drop_store.update(cx, |store, cx| {
                store.new_drag(action_id.clone(), cx);
            });
        }
        if bounds.contains(&window.mouse_position()) {
            let drop_index = self.calculate_drop_index(position, bounds, window, cx);
            self.drag_drop_store.update(cx, |store, cx| {
                let location = Some(ActionLocation::Pipeline(drop_index));
                store.set_drop_target(location, cx);
            });
            self.drag_active_here = true;
        } else if self.drag_active_here {
            // drag moved out of bounds, clear target
            self.drag_drop_store.update(cx, |store, cx| {
                // only clear if current target is within this component
                if let Some(ActionLocation::Pipeline(_)) = store.get_drop_target() {
                    store.clear_drop_target(cx);
                }
            });
            // if let Some(id) = task_id {
            //     self.task_store
            //         .update(cx, |store, cx| store.update_location(&id, None, cx));
            // }
            self.drag_active_here = false;
        }
    }
}

impl Render for Pipeline {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let dragged_task_id = self
            .drag_drop_store
            .read(cx)
            .get_active_drag_item()
            .and_then(|action_id| Some(action_id.clone()));
        let target_index =
            self.drag_drop_store
                .read(cx)
                .get_drop_target()
                .and_then(|loc| match loc {
                    // TaskLocation::Pipeline(ix) => Some(*ix),
                    ActionLocation::Pipeline(ix) => Some(*ix),
                    _ => None,
                });

        // div().size_full().p_2().child(
        div().size_full().overflow_y_scrollbar().child(
            DropZone::<DragData<Action>>::new("pipeline-drop-zone")
                .active(self.drag_active_here)
                .size_full()
                .insertion_indicator(target_index.map(|index| DropIndicator {
                    index,
                    position: DropPosition::Before,
                }))
                .on_drop(
                    cx.listener(move |this, data: &DragData<Action>, _window, cx| {
                        if let Some(index) = target_index {
                            let id = data.data.id.clone();
                            let position = (index as i64) + 1;

                            let is_duplicate = this
                                .pending_drops
                                .iter()
                                .any(|(pid, ppos)| pid == &id && *ppos == position);

                            if is_duplicate {
                                println!(
                                    "Ignoring duplicate drop for action {} at position {}",
                                    id, position
                                );
                                return;
                            }

                            if this.processing_drop {
                                println!(
                                    "Drop in progress, queueing: action {} at position {}",
                                    id, position
                                );
                                this.pending_drops.push((id, position));
                            } else {
                                println!("Starting drop: action {} at position {}", id, position);
                                this.pending_drops.push((id.clone(), position));
                                this.process_next_drop(cx);
                            }
                        }
                    }),
                )
                .on_drag_move(cx.listener(
                    move |this, event: &gpui::DragMoveEvent<DragData<Action>>, window, cx| {
                        this.handle_drag_move(event, window, cx);
                    },
                ))
                .when(!self.items.is_empty(), |this| {
                    this.children(
                        self.items
                            .iter()
                            // .take(10)
                            .enumerate()
                            // .filter(|(_i, item)| {
                            //     if let Some(dragged_id) = &dragged_task_id {
                            //         item.instance_id.as_ref() != Some(dragged_id)
                            //     } else {
                            //         true
                            //     }
                            // })
                            .map(|(i, (item, instance))| {
                                let title =
                                    item.action_title.clone().unwrap_or("Untitled".to_string());
                                let instance_id = instance.id.clone();
                                let completed = &instance.status == "completed";
                                // let opacity = 1.0 - (i as f32 * 0.2);
                                let opacity = 1.0;

                                let drag_title = title.clone();

                                let theme_clone = theme.clone();
                                // let task_drag_data = TaskDragData {
                                //     task: task_clone.clone(),
                                //     source_view: "pipeline".to_string(),
                                //     source_index: i,
                                // };
                                let drag_data = DragData::new(item.clone())
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
                                    .h_flex()
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
                                    .gap_3()
                                    .child(
                                        Checkbox::new(ElementId::Name(
                                            format!("pipeline-checkbox-{}", instance_id).into(),
                                        ))
                                        .checked(completed)
                                        .large()
                                        .on_click(
                                            cx.listener(
                                                move |this, checked: &bool, _window, cx| {
                                                    println!("[CHECKBOX] Click handler entered, checked={}, instance={}", checked, instance_id);
                                                    if *checked {
                                                        if this.in_progress_deletes.contains(&instance_id) {
                                                            println!(
                                                                "[CHECKBOX] delete_instance already in progress for {}",
                                                                instance_id
                                                            );
                                                            return;
                                                        }

                                                        this
                                                            .in_progress_deletes
                                                            .insert(instance_id.clone());

                                                        println!("[CHECKBOX] Calling delete_instance for {}", instance_id);
                                                        this.database_store.update(
                                                            cx,
                                                            |store, cx| {
                                                                store.delete_instance(
                                                                    instance_id.clone(),
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                        println!("[CHECKBOX] delete_instance call completed for {}", instance_id);
                                                    } else {
                                                        println!("[CHECKBOX] Calling uncomplete_pipeline_item for {}", instance_id);
                                                        this.database_store.update(
                                                            cx,
                                                            |store, cx| {
                                                                store.uncomplete_pipeline_item(
                                                                    instance_id.clone(),
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                        println!("[CHECKBOX] uncomplete_pipeline_item call completed for {}", instance_id);
                                                    }
                                                    println!("[CHECKBOX] Click handler exiting");
                                                },
                                            ),
                                        ),
                                    )
                                    .child(Label::new(title).text_sm())
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .when(self.items.is_empty(), |this| {
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
                                    .child("Drop actions here to add to pipeline"),
                            ),
                    )
                }),
        )
        // )
    }
}

pub struct RightSidebarView {
    collapsed: bool,
    pipeline: Entity<Pipeline>,
}

impl RightSidebarView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        drag_drop_store: Entity<DragDropStore>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // cx.subscribe(
        //     &task_store,
        //     |this, _task_store, _event: &TasksUpdated, cx| {
        //         this.update_pipeline(cx);
        //         cx.notify();
        //     },
        // )
        // .detach();

        cx.subscribe(
            &database_store,
            |this, _task_store, _event: &ActionsLoaded, cx| {
                this.update_pipeline(cx);
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(
            &database_store,
            |this, _task_store, _event: &PipelineLoaded, cx| {
                this.update_pipeline(cx);
                cx.notify();
            },
        )
        .detach();

        let pipeline_list = cx.new(|cx| Pipeline::new(database_store, drag_drop_store, cx));

        Self {
            collapsed: false,
            pipeline: pipeline_list,
        }
    }

    fn update_pipeline(&mut self, cx: &mut Context<Self>) {
        self.pipeline.update(cx, |pipeline, cx| {
            pipeline.update_items(cx);
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
                    .overflow_hidden()
                    .bg(cx.theme().background)
                    .rounded_lg()
                    .child(
                        // right sidebar content
                        v_flex()
                            .size_full()
                            .pt_4()
                            .gap_3()
                            // .rounded_lg()
                            .items_center()
                            .child(Label::new("Pipeline").text_lg().font(font("Georgia")))
                            .child(self.pipeline.clone()),
                    ),
            )
    }
}
