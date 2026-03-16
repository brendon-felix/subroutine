use chrono::{DateTime, Local, Utc};
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    div, prelude::FluentBuilder as _, px,
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
use simple_core::ActionCompletion;
use uuid::Uuid;

use crate::{
    components::custom_list::{List, ListDelegate, ListItem, ListState},
    stores::{
        DatabaseStore,
        database_store::{CompletionsLoaded, DatabaseError},
    },
    views::main_view::MainViewMode,
};

#[derive(Clone, Debug)]
pub struct NavigateFromCompletions {
    pub mode: MainViewMode,
}

pub struct CompletionsDelegate {
    completions: Vec<ActionCompletion>,
    action_titles: std::collections::HashMap<Uuid, String>,
    selected_index: Option<usize>,
    database_store: Entity<DatabaseStore>,
}

impl CompletionsDelegate {
    pub fn new(database_store: Entity<DatabaseStore>, cx: &App) -> Self {
        let store = database_store.read(cx);
        let completions = store.get_completions().clone();
        let action_titles = Self::build_title_map(&store.actions);

        Self {
            completions,
            action_titles,
            selected_index: None,
            database_store,
        }
    }

    fn build_title_map(actions: &[simple_core::Action]) -> std::collections::HashMap<Uuid, String> {
        actions.iter().map(|a| (a.id, a.title.clone())).collect()
    }

    pub fn update_completions(&mut self, cx: &App) {
        let store = self.database_store.read(cx);
        self.completions = store.get_completions().clone();
        self.action_titles = Self::build_title_map(&store.actions);
    }

    fn format_timestamp(dt: DateTime<Utc>) -> String {
        let local: DateTime<Local> = dt.into();
        local.format("%Y-%m-%d %H:%M").to_string()
    }
}

impl ListDelegate for CompletionsDelegate {
    fn items_count(&self, _cx: &App) -> usize {
        self.completions.len()
    }

    fn render_item(
        &mut self,
        ix: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let completion = self.completions.get(ix)?.clone();
        let completion_id = completion.id;
        let is_selected = Some(ix) == self.selected_index;
        let theme = cx.theme().clone();

        let list_entity = cx.entity();

        let action_title: SharedString = self
            .action_titles
            .get(&completion.action_id)
            .cloned()
            .unwrap_or_else(|| format!("Action {}", &completion.action_id.to_string()[..8]))
            .into();

        let timestamp: SharedString = Self::format_timestamp(completion.completed_at).into();

        let notes: Option<SharedString> = completion
            .notes
            .as_deref()
            .map(|n| n.replace('\n', " ").replace('\r', " ").into());

        Some(
            ListItem::new(ix)
                .rounded_md()
                .child(
                    h_flex()
                        .w_full()
                        .items_start()
                        .py_2()
                        .px_1()
                        .gap_3()
                        .context_menu(move |menu, _window, _cx| {
                            let list_entity = list_entity.clone();
                            menu.item(
                                PopupMenuItem::new("Delete")
                                    .icon(IconName::Delete)
                                    .on_click(move |_event, _window, cx| {
                                        list_entity.update(cx, |list_state, cx| {
                                            list_state.delegate().database_store.update(
                                                cx,
                                                |store, cx| {
                                                    store.delete_completion(completion_id, cx);
                                                },
                                            );
                                        });
                                    }),
                            )
                        })
                        .child(
                            div()
                                .flex_shrink_0()
                                .mt(px(6.0))
                                .size(px(6.0))
                                .rounded_full()
                                .bg(theme.success),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .child(Label::new(action_title).truncate().flex_1())
                                        .child(
                                            Label::new(timestamp)
                                                .text_xs()
                                                .text_color(theme.muted_foreground),
                                        ),
                                )
                                .when_some(notes, |this, notes_text| {
                                    this.child(
                                        Label::new(notes_text)
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .truncate(),
                                    )
                                }),
                        ),
                )
                .selected(is_selected)
                .on_click(cx.listener(move |list_state, _event, _window, cx| {
                    list_state.delegate_mut().selected_index = Some(ix);
                    cx.notify();
                })),
        )
    }

    fn set_selected_index(&mut self, ix: Option<usize>) {
        self.selected_index = ix;
    }
}

pub struct CompletionsView {
    list_state: Entity<ListState<CompletionsDelegate>>,
    _subscriptions: Vec<Subscription>,
}

impl CompletionsView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let delegate = CompletionsDelegate::new(database_store.clone(), cx);
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, _store, _event: &CompletionsLoaded, cx| {
                this.list_state.update(cx, |list_state, cx| {
                    list_state.delegate_mut().update_completions(cx);
                    cx.notify();
                });
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |_this, _store, event: &DatabaseError, _cx| {
                eprintln!("CompletionsView database error: {}", event.message);
            },
        ));

        Self {
            list_state,
            _subscriptions: subscriptions,
        }
    }
}

impl EventEmitter<NavigateFromCompletions> for CompletionsView {}

impl Render for CompletionsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.list_state.read(cx).delegate().completions.len();

        v_flex()
            .id("completions-view")
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
                                Button::new("completions-back")
                                    .icon(IconName::ArrowLeft)
                                    .ghost()
                                    .small()
                                    .on_click(cx.listener(|_this, _event, _window, cx| {
                                        cx.emit(NavigateFromCompletions {
                                            mode: MainViewMode::Home,
                                        });
                                    })),
                            )
                            .child(
                                Label::new("Completions")
                                    .text_2xl()
                                    .font(gpui::font("Georgia")),
                            ),
                    )
                    .child(
                        Label::new(format!("{} total", count))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(Divider::horizontal().color(cx.theme().border).w_full())
            .child(
                div()
                    .flex_1()
                    .id("completions-scroll")
                    .overflow_y_scroll()
                    .child(List::new(&self.list_state)),
            )
    }
}
