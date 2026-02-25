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

use app_core::PipelineEntry;

use crate::stores::DatabaseStore;
use crate::stores::database_store::PipelineChanged;
use crate::views::MainViewMode;

pub struct NavigateFromFocus {
    pub mode: MainViewMode,
}

pub struct FocusView {
    database_store: Entity<DatabaseStore>,
    entries: Vec<PipelineEntry>,
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
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        cx.focus_self(window);

        let entries = database_store
            .read(cx)
            .get_pipeline()
            .queue()
            .iter()
            .filter(|e| !e.is_transition())
            .take(3)
            .cloned()
            .collect();

        cx.subscribe(
            &database_store,
            |this, store, _event: &PipelineChanged, cx| {
                this.entries = store
                    .read(cx)
                    .get_pipeline()
                    .queue()
                    .iter()
                    .filter(|e| !e.is_transition())
                    .take(3)
                    .cloned()
                    .collect();
                this.selected_index = 0;
                cx.notify();
            },
        )
        .detach();

        Self {
            database_store,
            entries,
            selected_index: 0,
            focus_handle,
        }
    }

    fn visible_entries(&self) -> &[PipelineEntry] {
        &self.entries
    }

    fn select_next(&mut self, cx: &mut Context<Self>) {
        let count = self.visible_entries().len();
        if count == 0 {
            return;
        }
        self.selected_index = (self.selected_index + 1) % count;
        cx.notify();
    }

    fn select_previous(&mut self, cx: &mut Context<Self>) {
        let count = self.visible_entries().len();
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

    pub fn refresh_entries(&mut self, cx: &mut Context<Self>) {
        self.database_store.update(cx, |store, cx| {
            store.refresh_pipeline(cx);
        });
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
                            mode: MainViewMode::Home,
                        });
                    })),
            )
            .child(
                Label::new("Focus Mode")
                    .text_2xl()
                    .font(gpui::font("Georgia")),
            )
            .child(
                h_flex().gap_2().child(
                    Button::new("focus-refresh")
                        .icon(IconName::Redo)
                        .ghost()
                        .small()
                        .tooltip("Re-score pipeline")
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.refresh_entries(cx);
                        })),
                ),
            )
    }

    fn render_empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(
                Button::new("focus-empty-refresh")
                    .label("Re-score")
                    .outline()
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.refresh_entries(cx);
                    })),
            )
    }

    fn render_entry_card(
        &self,
        entity: gpui::Entity<Self>,
        ix: usize,
        entry: &PipelineEntry,
        score: f32,
        cx: &App,
    ) -> impl IntoElement {
        let is_selected = ix == self.selected_index;
        let title = entry.title().to_string();
        let score_display = format!("{:.0}%", (score.clamp(0.0, 1.0) * 100.0));
        let theme = cx.theme().clone();

        div()
            .id(ElementId::NamedInteger("focus-entry".into(), ix as u64))
            .w_full()
            .p_4()
            .rounded_lg()
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
            .cursor_pointer()
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |this, cx| {
                    this.selected_index = ix;
                    cx.notify();
                });
            })
            .child(
                v_flex().gap_1().child(Label::new(title).text_base()).child(
                    Label::new(format!("Priority: {}", score_display))
                        .text_xs()
                        .text_color(theme.muted_foreground),
                ),
            )
    }
}

impl Render for FocusView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        let content = if self.visible_entries().is_empty() {
            v_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .justify_center()
                .gap_4()
                .child(self.render_empty_state(cx))
        } else {
            let scores: Vec<f32> = self
                .entries
                .iter()
                .map(|entry| self.database_store.read(cx).score_entry(entry))
                .collect();

            let entity = cx.entity();
            let mut cards = Vec::new();
            for (ix, entry) in self.entries.iter().enumerate() {
                let score = scores.get(ix).copied().unwrap_or(0.0);
                cards.push(self.render_entry_card(entity.clone(), ix, entry, score, cx));
            }

            let selected_entry_id = self
                .visible_entries()
                .get(self.selected_index)
                .map(|e| e.id());

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
                    .when_some(selected_entry_id, |row, entry_id| {
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
                                        store.demote(entry_id, cx);
                                    });
                                })),
                        )
                        .child(
                            Button::new("focus-remove")
                                .label("Remove")
                                .ghost()
                                .tooltip("Remove from pipeline entirely")
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.database_store.update(cx, |store, cx| {
                                        store.remove_from_pipeline(entry_id, cx);
                                    });
                                })),
                        )
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
