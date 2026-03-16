use chrono::Utc;
use gpui::prelude::*;
use gpui::{
    App, Context, DragMoveEvent, Entity, EventEmitter, IntoElement, Render, SharedString,
    Subscription, Window, div, px,
};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::{ActiveTheme, IconName, Sizable, h_flex, label::Label, v_flex};
use simple_core::Action;
use uuid::Uuid;

use crate::{
    components::drag_drop::{DragData, Draggable, DropZone},
    stores::{
        DatabaseStore,
        database_store::{DatabaseError, PipelineChanged},
    },
    views::action_editor::StartActionEditor,
};

pub struct BacklogView {
    database_store: Entity<DatabaseStore>,
    actions: Vec<Action>,
    drop_active: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<StartActionEditor> for BacklogView {}

impl BacklogView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let actions = database_store.read(cx).get_pipeline().backlog.clone();

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, store, _event: &PipelineChanged, cx| {
                this.actions = store.read(cx).get_pipeline().backlog.clone();
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |_this, _store, event: &DatabaseError, _cx| {
                eprintln!("BacklogView database error: {}", event.message);
            },
        ));

        Self {
            database_store,
            actions,
            drop_active: false,
            _subscriptions: subscriptions,
        }
    }

    fn promote(&mut self, action_id: Uuid, cx: &mut Context<Self>) {
        self.database_store.update(cx, |store, cx| {
            store.promote(action_id, Utc::now(), cx);
        });
    }

    fn render_item(&self, action: &Action, ix: usize, cx: &Context<Self>) -> impl IntoElement {
        let action_id = action.id;
        let title = SharedString::from(action.title.clone());
        let theme = cx.theme().clone();
        let entity = cx.entity();

        let drag_data = DragData::new(action.clone()).with_label(title.clone());

        Draggable::new(("backlog-item", ix as u64), drag_data)
            .w_full()
            .context_menu({
                let entity = entity.clone();
                move |menu, _window, _cx| {
                    let entity_promote = entity.clone();
                    let entity_edit = entity.clone();
                    let entity_delete = entity.clone();
                    menu.item(
                        PopupMenuItem::new("Promote to queue")
                            .icon(IconName::ArrowUp)
                            .on_click(move |_event, _window, cx: &mut App| {
                                entity_promote.update(cx, |this, cx| {
                                    this.promote(action_id, cx);
                                });
                            }),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new("Edit")
                            .icon(IconName::Settings2)
                            .on_click(move |_event, _window, cx: &mut App| {
                                entity_edit.update(cx, |_this, cx| {
                                    cx.emit(StartActionEditor {
                                        action_id: Some(action_id),
                                    });
                                });
                            }),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new("Remove from backlog")
                            .icon(IconName::Minus)
                            .on_click(move |_event, _window, cx: &mut App| {
                                entity_delete.update(cx, |this, cx| {
                                    this.database_store.update(cx, |store, cx| {
                                        store.remove_from_pipeline(action_id, cx);
                                    });
                                });
                            }),
                    )
                }
            })
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1p5()
                    .rounded_md()
                    .hover(|s| s.bg(theme.list_hover))
                    .child(
                        div()
                            .flex_shrink_0()
                            .size(px(6.0))
                            .rounded_full()
                            .bg(theme.muted_foreground.opacity(0.5)),
                    )
                    .child(Label::new(title).text_sm().truncate().flex_1()),
            )
    }
}

impl Render for BacklogView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let entity = cx.entity();
        let drop_active = self.drop_active;

        if self.actions.is_empty() {
            return v_flex()
                .w_full()
                .py_4()
                .items_center()
                .child(
                    Label::new("Backlog is empty")
                        .text_xs()
                        .text_color(theme.muted_foreground),
                )
                .into_any_element();
        }

        // Render items outside the drop-zone closure to avoid borrow of cx.
        let mut item_elements = Vec::new();
        for (ix, action) in self.actions.iter().enumerate() {
            item_elements.push(self.render_item(action, ix, cx).into_any_element());
        }

        let mouse_position = window.mouse_position();

        DropZone::<DragData<Action>>::new("backlog-drop-zone")
            .w_full()
            .active(drop_active)
            .on_drop(cx.listener(|this, data: &DragData<Action>, _window, cx| {
                // A backlog item was dragged back onto the backlog — no-op.
                let _ = data;
                this.drop_active = false;
                cx.notify();
            }))
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DragData<Action>>, _window, cx| {
                    if event.bounds.contains(&mouse_position) {
                        if !this.drop_active {
                            this.drop_active = true;
                            cx.notify();
                        }
                    } else if this.drop_active {
                        this.drop_active = false;
                        cx.notify();
                    }
                },
            ))
            .children(item_elements)
            .into_any_element()
    }
}

// ── Queue drop zone wrapper ───────────────────────────────────────────────────
// This is a thin component that wraps the queue Pipeline view and accepts
// drops of backlog actions, promoting them.

pub struct QueueDropTarget {
    database_store: Entity<DatabaseStore>,
    drop_active: bool,
}

impl QueueDropTarget {
    pub fn new(database_store: Entity<DatabaseStore>) -> Self {
        Self {
            database_store,
            drop_active: false,
        }
    }

    pub fn is_drop_active(&self) -> bool {
        self.drop_active
    }
}

/// A standalone stateless helper that renders a `DropZone` wrapping arbitrary
/// children and calls `store.promote` when an `Action` is dropped into it.
pub fn queue_drop_zone(
    database_store: Entity<DatabaseStore>,
    drop_active: bool,
    child: impl IntoElement + 'static,
    on_active_change: impl Fn(bool, &mut App) + 'static,
    cx: &mut Context<impl EventEmitter<()> + 'static>,
) -> impl IntoElement {
    let on_active_change = std::rc::Rc::new(on_active_change);
    let on_active_change_move = on_active_change.clone();

    DropZone::<DragData<Action>>::new("queue-drop-zone")
        .w_full()
        .active(drop_active)
        .on_drop({
            let database_store = database_store.clone();
            cx.listener(move |_this, data: &DragData<Action>, _window, cx| {
                let action_id = data.data.id;
                database_store.update(cx, |store, cx| {
                    store.promote(action_id, Utc::now(), cx);
                });
                on_active_change(false, cx);
            })
        })
        .on_drag_move(cx.listener(
            move |_this, event: &DragMoveEvent<DragData<Action>>, window, cx| {
                let is_over = event.bounds.contains(&window.mouse_position());
                on_active_change_move(is_over, cx);
            },
        ))
        .child(child)
}
