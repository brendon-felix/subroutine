use app_core::SavedAction;
use gpui::{
    App, AppContext, Context, CursorStyle, DragMoveEvent, ElementId, Entity, EventEmitter,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, actions, div, px,
};
use gpui_component::{
    ActiveTheme, IconName, IndexPath, Selectable, Sizable,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    menu::ContextMenuExt,
    select::{Select, SelectState},
    v_flex,
};

use crate::{
    components::{
        custom_list::{List, ListDelegate, ListEvent, ListItem, ListState},
        drag_drop::{DragData, Draggable, DropZone},
    },
    stores::{
        DatabaseStore,
        database_store::{DatabaseError, SavedActionsLoaded},
        drag_drop_store::ActionLocation,
    },
    views::StartActionEditor,
};

use crate::stores::drag_drop_store::DragDropStore;

use crate::views::main_view::MainViewMode;

actions!(action_list, [CopyAction, PasteAction, DeleteAction]);

#[derive(Clone, Debug)]
pub struct NavigateToView {
    pub mode: MainViewMode,
}

pub struct ActionListDelegate {
    actions: Vec<SavedAction>,
    filtered: Vec<SavedAction>,
    is_searching: bool,
    search_query: String,
    selected_index: Option<usize>,
    database_store: Entity<DatabaseStore>,
}

impl ActionListDelegate {
    pub fn new(database_store: Entity<DatabaseStore>, cx: &App) -> Self {
        let actions = database_store.read(cx).get_saved_actions().clone();

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
        self.actions = self.database_store.read(cx).get_saved_actions().clone();

        if self.is_searching && !self.search_query.is_empty() {
            let query = self.search_query.to_lowercase();
            self.filtered = self
                .actions
                .iter()
                .filter(|action| action.title.to_lowercase().contains(&query))
                .cloned()
                .collect();
        } else {
            self.is_searching = false;
            self.filtered.clear();
        }
    }

    fn current_actions(&self) -> &[SavedAction] {
        if self.is_searching {
            &self.filtered
        } else {
            &self.actions
        }
    }
}

impl ListDelegate for ActionListDelegate {
    fn items_count(&self, _cx: &App) -> usize {
        self.current_actions().len()
    }

    fn render_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<ListState<ActionListDelegate>>,
    ) -> Option<ListItem> {
        self.current_actions().get(ix).map(|action| {
            let title = action.title.replace('\n', " ").replace('\r', " ");
            let id = action.id;
            let is_selected = Some(ix) == self.selected_index;

            let mouse_position = window.mouse_position();
            let drag_data = DragData::new(action.clone())
                .with_label(SharedString::from(title.clone()))
                .with_position(mouse_position);

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
                                        .child(Label::new(title).truncate()),
                                )
                                .child(div().w_2()),
                        )
                        .context_menu(|menu, _window, _cx| {
                            menu.menu_with_icon("Copy", IconName::Copy, Box::new(CopyAction))
                                .separator()
                                .menu_with_icon("Delete", IconName::Delete, Box::new(DeleteAction))
                        }),
                )
                .selected(is_selected)
                .on_action({
                    cx.listener(move |list_state, _: &DeleteAction, _window, cx| {
                        list_state
                            .delegate_mut()
                            .database_store
                            .update(cx, |store, cx| {
                                store.delete_saved_action(id, cx);
                            });
                        cx.notify();
                    })
                })
                .on_click({
                    cx.listener(move |list_state, _event, _window, cx| {
                        if Some(ix) == list_state.delegate().selected_index {
                            cx.emit(StartActionEditor {
                                action_id: Some(id),
                            });
                        } else {
                            list_state.delegate_mut().selected_index = Some(ix);
                            cx.notify();
                        }
                    })
                })
        })
    }

    fn set_selected_index(&mut self, ix: Option<usize>) {
        self.selected_index = ix;
    }
}

