use gpui::{
    Bounds, BoxShadow, Context, ElementId, Entity, EventEmitter, FontWeight, IntoElement, Pixels,
    Point, Render, Subscription, Window, div, font, hsla, point, px,
};
use gpui::{DragMoveEvent, prelude::*};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::label::Label;
use gpui_component::notification::NotificationType;
use gpui_component::scroll::ScrollableElement;
use gpui_component::slider::{Slider, SliderEvent, SliderState};
use gpui_component::{ActiveTheme, IconName, Sizable, StyledExt, WindowExt, h_flex, v_flex};
use std::collections::HashSet;

use crate::components::checkbox::Checkbox;
use crate::components::drag_drop::{DragData, Draggable, DropIndicator, DropPosition, DropZone};
use crate::stores::DatabaseStore;
use crate::stores::database_store::{
    ActionsLoaded, ContextLoaded, MentalStatesLoaded, PipelineLoaded, PipelineScored,
    SuggestionsLoaded,
};
use crate::stores::drag_drop_store::{ActionLocation, DragDropStore};
use crate::views::StartActionEditor;
use database::{Action, Instance, PipelineItem};

pub struct Pipeline {
    database_store: Entity<DatabaseStore>,
    items: Vec<(PipelineItem, Instance)>,
    drag_drop_store: Entity<DragDropStore>,
    drag_active_here: bool,
    item_height: Pixels,
    gap: Pixels,
    pending_drops: Vec<(String, i64)>,
    in_progress_deletes: HashSet<String>,
    processing_drop: bool,
    scores: Vec<(String, f64)>,
    hovered_item: Option<String>,
}

