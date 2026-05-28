use gpui::{
    App, AppContext as _, Context, DragMoveEvent, Entity, EventEmitter, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, IconName, Selectable, h_flex,
    label::Label,
    menu::{ContextMenuExt, PopupMenuItem},
    v_flex,
};
use simple_core::Action;

use crate::{
    components::{
        custom_list::{List, ListDelegate, ListEvent, ListItem, ListState},
        drag_drop::{DragData, Draggable, DropZone},
    },
    stores::{
        AppDatabaseStore,
        database_store::{ActionDataChanged, DatabaseError},
    },
    utils::format_recurrence,
    views::action_editor::StartActionEditor,
};

fn format_duration(d: chrono::Duration) -> String {
    let hours = d.num_hours();
    let mins = d.num_minutes() % 60;
    match (hours, mins) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

// fn format_recurrence(d: chrono::Duration) -> String {
//     let days = d.num_days();
//     match days {
//         1 => "daily".into(),
//         7 => "weekly".into(),
//         14 => "fortnightly".into(),
//         28 | 30 | 31 => "monthly".into(),
//         365 | 366 => "yearly".into(),
//         n if n % 7 == 0 => format!("every {} weeks", n / 7),
//         n => format!("every {} days", n),
//     }
// }

// ── Delegate ──────────────────────────────────────────────────────────────────

pub struct SavedActionsDelegate {
    pub actions: Vec<Action>,
    selected_index: Option<usize>,
    pub database_store: Entity<AppDatabaseStore>,
}

impl SavedActionsDelegate {
    pub fn new(database_store: Entity<AppDatabaseStore>, cx: &App) -> Self {
        let actions = database_store
            .read(cx)
            .actions()
            .iter()
            .filter(|a| a.saved)
            .cloned()
            .collect();
        Self {
            actions,
            selected_index: None,
            database_store,
        }
    }

    pub fn update_actions(&mut self, cx: &App) {
        self.actions = self
            .database_store
            .read(cx)
            .actions()
            .iter()
            .filter(|a| a.saved)
            .cloned()
            .collect();
    }
}

impl ListDelegate for SavedActionsDelegate {
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
        let duration_label = action.duration.map(format_duration);
        let recurrence_label = action.recurrence.map(format_recurrence);
        let has_content = action.content.as_ref().is_some_and(|c| !c.is_empty());

        let drag_data = DragData::new(action.clone()).with_label(title.clone());

        Some(
            ListItem::new(ix)
                .rounded_md()
                .child(
                    Draggable::new(("saved-actions-drag", ix as u64), drag_data)
                        .w_full()
                        .context_menu({
                            let list_entity = cx.entity();
                            move |menu, _window, _cx| {
                                let entity_add = list_entity.clone();
                                let entity_edit = list_entity.clone();
                                menu.item(
                                    PopupMenuItem::new("Add to pipeline")
                                        .icon(IconName::ArrowUp)
                                        .on_click(move |_event, _window, cx| {
                                            entity_add.update(cx, |list_state, cx| {
                                                let action = list_state
                                                    .delegate()
                                                    .actions
                                                    .iter()
                                                    .find(|a| a.id == id)
                                                    .cloned()
                                                    .map(|a| a.with_saved(true));
                                                if let Some(action) = action {
                                                    list_state.delegate().database_store.update(
                                                        cx,
                                                        |store, cx| {
                                                            store.upsert_action(action, cx);
                                                        },
                                                    );
                                                }
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
                            }
                        })
                        .child(
                            h_flex()
                                .w_full()
                                .items_start()
                                .py_2()
                                .px_1()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .mt(px(5.0))
                                        .size(px(6.0))
                                        .rounded_full()
                                        .bg(theme.muted_foreground.opacity(0.4)),
                                )
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .flex_1()
                                        .gap_0p5()
                                        .child(Label::new(title.clone()).truncate())
                                        .when(
                                            duration_label.is_some()
                                                || recurrence_label.is_some()
                                                || has_content,
                                            |this| {
                                                this.child(
                                                    h_flex()
                                                        .gap_2()
                                                        .when(has_content, |row| {
                                                            row.child(
                                                                Label::new("note")
                                                                    .text_xs()
                                                                    .text_color(
                                                                        theme
                                                                            .muted_foreground
                                                                            .opacity(0.6),
                                                                    ),
                                                            )
                                                        })
                                                        .when_some(duration_label, |row, d| {
                                                            row.child(
                                                                Label::new(d).text_xs().text_color(
                                                                    theme.muted_foreground,
                                                                ),
                                                            )
                                                        })
                                                        .when_some(recurrence_label, |row, r| {
                                                            row.child(
                                                                h_flex()
                                                                    .gap_1()
                                                                    .items_center()
                                                                    .child(
                                                                        Label::new("↻")
                                                                            .text_xs()
                                                                            .text_color(
                                                                            theme.muted_foreground,
                                                                        ),
                                                                    )
                                                                    .child(
                                                                        Label::new(r)
                                                                            .text_xs()
                                                                            .text_color(
                                                                            theme.muted_foreground,
                                                                        ),
                                                                    ),
                                                            )
                                                        }),
                                                )
                                            },
                                        ),
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

impl EventEmitter<StartActionEditor> for ListState<SavedActionsDelegate> {}

// ── View ──────────────────────────────────────────────────────────────────────

pub struct SavedActionsListView {
    pub list_state: Entity<ListState<SavedActionsDelegate>>,
    drop_active: bool,
    database_store: Entity<AppDatabaseStore>,
    _subscriptions: Vec<Subscription>,
}

impl SavedActionsListView {
    pub fn new(
        database_store: Entity<AppDatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = SavedActionsDelegate::new(database_store.clone(), cx);
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, _store, _event: &ActionDataChanged, cx| {
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
                eprintln!("SavedActionsListView database error: {}", event.message);
            },
        ));

        subscriptions.push(cx.subscribe(
            &list_state,
            |_this, _list_state, _event: &ListEvent, _cx| {},
        ));

        Self {
            list_state,
            drop_active: false,
            database_store,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for SavedActionsListView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drop_active = self.drop_active;
        let mouse_position = window.mouse_position();
        let is_empty = self.list_state.read(cx).delegate().actions.is_empty();

        DropZone::<DragData<Action>>::new("saved-actions-drop-zone")
            .size_full()
            .active(drop_active)
            .on_drop(cx.listener(|this, data: &DragData<Action>, _window, cx| {
                let mut action = data.data.clone();
                action.saved = true;
                this.database_store.update(cx, |store, cx| {
                    store.upsert_action(action, cx);
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
            .child(
                v_flex()
                    .size_full()
                    .when(is_empty, |this| {
                        this.items_center().justify_center().child(
                            Label::new("No saved actions")
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                    })
                    .when(!is_empty, |this| {
                        this.child(
                            div()
                                .id("saved-actions-scroll")
                                .size_full()
                                .overflow_y_scroll()
                                .p_2()
                                .child(List::new(&self.list_state)),
                        )
                    }),
            )
    }
}
