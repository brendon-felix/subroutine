use gpui::{
    App, AppContext as _, Context, DragMoveEvent, Entity, EventEmitter, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window,
    prelude::FluentBuilder,
};
use gpui_component::{
    ActiveTheme, IconName, Selectable, h_flex,
    label::Label,
    menu::{ContextMenuExt, PopupMenuItem},
    v_flex,
};
use simple_core::Event;

use crate::{
    components::{
        custom_list::{List, ListDelegate, ListEvent, ListItem, ListState},
        drag_drop::{DragData, Draggable, DropZone},
    },
    icons::AppIcon,
    stores::{
        AppDatabaseStore,
        database_store::{DatabaseError, EventDataChanged},
    },
    utils::format_recurrence,
    views::event_editor::StartEventEditor,
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

// ── Delegate ──────────────────────────────────────────────────────────────────

pub struct SavedEventsDelegate {
    pub events: Vec<Event>,
    selected_index: Option<usize>,
    pub database_store: Entity<AppDatabaseStore>,
}

impl SavedEventsDelegate {
    pub fn new(database_store: Entity<AppDatabaseStore>, cx: &App) -> Self {
        let events = database_store
            .read(cx)
            .events()
            .iter()
            .filter(|e| e.saved)
            .cloned()
            .collect();
        Self {
            events,
            selected_index: None,
            database_store,
        }
    }

    pub fn update_events(&mut self, cx: &App) {
        self.events = self
            .database_store
            .read(cx)
            .events()
            .iter()
            .filter(|e| e.saved)
            .cloned()
            .collect();
    }
}

impl ListDelegate for SavedEventsDelegate {
    fn items_count(&self, _cx: &App) -> usize {
        self.events.len()
    }

    fn render_item(
        &mut self,
        ix: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let event = self.events.get(ix)?.clone();
        let id = event.id;
        let title = gpui::SharedString::from(event.title.replace('\n', " ").replace('\r', " "));
        let is_selected = Some(ix) == self.selected_index;
        let theme = cx.theme().clone();
        let duration_label = event.duration.map(format_duration);
        let recurrence_label = event.recurrence.map(format_recurrence);

        let drag_data = DragData::new(event.clone()).with_label(title.clone());

        Some(
            ListItem::new(ix)
                .rounded_md()
                .child(
                    Draggable::new(("saved-events-drag", ix as u64), drag_data)
                        .w_full()
                        .context_menu({
                            let list_entity = cx.entity();
                            move |menu, _window, _cx| {
                                let entity_add = list_entity.clone();
                                let entity_edit = list_entity.clone();
                                let entity_delete = list_entity.clone();
                                menu.item(
                                    PopupMenuItem::new("Add to pipeline")
                                        .icon(IconName::ArrowUp)
                                        .on_click(move |_event, _window, cx| {
                                            entity_add.update(cx, |list_state, cx| {
                                                let event = list_state
                                                    .delegate()
                                                    .events
                                                    .iter()
                                                    .find(|e| e.id == id)
                                                    .cloned();
                                                if let Some(event) = event {
                                                    list_state.delegate().database_store.update(
                                                        cx,
                                                        |store, cx| {
                                                            store.upsert_event(event, cx);
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
                                                cx.emit(StartEventEditor { event_id: Some(id) });
                                            });
                                        }),
                                )
                                .separator()
                                .item(
                                    PopupMenuItem::new("Delete").icon(AppIcon::Trash).on_click(
                                        move |_event, _window, cx| {
                                            entity_delete.update(cx, |list_state, cx| {
                                                list_state.delegate().database_store.update(
                                                    cx,
                                                    |store, cx| {
                                                        store.delete_event(id, cx);
                                                    },
                                                );
                                            });
                                        },
                                    ),
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
                                    gpui::div()
                                        .flex_shrink_0()
                                        .mt(gpui::px(5.0))
                                        .size(gpui::px(6.0))
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
                                            duration_label.is_some() || recurrence_label.is_some(),
                                            |this| {
                                                this.child(
                                                    h_flex()
                                                        .gap_2()
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
                        cx.emit(StartEventEditor { event_id: Some(id) });
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

impl EventEmitter<StartEventEditor> for ListState<SavedEventsDelegate> {}

// ── View ──────────────────────────────────────────────────────────────────────

pub struct SavedEventsListView {
    pub list_state: Entity<ListState<SavedEventsDelegate>>,
    drop_active: bool,
    database_store: Entity<AppDatabaseStore>,
    _subscriptions: Vec<Subscription>,
}

impl SavedEventsListView {
    pub fn new(
        database_store: Entity<AppDatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = SavedEventsDelegate::new(database_store.clone(), cx);
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, _store, _event: &EventDataChanged, cx| {
                this.list_state.update(cx, |list_state, cx| {
                    list_state.delegate_mut().update_events(cx);
                    cx.notify();
                });
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |_this, _store, event: &DatabaseError, _cx| {
                eprintln!("SavedEventsListView database error: {}", event.message);
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

impl Render for SavedEventsListView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drop_active = self.drop_active;
        let mouse_position = window.mouse_position();
        let is_empty = self.list_state.read(cx).delegate().events.is_empty();

        DropZone::<DragData<Event>>::new("saved-events-drop-zone")
            .size_full()
            .active(drop_active)
            .on_drop(cx.listener(|this, data: &DragData<Event>, _window, cx| {
                let mut event = data.data.clone();
                event.saved = true;
                this.database_store.update(cx, |store, cx| {
                    store.upsert_event(event, cx);
                });
                this.drop_active = false;
                cx.notify();
            }))
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DragData<Event>>, _window, cx| {
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
                            Label::new("No saved events")
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                    })
                    .when(!is_empty, |this| {
                        this.child(
                            gpui::div()
                                .id("saved-events-scroll")
                                .size_full()
                                .overflow_y_scroll()
                                .p_2()
                                .child(List::new(&self.list_state)),
                        )
                    }),
            )
    }
}
