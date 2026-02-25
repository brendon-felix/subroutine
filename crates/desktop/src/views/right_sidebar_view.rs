use gpui::prelude::*;
use gpui::{
    Context, Entity, EventEmitter, IntoElement, Render, Subscription, Window, div, font, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::label::Label;
use gpui_component::{ActiveTheme, IconName, Sizable, h_flex, v_flex};

use crate::stores::DatabaseStore;
use crate::stores::database_store::{MentalStateChanged, PipelineChanged, SavedMentalStatesLoaded};
use crate::stores::drag_drop_store::DragDropStore;
use crate::views::Pipeline;

use app_core::SavedMentalState;

pub struct RightSidebarView {
    collapsed: bool,
    pub pipeline: Entity<Pipeline>,
    database_store: Entity<DatabaseStore>,
    saved_mental_states: Vec<SavedMentalState>,
    current_mental_state_name: Option<String>,
    context_expanded: bool,
    _subscriptions: Vec<Subscription>,
}

impl RightSidebarView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        drag_drop_store: Entity<DragDropStore>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, store, _event: &PipelineChanged, cx| {
                this.pipeline.update(cx, |pipeline, cx| {
                    pipeline.update_items(cx);
                    cx.notify();
                });
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, store, _event: &SavedMentalStatesLoaded, cx| {
                this.saved_mental_states = store.read(cx).get_saved_mental_states().clone();
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, store, _event: &MentalStateChanged, cx| {
                let mental_state = store.read(cx).get_mental_state();
                this.current_mental_state_name =
                    mental_state.declared.as_ref().map(|s| s.name.clone());
                cx.notify();
            },
        ));

        let saved_mental_states = database_store.read(cx).get_saved_mental_states().clone();
        let current_mental_state_name = database_store
            .read(cx)
            .get_mental_state()
            .declared
            .as_ref()
            .map(|s| s.name.clone());

        let pipeline_list = cx.new(|cx| Pipeline::new(database_store.clone(), drag_drop_store, cx));

        Self {
            collapsed: false,
            pipeline: pipeline_list,
            database_store,
            saved_mental_states,
            current_mental_state_name,
            context_expanded: true,
            _subscriptions: subscriptions,
        }
    }

    pub fn toggle_collapsed(&mut self, cx: &mut Context<Self>) -> bool {
        self.collapsed = !self.collapsed;
        cx.notify();
        self.collapsed
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn render_axis_row(
        label: impl Into<gpui::SharedString>,
        value: i8,
        negative_label: &str,
        positive_label: &str,
        cx: &Context<RightSidebarView>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let label: gpui::SharedString = label.into();
        let display = if value == 0 {
            "Neutral".to_string()
        } else if value < 0 {
            format!("{} ({})", negative_label, value)
        } else {
            format!("{} (+{})", positive_label, value)
        };

        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .child(
                Label::new(label)
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
            .child(Label::new(display).text_xs().text_color(theme.foreground))
    }

    fn render_mental_state_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mental_state = self.database_store.read(cx).get_mental_state().clone();
        let spoons = mental_state.remaining_spoons;
        let attention = mental_state.attention_mode();
        let sensory = mental_state.sensory_tolerance();
        let regulation = mental_state.emotional_regulation();
        let social = mental_state.social_battery();

        v_flex()
            .w_full()
            .px_3()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        Label::new("Mental State")
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        Button::new("toggle-context")
                            .icon(if self.context_expanded {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.context_expanded = !this.context_expanded;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.context_expanded, |this| {
                this.child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        // Declared state badge
                        .when_some(self.current_mental_state_name.clone(), |container, name| {
                            container.child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Label::new("State:")
                                            .text_xs()
                                            .text_color(theme.muted_foreground),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py(px(2.0))
                                            .rounded(px(4.0))
                                            .bg(theme.accent.opacity(0.15))
                                            .child(
                                                Label::new(name)
                                                    .text_xs()
                                                    .text_color(theme.accent_foreground),
                                            ),
                                    )
                                    .child(
                                        Button::new("clear-state")
                                            .icon(IconName::Close)
                                            .ghost()
                                            .xsmall()
                                            .tooltip("Clear declared state")
                                            .on_click(cx.listener(|this, _event, _window, cx| {
                                                this.database_store.update(cx, |store, cx| {
                                                    store.clear_declared_state(cx);
                                                });
                                            })),
                                    ),
                            )
                        })
                        // Spoons
                        .child(
                            h_flex()
                                .w_full()
                                .justify_between()
                                .items_center()
                                .child(
                                    Label::new("Spoons")
                                        .text_xs()
                                        .text_color(theme.muted_foreground),
                                )
                                .child(
                                    Label::new(format!("{}/10", spoons))
                                        .text_xs()
                                        .text_color(theme.foreground),
                                ),
                        )
                        // Axes (only when a state is declared)
                        .when(self.current_mental_state_name.is_some(), |this| {
                            this.child(Self::render_axis_row(
                                "Attention",
                                attention,
                                "Scattered",
                                "Hyperfocused",
                                cx,
                            ))
                            .child(Self::render_axis_row(
                                "Sensory",
                                sensory,
                                "Understimulated",
                                "Overstimulated",
                                cx,
                            ))
                            .child(Self::render_axis_row(
                                "Regulation",
                                regulation,
                                "Dysregulated",
                                "Regulated",
                                cx,
                            ))
                            .child(Self::render_axis_row(
                                "Social", social, "Drained", "Charged", cx,
                            ))
                        })
                        // Quick-declare buttons
                        .child(
                            v_flex()
                                .w_full()
                                .gap_1()
                                .child(
                                    Label::new("I'm feeling...")
                                        .text_xs()
                                        .text_color(theme.muted_foreground),
                                )
                                .child(
                                    div()
                                        .id("mental-state-scroll")
                                        .overflow_y_scroll()
                                        .max_h(px(120.0))
                                        .w_full()
                                        .child(
                                            v_flex().w_full().gap_1().children(
                                                self.saved_mental_states
                                                    .iter()
                                                    .map(|state| {
                                                        let state_id = state.id;
                                                        let name = state.name.clone();
                                                        let is_active = self
                                                            .current_mental_state_name
                                                            .as_deref()
                                                            == Some(&state.name);

                                                        Button::new(gpui::SharedString::from(
                                                            format!("declare-{}", state_id),
                                                        ))
                                                        .label(name)
                                                        .xsmall()
                                                        .w_full()
                                                        .map(|b| {
                                                            if is_active {
                                                                b.primary()
                                                            } else {
                                                                b.ghost()
                                                            }
                                                        })
                                                        .on_click(cx.listener(
                                                            move |this, _event, _window, cx| {
                                                                this.database_store.update(
                                                                    cx,
                                                                    |store, cx| {
                                                                        store.declare_mental_state(
                                                                            state_id, cx,
                                                                        );
                                                                    },
                                                                );
                                                            },
                                                        ))
                                                    })
                                                    .collect::<Vec<_>>(),
                                            ),
                                        ),
                                ),
                        ),
                )
            })
    }

    fn render_pipeline_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .px_3()
            .items_center()
            .justify_between()
            .child(Label::new("Pipeline").text_lg().font(font("Georgia")))
            .child(
                h_flex().gap(px(2.0)).child(
                    Button::new("refresh-pipeline")
                        .icon(IconName::Redo)
                        .ghost()
                        .xsmall()
                        .tooltip("Re-score and reorder by priority")
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.database_store.update(cx, |store, cx| {
                                store.refresh_pipeline(cx);
                            });
                        })),
                ),
            )
    }
}

impl EventEmitter<()> for RightSidebarView {}

impl Render for RightSidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_2()
            .pl_1()
            .bg(cx.theme().secondary)
            .child(
                div()
                    .size_full()
                    .overflow_hidden()
                    .bg(cx.theme().background)
                    .rounded_lg()
                    .child(
                        v_flex()
                            .size_full()
                            .pt_4()
                            .gap_3()
                            .items_center()
                            .child(self.render_mental_state_section(cx))
                            .child(
                                div().w_full().px_3().child(
                                    gpui_component::divider::Divider::horizontal()
                                        .color(cx.theme().border),
                                ),
                            )
                            .child(self.render_pipeline_header(cx))
                            .child(self.pipeline.clone()),
                    ),
            )
    }
}
