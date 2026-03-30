use std::time::Duration;

use chrono::{DateTime, Local, Timelike, Utc};
use gpui::{
    AnchoredPositionMode, App, Context, Div, Entity, EventEmitter, Hsla, InteractiveElement,
    IntoElement, Pixels, Render, ScrollHandle, Styled, Window, actions, div, px,
};
use gpui::{Stateful, prelude::*};
use gpui_component::label::Label;
use gpui_component::menu::{ContextMenu, ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_component::scroll::Scrollbar;
use gpui_component::{ActiveTheme, Icon, IconName, StyledExt, h_flex, v_flex};
use gpui_transitions::WindowUseTransition;
use simple_core::{Action, Event, QueueItem};

use crate::components::checkbox::Checkbox;
use crate::stores::DatabaseStore;
use crate::stores::database_store::PipelineChanged;
use crate::views::action_editor::StartActionEditor;
use crate::views::event_editor::StartEventEditor;

const ITEM_MIN_HEIGHT: f32 = 60.0;

pub struct StartQueueEventEditor {
    pub event_id: uuid::Uuid,
}

actions!(
    pipeline,
    [CompleteAction, DemoteAction, RemoveFromPipeline,]
);

pub struct Pipeline {
    database_store: Entity<DatabaseStore>,
    entries: Vec<QueueItem>,
    scroll_handle: ScrollHandle,
}

impl Pipeline {
    pub fn new(database_store: Entity<DatabaseStore>, cx: &mut Context<Self>) -> Self {
        let entries = database_store.read(cx).pipeline.queue.clone();

        cx.subscribe(
            &database_store,
            |this, store, _event: &PipelineChanged, cx| {
                this.entries = store.read(cx).pipeline.queue.clone();
                cx.notify();
            },
        )
        .detach();

        Self {
            database_store,
            entries,
            scroll_handle: ScrollHandle::default(),
        }
    }

    pub fn update_items(&mut self, cx: &mut Context<Self>) {
        self.entries = self.database_store.read(cx).pipeline.queue.clone();
    }

    fn item_base(&self, height: Option<Pixels>, color: Hsla, cx: &Context<Self>) -> Div {
        let theme = cx.theme().clone();
        h_flex()
            .hover(|s| s.bg(colors.hover))
            .w_full()
            .when_some(height, |div, h| div.h(h))
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(color.alpha(0.7))
            .gap_2()
            .items_center()
    }

    fn item_content(
        &self,
        title: String,
        target_label: Option<String>,
        duration: Option<chrono::Duration>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        v_flex()
            .w_full()
            .items_end()
            .gap_1()
            .child(Label::new(title).text_sm().truncate())
            .child(
                h_flex()
                    .gap_2()
                    .when_some(target_label, |this, label| {
                        this.child(
                            Label::new(label)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .truncate(),
                        )
                    })
                    .when_some(duration, |div, duration| {
                        let duration_label = format_duration(&duration);
                        div.child(
                            Label::new(duration_label)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .truncate(),
                        )
                    }),
            )
    }

    fn action_context_menu(
        &self,
        action_id: uuid::Uuid,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let entity = cx.entity();
        move |menu, _window, _cx| {
            let entity_complete = entity.clone();
            let entity_demote = entity.clone();
            let entity_delete = entity.clone();
            menu.item(
                PopupMenuItem::new("Complete")
                    .icon(IconName::CircleCheck)
                    .on_click(move |_event, _window, cx: &mut App| {
                        entity_complete.update(cx, |this, cx| {
                            this.database_store.update(cx, |store, cx| {
                                store.complete_action(action_id, cx);
                            });
                        });
                    }),
            )
            .item(
                PopupMenuItem::new("Demote to backlog")
                    .icon(IconName::ChevronDown)
                    .on_click(move |_event, _window, cx: &mut App| {
                        entity_demote.update(cx, |this, cx| {
                            this.database_store.update(cx, |store, cx| {
                                store.demote_action(action_id, cx);
                            });
                        });
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new("Delete")
                    .icon(IconName::Delete)
                    .on_click(move |_event, _window, cx: &mut App| {
                        entity_delete.update(cx, |this, cx| {
                            this.database_store.update(cx, |store, cx| {
                                store.delete_queue_action(action_id, cx);
                            });
                        });
                    }),
            )
        }
    }

    fn event_context_menu(
        &self,
        event_id: uuid::Uuid,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let entity = cx.entity();
        move |menu, _window, _cx| {
            let entity_delete = entity.clone();
            menu.item(
                PopupMenuItem::new("Delete")
                    .icon(IconName::Delete)
                    .on_click(move |_event, _window, cx: &mut App| {
                        entity_delete.update(cx, |this, cx| {
                            this.database_store.update(cx, |store, cx| {
                                store.remove_from_pipeline(event_id, cx);
                            });
                        });
                    }),
            )
        }
    }

    fn render_action(
        &self,
        action: &Action,
        ix: usize,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> ContextMenu<Stateful<Div>> {
        let action_id = action.id;
        let title = action.title.clone();
        let target_label = action.target.map(format_target_time);
        let theme = cx.theme().clone();

        let item_height = if let Some(duration) = action.duration.as_ref() {
            let mins = duration.num_minutes().max(1);
            (mins as f32 * 4.).min(320.0).max(ITEM_MIN_HEIGHT)
        } else {
            ITEM_MIN_HEIGHT
        };

        self.item_base(Some(px(item_height)), theme.green, cx)
            .id(("pipeline-action", ix as u64))
            .on_click(cx.listener(move |_this, _event, _window, cx| {
                cx.emit(StartActionEditor {
                    action_id: Some(action_id),
                });
            }))
            .context_menu(self.action_context_menu(action_id, cx))
            .child(
                Checkbox::new(("pipeline-check", ix as u64))
                    .checked(false)
                    .occlude()
                    .on_click(cx.listener(move |this, _checked, _window, cx| {

                        // this.database_store.update(cx, |store, cx| {
                        //     store.complete_action(action_id, cx);
                        // });
                    })),
            )
            .child(self.item_content(title, target_label, action.duration, cx))
    }

    fn render_event(
        &self,
        event: &Event,
        ix: usize,
        cx: &Context<Self>,
    ) -> ContextMenu<Stateful<Div>> {
        let event_id = event.id;
        let title = event.title.clone();
        let time_label = format_target_time(event.time);
        let theme = cx.theme().clone();

        let item_height = if let Some(duration) = event.duration.as_ref() {
            let mins = duration.num_minutes().max(1);
            (mins as f32 * 4.).min(320.0).max(ITEM_MIN_HEIGHT)
        } else {
            ITEM_MIN_HEIGHT
        };

        self.item_base(Some(px(item_height)), theme.blue, cx)
            .id(("pipeline-event", ix as u64))
            .on_click(cx.listener(move |_this, _event, _window, cx| {
                cx.emit(StartQueueEventEditor { event_id });
            }))
            .context_menu(self.event_context_menu(event_id, cx))
            .child(Icon::new(IconName::Calendar).opacity(0.5))
            .child(self.item_content(title, Some(time_label), event.duration, cx))
    }
}

fn format_target_time(time: DateTime<Utc>) -> String {
    let local = time.with_timezone(&Local);
    let now = Local::now();
    let is_today = local.date_naive() == now.date_naive();
    let is_tomorrow = local.date_naive() == (now + chrono::Duration::days(1)).date_naive();

    let time_str = if local.minute() == 0 {
        format!(
            "{}{}",
            if local.hour12().1 == 0 {
                12
            } else {
                local.hour12().1
            },
            if local.hour12().0 { "pm" } else { "am" }
        )
    } else {
        format!(
            "{}:{:02}{}",
            if local.hour12().1 == 0 {
                12
            } else {
                local.hour12().1
            },
            local.minute(),
            if local.hour12().0 { "pm" } else { "am" }
        )
    };

    if is_today {
        format!("Today {}", time_str)
    } else if is_tomorrow {
        format!("Tomorrow {}", time_str)
    } else {
        format!("{} {}", local.format("%b %-d"), time_str)
    }
}

fn format_duration(duration: &chrono::Duration) -> String {
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

impl EventEmitter<StartActionEditor> for Pipeline {}
impl EventEmitter<StartEventEditor> for Pipeline {}
impl EventEmitter<StartQueueEventEditor> for Pipeline {}

impl Render for Pipeline {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        if self.entries.is_empty() {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .py_8()
                .gap_2()
                .child(
                    Label::new("Queue is empty")
                        .text_sm()
                        .text_color(theme.muted_foreground),
                )
                .into_any_element();
        }

        let scroll_handle = self.scroll_handle.clone();

        div()
            .relative()
            .w_full()
            .child(
                div()
                    .id("pipeline-items")
                    .overflow_y_scroll()
                    .track_scroll(&scroll_handle)
                    .w_full()
                    .child(v_flex().w_full().gap_2().p_2().children(
                        self.entries.iter().enumerate().map(|(ix, entry)| {
                            match entry {
                                QueueItem::Action(action) => self
                                    .render_action(action, ix, window, cx)
                                    .into_any_element(),
                                QueueItem::Event(event) => {
                                    self.render_event(event, ix, cx).into_any_element()
                                }
                            }
                        }),
                    )),
            )
            .child(Scrollbar::vertical(&scroll_handle))
            .into_any_element()
    }
}
