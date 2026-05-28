use gpui::prelude::FluentBuilder;
use gpui::prelude::*;
use gpui::{
    App, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Window, div, px,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    v_flex,
};

use simple_core::AnyItem;

use crate::stores::AppDatabaseStore;
use crate::stores::database_store::DataChanged;
use crate::views::MainViewMode;

pub struct NavigateFromFocus {
    pub mode: MainViewMode,
}

pub struct FocusView {
    database_store: Entity<AppDatabaseStore>,
    entries: Vec<AnyItem>,
    selected_index: usize,
    focus_handle: FocusHandle,
}

impl EventEmitter<NavigateFromFocus> for FocusView {}

impl Focusable for FocusView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl FocusView {
    pub fn new(
        database_store: Entity<AppDatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        cx.focus_self(window);

        let entries = database_store
            .read(cx)
            .sorted_queue()
            .into_iter()
            .take(3)
            .collect();

        cx.subscribe(&database_store, |this, store, _event: &DataChanged, cx| {
            this.entries = store.read(cx).sorted_queue().into_iter().take(3).collect();
            this.selected_index = 0;
            cx.notify();
        })
        .detach();

        Self {
            database_store,
            entries,
            selected_index: 0,
            focus_handle,
        }
    }

    fn select_next(&mut self, cx: &mut Context<Self>) {
        let count = self.entries.len();
        if count == 0 {
            return;
        }
        self.selected_index = (self.selected_index + 1) % count;
        cx.notify();
    }

    fn select_previous(&mut self, cx: &mut Context<Self>) {
        let count = self.entries.len();
        if count == 0 {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = count - 1;
        } else {
            self.selected_index -= 1;
        }
        cx.notify();
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .child(
                Button::new("focus-back")
                    .icon(IconName::ArrowLeft)
                    .ghost()
                    .small()
                    .on_click(cx.listener(|_this, _event, _window, cx| {
                        cx.emit(NavigateFromFocus {
                            mode: MainViewMode::Dashboard,
                        });
                    })),
            )
            .child(
                Label::new("Focus Mode")
                    .text_2xl()
                    .font(gpui::font("Georgia")),
            )
            .child(div().w_8())
    }

    fn render_empty_state(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .items_center()
            .justify_center()
            .py_10()
            .gap_2()
            .child(Label::new("Nothing in the queue yet").text_color(cx.theme().muted_foreground))
            .child(
                Label::new("Add actions to the pipeline to see them here")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
    }

    fn render_entry_card(
        &self,
        entity: gpui::Entity<Self>,
        ix: usize,
        entry: &AnyItem,
        cx: &App,
    ) -> impl IntoElement {
        let is_selected = ix == self.selected_index;
        let title = entry.title().to_string();
        let theme = cx.theme().clone();

        div()
            .id(ElementId::NamedInteger("focus-entry".into(), ix as u64))
            .w_full()
            .p_4()
            .rounded_xl()
            .border_1()
            .border_color(if is_selected {
                theme.accent
            } else {
                theme.border
            })
            .bg(if is_selected {
                theme.list_hover.opacity(0.4)
            } else {
                theme.background
            })
            .hover(|style| style.bg(theme.list_hover.opacity(0.35)))
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |this, cx| {
                    this.selected_index = ix;
                    cx.notify();
                });
            })
            .child(v_flex().gap_1().child(Label::new(title).text_base()))
    }
}

impl Render for FocusView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        let content = if self.entries.is_empty() {
            v_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .justify_center()
                .gap_4()
                .child(self.render_empty_state(cx))
                .into_any_element()
        } else {
            let entity = cx.entity();
            let mut cards = Vec::new();
            for (ix, entry) in self.entries.iter().enumerate() {
                cards.push(self.render_entry_card(entity.clone(), ix, entry, cx));
            }

            let selected_entry = self.entries.get(self.selected_index).cloned();

            let navigation_controls = h_flex()
                .gap_2()
                .items_center()
                .child(
                    Button::new("focus-prev")
                        .icon(IconName::ChevronLeft)
                        .ghost()
                        .small()
                        .tooltip("Previous")
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.select_previous(cx);
                        })),
                )
                .child(
                    Button::new("focus-next")
                        .icon(IconName::ChevronRight)
                        .ghost()
                        .small()
                        .tooltip("Next")
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.select_next(cx);
                        })),
                );

            let actions =
                h_flex()
                    .gap_2()
                    .items_center()
                    .when_some(selected_entry, |row, entry| {
                        let entry_id = entry.id();
                        let is_action = matches!(entry, AnyItem::Action(_));

                        row.when(is_action, |row| {
                            row.child(
                                Button::new("focus-complete")
                                    .label("Complete")
                                    .primary()
                                    .on_click(cx.listener(move |this, _event, _window, cx| {
                                        this.database_store.update(cx, |store, cx| {
                                            store.complete_action(entry_id, cx);
                                        });
                                    })),
                            )
                            .child(
                                Button::new("focus-demote")
                                    .label("Later")
                                    .ghost()
                                    .tooltip("Move back to backlog")
                                    .on_click(cx.listener(move |this, _event, _window, cx| {
                                        this.database_store.update(cx, |store, cx| {
                                            store.backlog_action(entry_id, cx);
                                        });
                                    })),
                            )
                        })
                        // .child(
                        //     Button::new("focus-remove")
                        //         .label("Delete")
                        //         .ghost()
                        //         .tooltip("Remove from pipeline entirely")
                        //         .on_click(cx.listener(move |this, _event, _window, cx| {
                        //             this.database_store.update(cx, |store, cx| {
                        //                 // store.remove_from_pipeline(entry_id, cx);
                        //             });
                        //         })),
                        // )
                    });

            v_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .justify_center()
                .gap_4()
                .child(v_flex().w(px(520.0)).gap_3().children(cards))
                .child(navigation_controls)
                .child(actions)
                .into_any_element()
        };

        v_flex()
            .size_full()
            .px_8()
            .py_6()
            .gap_4()
            .child(self.render_header(cx))
            .child(Divider::horizontal().color(theme.border).w_full())
            .child(content)
    }
}
