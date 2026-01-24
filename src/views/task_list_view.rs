use gpui::{
    App, AppContext, Context, CursorStyle, ElementId, Entity, EventEmitter, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px,
};
use gpui_component::{
    ActiveTheme, IconName, IndexPath, Selectable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    select::{Select, SelectState},
    v_flex,
};

use crate::{
    app::ResultExt,
    components::{
        checkbox::Checkbox,
        custom_list::{List, ListDelegate, ListEvent, ListItem, ListState},
        drag_drop::{DragData, Draggable, DropZone, DropZoneStyle},
    },
};

use crate::stores::{
    TaskStore,
    drag_drop_store::DragDropStore,
    task_store::{ApiError, TaskLocation, TasksUpdated},
};

use crate::tasks::{TaskData, task_data_compare};
use crate::views::main_view::MainViewMode;

#[derive(Clone, Debug)]
pub struct NavigateToView {
    pub mode: MainViewMode,
}

pub struct TaskListDelegate {
    tasks_data: Vec<TaskData>,
    filtered_tasks: Vec<TaskData>,
    is_searching: bool,
    search_query: String,
    selected_index: Option<usize>,
    task_store: Entity<TaskStore>,
}

impl TaskListDelegate {
    pub fn new(task_store: Entity<TaskStore>, cx: &App) -> Self {
        let mut tasks_data: Vec<TaskData> = task_store.read(cx).task_list_data();
        tasks_data.sort_by(task_data_compare);

        Self {
            tasks_data,
            filtered_tasks: Vec::new(),
            is_searching: false,
            search_query: String::new(),
            selected_index: None,
            task_store,
        }
    }

    pub fn update_tasks(&mut self, cx: &App) {
        self.tasks_data = self.task_store.read(cx).task_list_data();

        if self.is_searching && !self.search_query.is_empty() {
            self.filtered_tasks = self
                .tasks_data
                .iter()
                .filter(|task| {
                    let title = task
                        .title
                        .as_ref()
                        .map(|t| t.to_lowercase())
                        .unwrap_or_default();
                    title.contains(&self.search_query.to_lowercase())
                })
                .cloned()
                .collect();
        } else {
            self.is_searching = false;
            self.filtered_tasks.clear();
        }
    }

    fn current_tasks(&self) -> &[TaskData] {
        if self.is_searching {
            &self.filtered_tasks
        } else {
            &self.tasks_data
        }
    }
}

impl ListDelegate for TaskListDelegate {
    fn items_count(&self, _cx: &App) -> usize {
        self.current_tasks().len()
    }

    fn render_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<ListState<TaskListDelegate>>,
    ) -> Option<ListItem> {
        self.current_tasks().get(ix).map(|task| {
            let default_title = "Untitled Task".to_string();
            let title = task
                .title
                .as_ref()
                .unwrap_or(&default_title)
                .replace('\n', " ")
                .replace('\r', " ");
            let is_selected = Some(ix) == self.selected_index;

            let mouse_position = window.mouse_position();
            let drag_data = DragData::new(task.clone())
                .with_label(SharedString::from(title.clone()))
                .with_position(mouse_position);
            // .with_preview(move || {
            //     div()
            //         .px(px(12.0))
            //         .py(px(8.0))
            //         .bg(theme.popover.opacity(0.95))
            //         .border_1()
            //         .border_color(theme.border)
            //         .rounded(px(6.0))
            //         .shadow(vec![BoxShadow {
            //             color: hsla(0.0, 0.0, 0.0, 0.25),
            //             offset: point(px(0.0), px(4.0)),
            //             blur_radius: px(12.0),
            //             spread_radius: px(0.0),
            //         }])
            //         .text_size(px(13.0))
            //         .text_color(theme.foreground)
            //         .font_weight(FontWeight::MEDIUM)
            //         .child(format!("Moving: {}", drag_title))
            //         .into_any_element()
            // });

            ListItem::new(ix)
                .rounded_md()
                .child(
                    Draggable::new(ix, drag_data)
                        .cursor_style(CursorStyle::PointingHand)
                        .w_full()
                        .child(
                            h_flex()
                                .items_center()
                                .py_2()
                                .child(
                                    h_flex()
                                        .items_center()
                                        .gap_3()
                                        .min_w_0()
                                        .flex_1()
                                        .child(
                                            Checkbox::new(ElementId::NamedInteger(
                                                "checkbox".into(),
                                                ix as u64,
                                            ))
                                            .on_click(|checked, _window, cx| {
                                                cx.stop_propagation();
                                                println!("Checkbox clicked: {}", checked);
                                            }),
                                        )
                                        .child(Label::new(title).truncate()),
                                )
                                .child(div().w_2()),
                        ),
                )
                .selected(is_selected)
                .on_click({
                    cx.listener(move |list_state, _event, _window, cx| {
                        list_state.delegate_mut().selected_index = Some(ix);
                        cx.notify();
                    })
                })
        })
    }

    fn set_selected_index(
        &mut self,
        ix: Option<usize>,
        // _window: &mut Window,
        // cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
    }
}

pub struct TaskListView {
    task_store: Entity<TaskStore>,
    drag_drop_store: Entity<DragDropStore>,
    drag_active_here: bool,
    // ui_store: Entity<UiStateStore>,
    list_state: Entity<ListState<TaskListDelegate>>,
    task_list_selection: Option<Entity<SelectState<Vec<&'static str>>>>,
}

