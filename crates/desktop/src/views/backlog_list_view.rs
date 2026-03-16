use gpui::prelude::*;
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, IntoElement, Render, SharedString,
    Subscription, Window, div, px,
};
use gpui_component::{
    ActiveTheme, IconName, Selectable, Sizable,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    menu::{ContextMenuExt, PopupMenuItem},
    v_flex,
};
use simple_core::Action;

use crate::{
    components::{
        custom_list::{List, ListDelegate, ListEvent, ListItem, ListState},
        drag_drop::{DragData, Draggable},
    },
    stores::{
        DatabaseStore,
        database_store::{DatabaseError, PipelineChanged},
    },
    views::{action_editor::StartActionEditor, main_view::MainViewMode},
};

#[derive(Clone, Debug)]
pub struct NavigateToView {
    pub mode: MainViewMode,
}

pub struct BacklogListDelegate {
    actions: Vec<Action>,
    selected_index: Option<usize>,
    database_store: Entity<DatabaseStore>,
}

impl BacklogListDelegate {
    pub fn new(database_store: Entity<DatabaseStore>, cx: &App) -> Self {
        let actions = database_store.read(cx).pipeline.backlog.clone();
        Self {
            actions,
            selected_index: None,
            database_store,
        }
    }

    pub fn update_actions(&mut self, cx: &App) {
        self.actions = self.database_store.read(cx).pipeline.backlog.clone();
    }
}

impl ListDelegate for BacklogListDelegate {
    fn items_count(&self, _cx: &App) -> usize {
        self.actions.len()
    }

    fn render_item(
        &mut self,
        ix: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let action = self.actions.get(ix)?.clone();
        let id = action.id;
        let title = SharedString::from(action.title.clone());
        let is_selected = Some(ix) == self.selected_index;
        let theme = cx.theme().clone();

        let drag_data = DragData::new(action.clone()).with_label(title.clone());

        Some(
            ListItem::new(ix)
                .rounded_md()
                .child(
                    Draggable::new(("backlog-drag", ix as u64), drag_data)
                        .w_full()
                        .context_menu({
                            let list_entity = cx.entity();
                            move |menu, _window, _cx| {
                                let entity_promote = list_entity.clone();
                                let entity_edit = list_entity.clone();
                                let entity_remove = list_entity.clone();

                                menu.item(
                                    PopupMenuItem::new("Promote to queue")
                                        .icon(IconName::ArrowUp)
                                        .on_click(move |_event, _window, cx| {
                                            entity_promote.update(cx, |list_state, cx| {
                                                list_state.delegate().database_store.update(
                                                    cx,
                                                    |store, cx| {
                                                        store.promote_action(id, cx);
                                                    },
                                                );
                                            });
                                        }),
                                )
                                .separator()
                                .item(
                                    PopupMenuItem::new("Edit")
                                        .icon(IconName::Settings2)
                                        .on_click(move |_event, _window, cx| {
                                            entity_edit.update(cx, |_list_state, cx| {
                                                cx.emit(StartActionEditor {
                                                    action_id: Some(id),
                                                });
                                            });
                                        }),
                                )
                                .separator()
                                .item(
                                    PopupMenuItem::new("Remove from backlog")
                                        .icon(IconName::Minus)
                                        .on_click(move |_event, _window, cx| {
                                            entity_remove.update(cx, |list_state, cx| {
                                                list_state.delegate().database_store.update(
                                                    cx,
                                                    |store, cx| {
                                                        store.remove_from_pipeline(id, cx);
                                                    },
                                                );
                                            });
                                        }),
                                )
                            }
                        })
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .py_2()
                                .px_1()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .size(px(6.0))
                                        .rounded_full()
                                        .bg(theme.muted_foreground.opacity(0.4)),
                                )
                                .child(Label::new(title.clone()).truncate().flex_1())
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground.opacity(0.5))
                                        .child("drag to queue"),
                                ),
                        ),
                )
                .selected(is_selected)
                .on_click(cx.listener(move |list_state, _event, _window, cx| {
                    if Some(ix) == list_state.delegate().selected_index {
                        cx.emit(StartActionEditor {
                            action_id: Some(id),
                        });
                    } else {
                        list_state.delegate_mut().selected_index = Some(ix);
                        cx.notify();
                    }
                })),
        )
    }

    fn set_selected_index(&mut self, ix: Option<usize>) {
        self.selected_index = ix;
    }
}

impl EventEmitter<StartActionEditor> for ListState<BacklogListDelegate> {}

pub struct BacklogListView {
    pub list_state: Entity<ListState<BacklogListDelegate>>,
    _subscriptions: Vec<Subscription>,
}

impl BacklogListView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = BacklogListDelegate::new(database_store.clone(), cx);
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, _store, _event: &PipelineChanged, cx| {
                this.list_state.update(cx, |list_state, cx| {
                    list_state.delegate_mut().update_actions(cx);
                    cx.notify();
                });
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |_this, _store, event: &DatabaseError, _cx| {
                eprintln!("BacklogListView database error: {}", event.message);
            },
        ));

        subscriptions.push(cx.subscribe(
            &list_state,
            |_this, _list_state, _event: &ListEvent, _cx| {},
        ));

        Self {
            list_state,
            _subscriptions: subscriptions,
        }
    }
}

impl EventEmitter<NavigateToView> for BacklogListView {}

impl Render for BacklogListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("backlog-list")
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
                                Button::new("backlog-back")
                                    .icon(IconName::ArrowLeft)
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|_this, _event, _window, cx| {
                                        cx.emit(NavigateToView {
                                            mode: MainViewMode::Home,
                                        });
                                    })),
                            )
                            .child(Label::new("Backlog").text_2xl().font(gpui::font("Georgia"))),
                    )
                    .child(
                        Label::new("Drag items into the pipeline queue")
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(Divider::horizontal().color(cx.theme().border).w_full())
            .child(
                div()
                    .flex_1()
                    .id("backlog-scroll")
                    .overflow_y_scroll()
                    .child(List::new(&self.list_state)),
            )
    }
}
