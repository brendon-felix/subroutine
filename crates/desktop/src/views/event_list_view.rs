use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, actions, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme, IconName, IndexPath, Selectable, Sizable,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    menu::{ContextMenuExt, PopupMenuItem},
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use simple_core::Event;
use uuid::Uuid;

use crate::{
    components::custom_list::{List, ListDelegate, ListEvent, ListItem, ListState},
    stores::{
        DatabaseStore,
        database_store::{DatabaseError, EventsLoaded},
    },
    views::{event_editor::StartEventEditor, main_view::MainViewMode},
};

actions!(event_list, [DeleteEvent]);

impl EventEmitter<StartEventEditor> for ListState<EventListDelegate> {}

#[derive(Clone, Debug)]
pub struct NavigateFromEventList {
    pub mode: MainViewMode,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum EventFilter {
    #[default]
    All,
    Recurring,
    OneOff,
}

pub struct EventListDelegate {
    events: Vec<Event>,
    filtered: Vec<Event>,
    is_searching: bool,
    search_query: String,
    filter: EventFilter,
    selected_index: Option<usize>,
    pub database_store: Entity<DatabaseStore>,
}

impl EventListDelegate {
    pub fn new(database_store: Entity<DatabaseStore>, cx: &App) -> Self {
        let events = database_store.read(cx).events.clone();

        Self {
            events,
            filtered: Vec::new(),
            is_searching: false,
            search_query: String::new(),
            filter: EventFilter::default(),
            selected_index: None,
            database_store,
        }
    }

    pub fn set_filter(&mut self, filter: EventFilter, cx: &App) {
        self.filter = filter;
        self.update_events(cx);
    }

    pub fn update_events(&mut self, cx: &App) {
        self.events = self.database_store.read(cx).events.clone();
        self.rebuild_filtered();
    }

    fn rebuild_filtered(&mut self) {
        let search_active = self.is_searching && !self.search_query.is_empty();

        let base: Vec<Event> = if search_active {
            let query = self.search_query.to_lowercase();
            self.events
                .iter()
                .filter(|event| event.title.to_lowercase().contains(&query))
                .cloned()
                .collect()
        } else {
            self.events.clone()
        };

        self.filtered = base
            .into_iter()
            .filter(|event| match self.filter {
                EventFilter::All => true,
                EventFilter::Recurring => event.recurrence.is_some(),
                EventFilter::OneOff => event.recurrence.is_none(),
            })
            .collect();

        let needs_filter = search_active || self.filter != EventFilter::All;
        if !needs_filter {
            self.is_searching = false;
            self.filtered.clear();
        }
    }

    fn current_events(&self) -> &[Event] {
        let needs_filter =
            (self.is_searching && !self.search_query.is_empty()) || self.filter != EventFilter::All;
        if needs_filter {
            &self.filtered
        } else {
            &self.events
        }
    }
}

impl ListDelegate for EventListDelegate {
    fn items_count(&self, _cx: &App) -> usize {
        self.current_events().len()
    }

    fn render_item(
        &mut self,
        ix: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<EventListDelegate>>,
    ) -> Option<ListItem> {
        let event = self.current_events().get(ix)?.clone();
        let id = event.id;
        let title = SharedString::from(event.title.replace('\n', " ").replace('\r', " "));
        let is_selected = Some(ix) == self.selected_index;
        let has_recurrence = event.recurrence.is_some();

        let list_entity = cx.entity();

        Some(
            ListItem::new(ix)
                .rounded_md()
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .py_2()
                        .px_1()
                        .gap_2()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_3()
                                .min_w_0()
                                .flex_1()
                                .child(Label::new(title).truncate())
                                .when(has_recurrence, |this| {
                                    this.child(
                                        Label::new("↻")
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                }),
                        )
                        .context_menu({
                            move |menu, _window, _cx| {
                                let entity_edit = list_entity.clone();
                                let entity_delete = list_entity.clone();
                                menu.item(
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
                                    PopupMenuItem::new("Delete")
                                        .icon(IconName::Delete)
                                        .on_click(move |_event, _window, cx| {
                                            entity_delete.update(cx, |list_state, cx| {
                                                list_state.delegate().database_store.update(
                                                    cx,
                                                    |store, cx| {
                                                        store.delete_event(id, cx);
                                                    },
                                                );
                                                cx.notify();
                                            });
                                        }),
                                )
                            }
                        }),
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

pub struct EventListView {
    pub list_state: Entity<ListState<EventListDelegate>>,
    filter_selection: Entity<SelectState<Vec<&'static str>>>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl EventListView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, _store, _event: &EventsLoaded, cx| {
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
                eprintln!("EventListView: Database error: {}", event.message);
            },
        ));

        let delegate = EventListDelegate::new(database_store, cx);
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        subscriptions.push(cx.subscribe(
            &list_state,
            |_this, _list_state, event: &ListEvent, cx| {
                if let ListEvent::Confirm(_ix) = event {
                    cx.notify();
                }
            },
        ));

        let filter_selection = cx.new(|cx| {
            SelectState::new(
                vec!["All Events", "Recurring", "One-off"],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });

        subscriptions.push(cx.subscribe_in(
            &filter_selection,
            window,
            |this, _select, event: &SelectEvent<Vec<&'static str>>, _window, cx| {
                let SelectEvent::Confirm(value) = event;
                let filter = match value.as_deref() {
                    Some("Recurring") => EventFilter::Recurring,
                    Some("One-off") => EventFilter::OneOff,
                    _ => EventFilter::All,
                };
                this.list_state.update(cx, |list_state, cx| {
                    list_state.delegate_mut().set_filter(filter, cx);
                    cx.notify();
                });
            },
        ));

        Self {
            list_state,
            filter_selection,
            _subscriptions: subscriptions,
        }
    }

    fn update_event_list(&mut self, cx: &mut Context<Self>) {
        self.list_state.update(cx, |list_state, cx| {
            list_state.delegate_mut().update_events(cx);
            cx.notify();
        });
        cx.notify();
    }
}

impl EventEmitter<NavigateFromEventList> for EventListView {}

impl Render for EventListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("event-list")
            .size_full()
            .p_4()
            .gap_4()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new("events-to-home-btn")
                                    .icon(IconName::ArrowLeft)
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|_this, _event, _window, cx| {
                                        cx.emit(NavigateFromEventList {
                                            mode: MainViewMode::Home,
                                        });
                                    })),
                            )
                            .child(Label::new("Events").text_2xl().font(gpui::font("Georgia"))),
                    )
                    .child(
                        Select::new(&self.filter_selection)
                            .appearance(false)
                            .text_center()
                            .rounded_md()
                            .w(px(144.0)),
                    ),
            )
            .child(Divider::horizontal().color(cx.theme().border).w_full())
            .child(
                div()
                    .flex_1()
                    .id("events-scroll")
                    .overflow_y_scroll()
                    .child(List::new(&self.list_state)),
            )
    }
}