impl TaskListView {
    pub fn new(
        task_store: Entity<TaskStore>,
        drag_drop_store: Entity<DragDropStore>,
        // ui_store: Entity<UiStateStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(
            &task_store,
            |this, _task_store, _event: &TasksUpdated, cx| {
                this.update_task_list(cx);
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(&task_store, |_this, _task_store, event: &ApiError, cx| {
            eprintln!("TaskListView: API Error: {}", event.message);
            cx.notify();
        })
        .detach();

        let delegate = TaskListDelegate::new(task_store.clone(), cx);
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        cx.subscribe(&list_state, |this, _list_state, event: &ListEvent, cx| {
            match event {
                ListEvent::Confirm(ix) => {
                    this.handle_task_click(*ix, cx);
                }
                _ => {}
            }
            cx.notify();
        })
        .detach();

        Self {
            task_store,
            drag_drop_store,
            drag_active_here: false,
            // ui_store,
            list_state,
            task_list_selection: cx
                .new(|cx| {
                    SelectState::new(
                        vec![
                            "All Tasks",
                            "Completed",
                            "Pending",
                            "High Priority",
                            "Due Today",
                        ],
                        Some(IndexPath::new(0)),
                        window,
                        cx,
                    )
                    // .selected_index(cx)
                })
                .into(),
        }
    }

    fn update_task_list(&mut self, cx: &mut Context<Self>) {
        self.list_state.update(cx, |list_state, cx| {
            list_state.delegate_mut().update_tasks(cx);
            cx.notify();
        });
    }

    fn handle_task_click(&mut self, ix: usize, _cx: &mut Context<Self>) {
        if let Some(task) = self.list_state.read(_cx).delegate().current_tasks().get(ix) {
            println!(
                "Task clicked: {}",
                task.title.clone().unwrap_or(ix.to_string())
            );
        }
    }

    fn handle_drag_move(
        &mut self,
        bounds: gpui::Bounds<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if bounds.contains(&window.mouse_position()) {
            // let item_size = self.measure_item(window, cx);
            // let drop_index = self.calculate_drop_index(item_size, position, bounds, window, cx);
            // let drop_index = self.calculate_drop_index();
            self.drag_drop_store.update(cx, |store, cx| {
                store.set_target(Some(TaskLocation::TaskList), cx);
            });
            self.drag_active_here = true;
        } else if self.drag_active_here {
            // drag moved out of bounds, clear target
            self.drag_drop_store.update(cx, |store, cx| {
                // only clear if current target is within this component
                if let Some(TaskLocation::TaskList) = store.get_target() {
                    store.clear_target(cx);
                }
            });
            self.drag_active_here = false;
        }
    }
}

impl EventEmitter<NavigateToView> for TaskListView {}

impl Render for TaskListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // self.ensure_list_state(window, cx);

        let selection_state = self.task_list_selection.as_ref().unwrap();
        // let is_drop_target: bool = self
        //     .drag_drop_store
        //     .read(cx)
        //     .get_target()
        //     .map(|loc| matches!(loc, TaskLocation::TaskList))
        //     .unwrap_or(false);

        div()
            .id("task-list")
            .size_full()
            .flex()
            .gap_3()
            // .p_0p5()
            .px_8()
            .child(
                div().flex().absolute().top_2().right_2().child(
                    Button::new("tasks-to-home-btn")
                        .w(px(112.0))
                        .icon(IconName::Map)
                        .ghost()
                        .label("Home")
                        .on_click(cx.listener(|_this, _event, _window, cx| {
                            cx.emit(NavigateToView {
                                mode: MainViewMode::Home,
                            });
                        })),
                ),
            )
            .child(
                v_flex()
                    .size_full()
                    .child(
                        div()
                            .w_full()
                            .h(px(50.0))
                            .border_b_1()
                            .border_color(cx.theme().border)
                            // .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div().child(
                                    Select::new(selection_state)
                                        .appearance(false)
                                        .text_center()
                                        .rounded_md()
                                        .w_32()
                                        .flex(),
                                ),
                            ),
                    )
                    .child({
                        DropZone::<DragData<TaskData>>::new("task-list-zone")
                            .drop_zone_style(DropZoneStyle::Dashed)
                            .min_h(px(300.0))
                            .size_full()
                            .active(self.drag_active_here)
                            // .insertion_indicator(target_index.map(|index| DropIndicator {
                            //     index,
                            //     position: DropPosition::Before,
                            // }))
                            .on_drop(cx.listener(
                                move |this, data: &DragData<TaskData>, _window, cx| {
                                    // Start the drag in our store if not already started
                                    // this.drag_drop_store.update(cx, |store, cx| {
                                    //     if !store.has_active_drag() {
                                    //         store.start_drag_from_data(&data.data, cx);
                                    //     }
                                    // });
                                    if this.drag_active_here {
                                        this.drag_drop_store.update(cx, |store, cx| {
                                            store.clear_target(cx);
                                        });
                                        this.task_store.update(cx, |store, cx| {
                                            if let Some(id) = &data.data.task_id {
                                                store
                                                    .update_location(id, TaskLocation::TaskList, cx)
                                                    .log_err();
                                            }
                                        });
                                        this.drag_active_here = false;
                                    }
                                    cx.notify();
                                },
                            ))
                            .on_drag_move(cx.listener(
                                move |this,
                                      event: &gpui::DragMoveEvent<DragData<TaskData>>,
                                      window,
                                      cx| {
                                    this.handle_drag_move(event.bounds, window, cx);
                                },
                            ))
                            .child(List::new(&self.list_state))
                    }),
            )
    }
}
