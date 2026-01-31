use database::Action;
use gpui::{
    App, AppContext, Context, CursorStyle, DragMoveEvent, ElementId, Entity, EventEmitter,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, actions, div, px,
};
use gpui_component::{
    ActiveTheme, IconName, IndexPath, Selectable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    menu::ContextMenuExt,
    select::{Select, SelectState},
    v_flex,
};

use crate::{
    app::ResultExt,
    components::{
        checkbox::Checkbox,
        custom_list::{List, ListDelegate, ListEvent, ListItem, ListState},
        drag_drop::{DragData, Draggable, DropZone},
    },
    stores::{
        DatabaseStore,
        database_store::{ActionsLoaded, DatabaseError},
        drag_drop_store::ActionLocation,
    },
};

use crate::stores::drag_drop_store::DragDropStore;

use crate::views::main_view::MainViewMode;

actions!(action_list, [CopyAction, PasteAction, DeleteAction]);

#[derive(Clone, Debug)]
pub struct NavigateToView {
    pub mode: MainViewMode,
}

pub struct ActionListDelegate {
    actions: Vec<Action>,
    filtered: Vec<Action>,
    is_searching: bool,
    search_query: String,
    selected_index: Option<usize>,
    database_store: Entity<DatabaseStore>,
}

impl ActionListDelegate {
    pub fn new(database_store: Entity<DatabaseStore>, cx: &App) -> Self {
        let actions: Vec<Action> = database_store.read(cx).get_actions().clone();

        Self {
            actions,
            filtered: Vec::new(),
            is_searching: false,
            search_query: String::new(),
            selected_index: None,
            database_store,
        }
    }

    pub fn update_actions(&mut self, cx: &App) {
        self.actions = self.database_store.read(cx).get_actions().clone();

        if self.is_searching && !self.search_query.is_empty() {
            self.filtered = self
                .actions
                .iter()
                .filter(|action| {
                    let title = action.title.to_lowercase();
                    title.contains(&self.search_query.to_lowercase())
                })
                .cloned()
                .collect();
        } else {
            self.is_searching = false;
            self.filtered.clear();
        }
    }

    fn current_tasks(&self) -> &[Action] {
        if self.is_searching {
            &self.filtered
        } else {
            &self.actions
        }
    }
}

impl ListDelegate for ActionListDelegate {
    fn items_count(&self, _cx: &App) -> usize {
        self.current_tasks().len()
    }

    fn render_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<ListState<ActionListDelegate>>,
    ) -> Option<ListItem> {
        self.current_tasks().get(ix).map(|task| {
            let title = task.title.replace('\n', " ").replace('\r', " ");
            let id = task.id.clone();
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
                        )
                        .context_menu(|menu, window, cx| {
                            menu.menu_with_icon("Copy", IconName::Copy, Box::new(CopyAction))
                                .menu_with_icon("Paste", IconName::Copy, Box::new(PasteAction))
                                .separator()
                                .menu_with_icon("Delete", IconName::Delete, Box::new(DeleteAction))
                        }),
                )
                .selected(is_selected)
                .on_action(
                    cx.listener(move |list_state, _: &DeleteAction, _window, cx| {
                        list_state
                            .delegate_mut()
                            .database_store
                            .update(cx, |store, cx| {
                                store.delete_action(id.clone(), cx);
                            });
                        cx.notify();
                    }),
                )
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

pub struct ActionListView {
    drag_drop_store: Entity<DragDropStore>,
    drag_active_here: bool,
    // ui_store: Entity<UiStateStore>,
    list_state: Entity<ListState<ActionListDelegate>>,
    task_list_selection: Option<Entity<SelectState<Vec<&'static str>>>>,
}

impl ActionListView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        drag_drop_store: Entity<DragDropStore>,
        // ui_store: Entity<UiStateStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(
            &database_store,
            |this, _task_store, _event: &ActionsLoaded, cx| {
                this.update_task_list(cx);
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(
            &database_store,
            |_this, _task_store, event: &DatabaseError, cx| {
                eprintln!("ActionListView: Database error: {}", event.message);
                cx.notify();
            },
        )
        .detach();

        let delegate = ActionListDelegate::new(database_store, cx);
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
            list_state.delegate_mut().update_actions(cx);
            cx.notify();
        });
    }

    fn handle_task_click(&mut self, ix: usize, _cx: &mut Context<Self>) {
        if let Some(task) = self.list_state.read(_cx).delegate().current_tasks().get(ix) {
            println!("Action clicked: {}", &task.title);
        }
    }

    fn handle_drag_move(
        &mut self,
        event: &DragMoveEvent<DragData<Action>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = event.bounds;
        let data = event.drag(cx).clone();
        let item = data.data;
        let id = item.id;
        if self.drag_drop_store.read(cx).is_dragging() == false {
            self.drag_drop_store.update(cx, |store, cx| {
                store.new_drag(id.clone(), cx);
            });
        }

        if bounds.contains(&window.mouse_position()) {
            // let item_size = self.measure_item(window, cx);
            // let drop_index = self.calculate_drop_index(item_size, position, bounds, window, cx);
            // let drop_index = self.calculate_drop_index();
            self.drag_drop_store.update(cx, |store, cx| {
                store.set_drop_target(Some(ActionLocation::ActionList), cx);
            });
            self.drag_active_here = true;
        } else if self.drag_active_here {
            // drag moved out of bounds, clear target
            self.drag_drop_store.update(cx, |store, cx| {
                // only clear if current target is within this component
                if let Some(ActionLocation::ActionList) = store.get_drop_target() {
                    store.clear_drop_target(cx);
                }
            });
            // self.database_store
            //     .update(cx, |store, cx| store.update_location(&id, None, cx));
            self.drag_active_here = false;
        }
    }
}

impl EventEmitter<NavigateToView> for ActionListView {}

impl Render for ActionListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // self.ensure_list_state(window, cx);

        let selection_state = self.task_list_selection.as_ref().unwrap();
        let current_selection_ix = selection_state
            .read(cx)
            .selected_index(cx)
            .unwrap_or_default();
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
                        DropZone::<DragData<Action>>::new("task-list-zone")
                            .min_h(px(300.0))
                            .size_full()
                            .active(self.drag_active_here)
                            // .insertion_indicator(target_index.map(|index| DropIndicator {
                            //     index,
                            //     position: DropPosition::Before,
                            // }))
                            .on_drop(cx.listener(
                                move |this, data: &DragData<Action>, _window, cx| {
                                    if this.drag_active_here {
                                        this.drag_drop_store.update(cx, |store, cx| {
                                            store.clear_drag(cx);
                                        });
                                        // this.task_store.update(cx, |store, cx| {
                                        //     if let Some(id) = &data.data.task_id {
                                        //         store
                                        //             .update_location(
                                        //                 id,
                                        //                 Some(TaskLocation::TaskList),
                                        //                 cx,
                                        //             )
                                        //             .log_err();
                                        //     }
                                        // });
                                        this.drag_active_here = false;
                                    }
                                    cx.notify();
                                },
                            ))
                            .on_drag_move(cx.listener(
                                move |this, event: &DragMoveEvent<DragData<Action>>, window, cx| {
                                    this.handle_drag_move(event, window, cx);
                                },
                            ))
                            .child(List::new(&self.list_state))
                    }),
            )
    }
}
