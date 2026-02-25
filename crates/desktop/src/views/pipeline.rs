use app_core::PipelineEntry;
use gpui::{
    Context, ElementId, Entity, EventEmitter, IntoElement, Pixels, Point, Render, Window, div,
    hsla, px,
};
use gpui::{DragMoveEvent, InteractiveElement, prelude::*};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::label::Label;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{ActiveTheme, IconName, Sizable, StyledExt, h_flex, v_flex};
use uuid::Uuid;

use crate::components::checkbox::Checkbox;
use crate::components::drag_drop::{DragData, DropIndicator, DropPosition, DropZone};
use crate::stores::DatabaseStore;
use crate::stores::database_store::PipelineChanged;
use crate::stores::drag_drop_store::{ActionLocation, DragDropStore};
use crate::views::StartActionEditor;

pub struct Pipeline {
    database_store: Entity<DatabaseStore>,
    entries: Vec<PipelineEntry>,
    drag_drop_store: Entity<DragDropStore>,
    drag_active_here: bool,
    item_height: Pixels,
    gap: Pixels,
}

impl Pipeline {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        drag_drop_store: Entity<DragDropStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let entries = database_store
            .read(cx)
            .get_pipeline()
            .queue()
            .iter()
            .filter(|e| !e.is_transition())
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
                    .cloned()
                    .collect();
                cx.notify();
            },
        )
        .detach();

        Self {
            database_store,
            entries,
            drag_drop_store,
            drag_active_here: false,
            item_height: px(80.0),
            gap: px(12.0),
        }
    }

    pub fn update_items(&mut self, cx: &mut Context<Self>) {
        self.entries = self
            .database_store
            .read(cx)
            .get_pipeline()
            .queue()
            .iter()
            .filter(|e| !e.is_transition())
            .cloned()
            .collect();
    }

    fn calculate_drop_index(&self, position: Point<Pixels>, bounds: gpui::Bounds<Pixels>) -> usize {
        let item_count = self.entries.len();
        if item_count == 0 {
            return 0;
        }
        let interval = self.item_height + self.gap;
        let relative_y = (position.y - bounds.origin.y).clamp(px(0.0), bounds.size.height);
        (relative_y / interval).floor() as usize
    }

    fn handle_drag_move(
        &mut self,
        event: &DragMoveEvent<DragData<app_core::SavedAction>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = event.bounds;
        let position = event.event.position;
        let data = event.drag(cx).clone();
        let action_id = data.data.id.to_string();

        if !self.drag_drop_store.read(cx).is_dragging() {
            self.drag_drop_store.update(cx, |store, cx| {
                store.new_drag(action_id.clone(), cx);
            });
        }

        if bounds.contains(&window.mouse_position()) {
            let drop_index = self.calculate_drop_index(position, bounds);
            self.drag_drop_store.update(cx, |store, cx| {
                store.set_drop_target(Some(ActionLocation::Pipeline(drop_index)), cx);
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

    fn score_color(score: f32) -> gpui::Hsla {
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

        // Compute scores for all entries upfront.
        let scores: Vec<f32> = self
            .entries
            .iter()
            .map(|entry| self.database_store.read(cx).score_entry(entry))
            .collect();

        div().size_full().overflow_y_scrollbar().child(
            DropZone::<DragData<app_core::SavedAction>>::new("pipeline-drop-zone")
                .active(self.drag_active_here)
                .size_full()
                .insertion_indicator(target_index.map(|index| DropIndicator {
                    index,
                    position: DropPosition::Before,
                }))
                .on_drop(cx.listener(
                    move |this, data: &DragData<app_core::SavedAction>, _window, cx| {
                        if let Some(_index) = target_index {
                            // When an action is dragged from the action list onto the pipeline,
                            // create a concrete action entry in the pipeline from the saved action.
                            let saved_action = data.data.clone();
                            this.database_store.update(cx, |store, cx| {
                                store.create_action(saved_action.title.clone(), cx);
                            });
                        }
                        this.drag_drop_store.update(cx, |store, cx| {
                            store.clear_drag(cx);
                        });
                        this.drag_active_here = false;
                    },
                ))
                .on_drag_move(cx.listener(
                    move |this,
                          event: &DragMoveEvent<DragData<app_core::SavedAction>>,
                          window,
                          cx| {
                        this.handle_drag_move(event, window, cx);
                    },
                ))
                .when(!self.entries.is_empty(), |this| {
                    this.children(
                        self.entries
                            .iter()
                            .enumerate()
                            .map(|(i, entry)| {
                                let entry_id = entry.id();
                                let title = entry.title().to_string();
                                let entry_id_complete = entry_id;
                                let entry_id_demote = entry_id;
                                let entry_id_remove = entry_id;
                                let score = scores.get(i).copied().unwrap_or(0.0);
                                let score_display = format!("{:.0}%", score * 100.0);
                                let score_color = Self::score_color(score);

                                // Only Action entries support editing a SavedAction.
                                let saved_action_id = if let PipelineEntry::Action(a) = entry {
                                    a.saved_action_id
                                } else {
                                    None
                                };

                                div()
                                    .id(ElementId::NamedInteger("pipeline-item".into(), i as u64))
                                    .h_flex()
                                    .hover(|s| s.bg(theme.list_hover.opacity(0.3)))
                                    .w_full()
                                    .h(self.item_height)
                                    .p_2()
                                    .bg(theme.background)
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme.border)
                                    .gap_2()
                                    .items_center()
                                    .on_click({
                                        cx.listener(move |pipeline, _event, _window, cx| {
                                            if let Some(id) = saved_action_id {
                                                cx.emit(StartActionEditor {
                                                    action_id: Some(id),
                                                });
                                            }
                                        })
                                    })
                                    .child(
                                        Checkbox::new(ElementId::Name(
                                            format!("pipeline-checkbox-{}", entry_id).into(),
                                        ))
                                        .large()
                                        .occlude()
                                        .on_mouse_up(
                                            cx.listener(
                                                move |this, checked: &bool, _window, cx| {
                                                    if *checked {
                                                        this.database_store.update(
                                                            cx,
                                                            |store, cx| {
                                                                store.complete_action(
                                                                    entry_id_complete,
                                                                    cx,
                                                                );
                                                            },
                                                        );
                                                    }
                                                },
                                            ),
                                        ),
                                    )
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap(px(2.0))
                                            .child(Label::new(title).text_sm().truncate())
                                            .child(
                                                h_flex()
                                                    .gap_1()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .w(px(32.0))
                                                            .h(px(3.0))
                                                            .rounded(px(2.0))
                                                            .bg(theme.muted_foreground.opacity(0.2))
                                                            .child(
                                                                div()
                                                                    .h_full()
                                                                    .w(px(32.0 * score))
                                                                    .rounded(px(2.0))
                                                                    .bg(score_color),
                                                            ),
                                                    )
                                                    .child(
                                                        Label::new(score_display)
                                                            .text_xs()
                                                            .text_color(theme.muted_foreground),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap(px(1.0))
                                            .flex_shrink_0()
                                            .opacity(0.4)
                                            .hover(|s| s.opacity(1.0))
                                            .child(
                                                Button::new(ElementId::Name(
                                                    format!("demote-{}", i).into(),
                                                ))
                                                .icon(IconName::ChevronRight)
                                                .ghost()
                                                .xsmall()
                                                .tooltip("Move to backlog")
                                                .on_click(cx.listener(
                                                    move |this, _event, _window, cx| {
                                                        this.database_store.update(
                                                            cx,
                                                            |store, cx| {
                                                                store.demote(entry_id_demote, cx);
                                                            },
                                                        );
                                                    },
                                                )),
                                            )
                                            .child(
                                                Button::new(ElementId::Name(
                                                    format!("remove-{}", i).into(),
                                                ))
                                                .icon(IconName::Close)
                                                .ghost()
                                                .xsmall()
                                                .tooltip("Remove from pipeline")
                                                .on_click(cx.listener(
                                                    move |this, _event, _window, cx| {
                                                        this.database_store.update(
                                                            cx,
                                                            |store, cx| {
                                                                store.remove_from_pipeline(
                                                                    entry_id_remove,
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
                .when(self.entries.is_empty(), |this| {
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
                                    .child("Nothing in the queue — add actions and promote them"),
                            ),
                    )
                }),
        )
    }
}