impl Pipeline {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        drag_drop_store: Entity<DragDropStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe(
            &database_store,
            |this, _store, _event: &ActionsLoaded, cx| {
                this.update_items(cx);
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(
            &database_store,
            |this, _store, _event: &PipelineLoaded, cx| {
                this.update_items(cx);
                if this.processing_drop {
                    this.processing_drop = false;
                    if !this.pending_drops.is_empty() {
                        this.process_next_drop(cx);
                    }
                }
                // Auto-score when pipeline changes
                this.trigger_scoring(cx);
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(
            &database_store,
            |this, store, _event: &PipelineScored, cx| {
                this.scores = store.read(cx).get_pipeline_scores().clone();
                cx.notify();
            },
        )
        .detach();

        cx.subscribe(
            &database_store,
            |this, _store, _event: &ContextLoaded, cx| {
                this.trigger_scoring(cx);
            },
        )
        .detach();

        cx.subscribe(
            &database_store,
            |this, _store, _event: &MentalStatesLoaded, cx| {
                this.trigger_scoring(cx);
            },
        )
        .detach();

        Self {
            database_store,
            items: vec![],
            drag_drop_store,
            drag_active_here: false,
            item_height: px(80.0),
            gap: px(12.0),
            pending_drops: vec![],
            in_progress_deletes: HashSet::new(),
            processing_drop: false,
            scores: vec![],
            hovered_item: None,
        }
    }

    fn trigger_scoring(&self, cx: &mut Context<Self>) {
        if self.items.is_empty() {
            return;
        }
        self.database_store.update(cx, |store, cx| {
            store.score_pipeline(cx);
        });
    }

    pub fn update_items(&mut self, cx: &mut Context<Self>) {
        let items = self.database_store.read(cx).get_pipeline_items().clone();

        self.items = items
            .into_iter()
            .filter_map(|item| {
                item.instance_id
                    .clone()
                    .map(|id| {
                        self.database_store
                            .read(cx)
                            .get_instance(&id)
                            .map(|instance| (item, instance.clone()))
                    })
                    .flatten()
            })
            .collect();
        self.in_progress_deletes
            .retain(|id| self.items.iter().any(|(_, instance)| &instance.id == id));
    }

    fn process_next_drop(&mut self, cx: &mut Context<Self>) {
        if self.processing_drop || self.pending_drops.is_empty() {
            return;
        }

        let (id, position) = self.pending_drops.remove(0);
        self.processing_drop = true;

        self.drag_drop_store.update(cx, |store, cx| {
            store.clear_drag(cx);
        });

        self.database_store.update(cx, |store, cx| {
            store.insert_instance_at_position(id, position, cx);
        });

        self.drag_active_here = false;
    }

    fn calculate_drop_index(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> usize {
        let item_count = self.items.len();
        if item_count == 0 {
            return 0;
        }

        let interval = self.item_height + self.gap;

        let relative_y = (position.y - bounds.origin.y).clamp(px(0.0), bounds.size.height);
        let item_index = (relative_y / interval).floor() as usize;
        item_index
    }

    fn handle_drag_move(
        &mut self,
        event: &DragMoveEvent<DragData<Action>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = event.bounds;
        let position = event.event.position;
        let data = event.drag(cx).clone();
        let item = data.data;
        let action_id = item.id.clone();
        if self.drag_drop_store.read(cx).is_dragging() == false {
            self.drag_drop_store.update(cx, |store, cx| {
                store.new_drag(action_id.clone(), cx);
            });
        }
        if bounds.contains(&window.mouse_position()) {
            let drop_index = self.calculate_drop_index(position, bounds, window, cx);
            self.drag_drop_store.update(cx, |store, cx| {
                let location = Some(ActionLocation::Pipeline(drop_index));
                store.set_drop_target(location, cx);
            });
            self.drag_active_here = true;
        } else if self.drag_active_here {
            self.drag_drop_store.update(cx, |store, cx| {
                if let Some(ActionLocation::Pipeline(_)) = store.get_drop_target() {
                    store.clear_drop_target(cx);
                }
            });
            self.drag_active_here = false;
        }
    }

    fn get_score_for_item(&self, pipeline_item_id: &str) -> Option<f64> {
        self.scores
            .iter()
            .find(|(id, _)| id == pipeline_item_id)
            .map(|(_, score)| *score)
    }

    fn score_color(score: f64) -> gpui::Hsla {
        let hue = (score.clamp(0.0, 1.0) * 120.0) as f32;
        hsla(hue / 360.0, 0.6, 0.45, 1.0)
    }
}

impl EventEmitter<StartActionEditor> for Pipeline {}

impl Render for Pipeline {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let target_index =
            self.drag_drop_store
                .read(cx)
                .get_drop_target()
                .and_then(|loc| match loc {
                    ActionLocation::Pipeline(ix) => Some(*ix),
                    _ => None,
                });

        div().size_full().overflow_y_scrollbar().child(
            DropZone::<DragData<Action>>::new("pipeline-drop-zone")
                .active(self.drag_active_here)
                .size_full()
                .insertion_indicator(target_index.map(|index| DropIndicator {
                    index,
                    position: DropPosition::Before,
                }))
                .on_drop(
                    cx.listener(move |this, data: &DragData<Action>, _window, cx| {
                        if let Some(index) = target_index {
                            let id = data.data.id.clone();
                            let position = (index as i64) + 1;

                            let is_duplicate = this
                                .pending_drops
                                .iter()
                                .any(|(pid, ppos)| pid == &id && *ppos == position);

                            if is_duplicate {
                                return;
                            }

                            if this.processing_drop {
                                this.pending_drops.push((id, position));
                            } else {
                                this.pending_drops.push((id.clone(), position));
                                this.process_next_drop(cx);
                            }
                        }
                    }),
                )
                .on_drag_move(cx.listener(
                    move |this, event: &gpui::DragMoveEvent<DragData<Action>>, window, cx| {
                        this.handle_drag_move(event, window, cx);
                    },
                ))
                .when(!self.items.is_empty(), |this| {
                    this.children(
                        self.items
                            .iter()
                            .enumerate()
                            .map(|(i, (item, instance))| {
                                let title =
                                    item.action_title.clone().unwrap_or("Untitled".to_string());
                                let instance_id = instance.id.clone();
                                let instance_id_skip = instance.id.clone();
                                let instance_id_snooze = instance.id.clone();
                                let instance_id_abandon = instance.id.clone();
                                let completed = &instance.status == "completed";
                                let opacity = 1.0;
                                let pipeline_item_id = item.id.clone();
                                let score = self.get_score_for_item(&pipeline_item_id);

                                let drag_title = title.clone();

                                let theme_clone = theme.clone();
                                let drag_data =
                                    DragData::new(item.clone()).with_preview(move || {
                                        div()
                                            .px(px(12.0))
                                            .py(px(8.0))
                                            .bg(theme_clone.popover.opacity(0.95))
                                            .border_1()
                                            .border_color(theme_clone.border)
                                            .rounded(px(6.0))
                                            .shadow(vec![BoxShadow {
                                                color: hsla(0.0, 0.0, 0.0, 0.25),
                                                offset: point(px(0.0), px(4.0)),
                                                blur_radius: px(12.0),
                                                spread_radius: px(0.0),
                                            }])
                                            .text_size(px(13.0))
                                            .text_color(theme_clone.foreground)
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(format!("Moving: {}", drag_title))
                                            .into_any_element()
                                    });

                                Draggable::new(("pipeline-item", i), drag_data)
                                    .h_flex()
                                    .hover_bg(theme.list_hover.opacity(0.3))
                                    .w_full()
                                    .h(self.item_height)
                                    .p_2()
                                    .bg(theme.background)
                                    .opacity(opacity)
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme.border)
                                    .gap_2()
                                    .items_center()
                                    .on_click({
                                        let action_id = instance.action_id.clone();
                                        cx.listener(move |pipeline, _event, _window, cx| {
                                            let event = StartActionEditor {
                                                action_id: Some(action_id.clone()),
                                            };
                                            cx.emit(event);
                                        })
                                    })
                                    // Checkbox
                                    .child(
                                        Checkbox::new(ElementId::Name(
                                            format!("pipeline-checkbox-{}", instance_id).into(),
                                        ))
                                        .checked(completed)
                                        .large()
                                        .occlude()
                                        .on_mouse_down(cx.listener(
                                            move |_this, _checked: &bool, _window, _cx| {},
                                        ))
                                        .on_mouse_up(
                                            cx.listener(
                                                move |this, checked: &bool, _window, cx| {
                                                    if *checked {
                                                        if this
                                                            .in_progress_deletes
                                                            .contains(&instance_id)
                                                        {
                                                            return;
                                                        }

                                                        this.in_progress_deletes
                                                            .insert(instance_id.clone());

                                                        this.database_store.update(
                                                            cx,
                                                            |store, cx| {
                                                                store.complete_with_event(
                                                                    instance_id.clone(),
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                    } else {
                                                        this.database_store.update(
                                                            cx,
                                                            |store, cx| {
                                                                store.uncomplete_pipeline_item(
                                                                    instance_id.clone(),
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                    }
                                                },
                                            ),
                                        ),
                                    )
                                    // Title and score in a column
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap(px(2.0))
                                            .child(Label::new(title).text_sm().truncate())
                                            .when_some(score, |this, score_value| {
                                                let score_display =
                                                    format!("{:.0}%", score_value * 100.0);
                                                let score_color = Self::score_color(score_value);
                                                this.child(
                                                    h_flex()
                                                        .gap_1()
                                                        .items_center()
                                                        .child(
                                                            div()
                                                                .w(px(32.0))
                                                                .h(px(3.0))
                                                                .rounded(px(2.0))
                                                                .bg(theme
                                                                    .muted_foreground
                                                                    .opacity(0.2))
                                                                .child(
                                                                    div()
                                                                        .h_full()
                                                                        .w(px(32.0
                                                                            * score_value as f32))
                                                                        .rounded(px(2.0))
                                                                        .bg(score_color),
                                                                ),
                                                        )
                                                        .child(
                                                            Label::new(score_display)
                                                                .text_xs()
                                                                .text_color(theme.muted_foreground),
                                                        ),
                                                )
                                            }),
                                    )
                                    // Action buttons (visible on hover via group)
                                    .child(
                                        h_flex()
                                            .gap(px(1.0))
                                            .flex_shrink_0()
                                            .opacity(0.4)
                                            .hover(|s| s.opacity(1.0))
                                            .child(
                                                Button::new(ElementId::Name(
                                                    format!("skip-{}", i).into(),
                                                ))
                                                .icon(IconName::ChevronRight)
                                                .ghost()
                                                .xsmall()
                                                .tooltip("Skip — not now, maybe later")
                                                .on_click(cx.listener(
                                                    move |this, _event, _window, cx| {
                                                        this.database_store.update(
                                                            cx,
                                                            |store, cx| {
                                                                store.skip_instance(
                                                                    instance_id_skip.clone(),
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                    },
                                                )),
                                            )
                                            .child(
                                                Button::new(ElementId::Name(
                                                    format!("snooze-{}", i).into(),
                                                ))
                                                .icon(IconName::Pause)
                                                .ghost()
                                                .xsmall()
                                                .tooltip("Snooze — remind me later")
                                                .on_click(cx.listener(
                                                    move |this, _event, _window, cx| {
                                                        this.database_store.update(
                                                            cx,
                                                            |store, cx| {
                                                                store.snooze_instance(
                                                                    instance_id_snooze.clone(),
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                    },
                                                )),
                                            )
                                            .child(
                                                Button::new(ElementId::Name(
                                                    format!("abandon-{}", i).into(),
                                                ))
                                                .icon(IconName::Close)
                                                .ghost()
                                                .xsmall()
                                                .tooltip("Abandon — I'm not doing this")
                                                .on_click(cx.listener(
                                                    move |this, _event, _window, cx| {
                                                        this.database_store.update(
                                                            cx,
                                                            |store, cx| {
                                                                store.abandon_instance(
                                                                    instance_id_abandon.clone(),
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                    },
                                                )),
                                            ),
                                    )
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .when(self.items.is_empty(), |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(8.0))
                            .py(px(32.0))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .text_color(theme.muted_foreground)
                                    .child("Drop actions here to add to pipeline"),
                            ),
                    )
                }),
        )
    }
}

pub struct RightSidebarView {
    collapsed: bool,
    pub pipeline: Entity<Pipeline>,
    database_store: Entity<DatabaseStore>,
    energy_slider: Entity<SliderState>,
    attention_slider: Entity<SliderState>,
    energy_value: f32,
    attention_value: f32,
    current_mental_state_name: Option<String>,
    context_expanded: bool,
    _subscriptions: Vec<Subscription>,
}

impl RightSidebarView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        drag_drop_store: Entity<DragDropStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, _store, _event: &ActionsLoaded, cx| {
                this.update_pipeline(cx);
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, _store, _event: &PipelineLoaded, cx| {
                this.update_pipeline(cx);
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe_in(
            &database_store,
            window,
            |this, store, _event: &ContextLoaded, window, cx| {
                let energy = store.read(cx).get_context_energy().unwrap_or(3.0);
                let attention = store.read(cx).get_context_attention().unwrap_or(3.0);
                this.energy_value = energy as f32;
                this.attention_value = attention as f32;
                this.energy_slider.update(cx, |slider, cx| {
                    slider.set_value(energy as f32, window, cx);
                });
                this.attention_slider.update(cx, |slider, cx| {
                    slider.set_value(attention as f32, window, cx);
                });
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe(
            &database_store,
            |this, store, _event: &MentalStatesLoaded, cx| {
                this.current_mental_state_name = store
                    .read(cx)
                    .get_current_mental_state()
                    .map(|state| state.name.clone());
                cx.notify();
            },
        ));

        subscriptions.push(cx.subscribe_in(
            &database_store,
            window,
            |_this, store, _event: &SuggestionsLoaded, window, cx| {
                let suggestions = store.read(cx).get_suggestions();
                if suggestions.is_empty() {
                    window.push_notification(
                        (
                            NotificationType::Info,
                            "No suggestions available — add some actions to the pipeline first.",
                        ),
                        cx,
                    );
                    return;
                }

                let mut message = String::new();
                for (i, (_instance, action, score)) in suggestions.iter().enumerate() {
                    if i > 0 {
                        message.push('\n');
                    }
                    message.push_str(&format!(
                        "{}. {} — {:.0}%",
                        i + 1,
                        action.title,
                        score * 100.0
                    ));
                }

                window.push_notification(
                    gpui_component::notification::Notification::new()
                        .title("What should I do next?")
                        .message(message)
                        .with_type(NotificationType::Success),
                    cx,
                );
            },
        ));

        let energy_slider = cx.new(|_cx| {
            SliderState::new()
                .min(1.0)
                .max(5.0)
                .step(1.0)
                .default_value(3.0)
        });

        let attention_slider = cx.new(|_cx| {
            SliderState::new()
                .min(1.0)
                .max(5.0)
                .step(1.0)
                .default_value(3.0)
        });

        {
            let db = database_store.clone();
            subscriptions.push(cx.subscribe(
                &energy_slider,
                move |this, _slider, event: &SliderEvent, cx| match event {
                    SliderEvent::Change(value) => {
                        this.energy_value = value.start();
                        db.update(cx, |store, cx| {
                            store.update_energy(this.energy_value as f64, cx);
                        });
                    }
                },
            ));
        }

        {
            let db = database_store.clone();
            subscriptions.push(cx.subscribe(
                &attention_slider,
                move |this, _slider, event: &SliderEvent, cx| match event {
                    SliderEvent::Change(value) => {
                        this.attention_value = value.start();
                        db.update(cx, |store, cx| {
                            store.update_attention(this.attention_value as f64, cx);
                        });
                    }
                },
            ));
        }

        let pipeline_list = cx.new(|cx| Pipeline::new(database_store.clone(), drag_drop_store, cx));

        // Load initial context and mental state data
        database_store.update(cx, |store, cx| {
            store.load_current_context(cx);
            store.load_current_mental_state(cx);
        });

        let energy = database_store.read(cx).get_context_energy().unwrap_or(3.0);
        let attention = database_store
            .read(cx)
            .get_context_attention()
            .unwrap_or(3.0);
        let current_mental_state_name = database_store
            .read(cx)
            .get_current_mental_state()
            .map(|state| state.name.clone());

        Self {
            collapsed: false,
            pipeline: pipeline_list,
            database_store,
            energy_slider,
            attention_slider,
            energy_value: energy as f32,
            attention_value: attention as f32,
            current_mental_state_name,
            context_expanded: true,
            _subscriptions: subscriptions,
        }
    }

    fn update_pipeline(&mut self, cx: &mut Context<Self>) {
        self.pipeline.update(cx, |pipeline, cx| {
            pipeline.update_items(cx);
            cx.notify();
        });
    }

    pub fn toggle_collapsed(&mut self, cx: &mut Context<Self>) -> bool {
        self.collapsed = !self.collapsed;
        cx.notify();
        self.collapsed
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn energy_label(value: f32) -> &'static str {
        match value.round() as i32 {
            1 => "Exhausted",
            2 => "Low",
            3 => "Moderate",
            4 => "High",
            5 => "Energized",
            _ => "Moderate",
        }
    }

    fn attention_label(value: f32) -> &'static str {
        match value.round() as i32 {
            1 => "Scattered",
            2 => "Distracted",
            3 => "Moderate",
            4 => "Focused",
            5 => "Locked In",
            _ => "Moderate",
        }
    }

    fn render_context_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let energy_label = Self::energy_label(self.energy_value);
        let attention_label = Self::attention_label(self.attention_value);

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
                        Label::new("Context")
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
                        .gap_3()
                        .child(
                            v_flex()
                                .w_full()
                                .gap_1()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .justify_between()
                                        .child(Label::new("Energy").text_xs())
                                        .child(
                                            Label::new(energy_label)
                                                .text_xs()
                                                .text_color(theme.muted_foreground),
                                        ),
                                )
                                .child(Slider::new(&self.energy_slider)),
                        )
                        .child(
                            v_flex()
                                .w_full()
                                .gap_1()
                                .child(
                                    h_flex()
                                        .w_full()
                                        .justify_between()
                                        .child(Label::new("Attention").text_xs())
                                        .child(
                                            Label::new(attention_label)
                                                .text_xs()
                                                .text_color(theme.muted_foreground),
                                        ),
                                )
                                .child(Slider::new(&self.attention_slider)),
                        )
                        .when_some(self.current_mental_state_name.clone(), |this, name| {
                            this.child(
                                h_flex()
                                    .w_full()
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
                                    ),
                            )
                        }),
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
                h_flex()
                    .gap(px(2.0))
                    .child(
                        Button::new("suggest-next")
                            .icon(IconName::Star)
                            .ghost()
                            .xsmall()
                            .tooltip("What should I do next?")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.database_store.update(cx, |store, cx| {
                                    store.suggest_next(3, cx);
                                });
                            })),
                    )
                    .child(
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
                            .child(self.render_context_section(cx))
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