pub struct ActionListView {
    drag_drop_store: Entity<DragDropStore>,
    drag_active_here: bool,
    pub list_state: Entity<ListState<ActionListDelegate>>,
    task_list_selection: Option<Entity<SelectState<Vec<&'static str>>>>,
}

impl ActionListView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        drag_drop_store: Entity<DragDropStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(
            &database_store,
            |this, _store, _event: &SavedActionsLoaded, cx| {
                this.update_action_list(cx);
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(
            &database_store,
            |_this, _store, event: &DatabaseError, _cx| {
                eprintln!("ActionListView: Database error: {}", event.message);
            },
        )
        .detach();

        let delegate = ActionListDelegate::new(database_store, cx);
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        cx.subscribe(&list_state, |_this, _list_state, event: &ListEvent, cx| {
            if let ListEvent::Confirm(_ix) = event {
                cx.notify();
            }
        })
        .detach();

        let task_list_selection = cx
            .new(|cx| {
                SelectState::new(
                    vec!["All Actions", "High Priority", "Pending"],
                    Some(IndexPath::new(0)),
                    window,
                    cx,
                )
            })
            .into();

        Self {
            drag_drop_store,
            drag_active_here: false,
            list_state,
            task_list_selection,
        }
    }

    fn update_action_list(&mut self, cx: &mut Context<Self>) {
        self.list_state.update(cx, |list_state, cx| {
            list_state.delegate_mut().update_actions(cx);
            cx.notify();
        });
    }

    fn handle_drag_move(
        &mut self,
        event: &DragMoveEvent<DragData<SavedAction>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = event.bounds;
        let data = event.drag(cx).clone();
        let id = data.data.id.to_string();
        if !self.drag_drop_store.read(cx).is_dragging() {
            self.drag_drop_store.update(cx, |store, cx| {
                store.new_drag(id.clone(), cx);
            });
        }

        if bounds.contains(&window.mouse_position()) {
            self.drag_drop_store.update(cx, |store, cx| {
                store.set_drop_target(Some(ActionLocation::ActionList), cx);
            });
            self.drag_active_here = true;
        } else if self.drag_active_here {
            self.drag_drop_store.update(cx, |store, cx| {
                if let Some(ActionLocation::ActionList) = store.get_drop_target() {
                    store.clear_drop_target(cx);
                }
            });
            self.drag_active_here = false;
        }
    }
}

impl EventEmitter<StartActionEditor> for ListState<ActionListDelegate> {}

impl EventEmitter<NavigateToView> for ActionListView {}

impl Render for ActionListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selection_state = self.task_list_selection.as_ref().unwrap();

        v_flex()
            .id("task-list")
            .size_full()
            .p_4()
            .gap_4()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("tasks-to-home-btn")
                                    .icon(IconName::ArrowLeft)
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|_this, _event, _window, cx| {
                                        cx.emit(NavigateToView {
                                            mode: MainViewMode::Home,
                                        });
                                    })),
                            )
                            .child(Label::new("Actions").text_2xl().font(gpui::font("Georgia"))),
                    )
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
            .child(Divider::horizontal().color(cx.theme().border).w_full())
            .child(
                div()
                    .flex_1()
                    .id("actions-scroll")
                    .overflow_y_scroll()
                    .child({
                        DropZone::<DragData<SavedAction>>::new("task-list-zone")
                            .min_h(px(300.0))
                            .size_full()
                            .active(self.drag_active_here)
                            .on_drop(cx.listener(
                                move |this, _data: &DragData<SavedAction>, _window, cx| {
                                    if this.drag_active_here {
                                        this.drag_drop_store.update(cx, |store, cx| {
                                            store.clear_drag(cx);
                                        });
                                        this.drag_active_here = false;
                                    }
                                    cx.notify();
                                },
                            ))
                            .on_drag_move(cx.listener(
                                move |this,
                                      event: &DragMoveEvent<DragData<SavedAction>>,
                                      window,
                                      cx| {
                                    this.handle_drag_move(event, window, cx);
                                },
                            ))
                            .child(List::new(&self.list_state))
                    }),
            )
    }
}
