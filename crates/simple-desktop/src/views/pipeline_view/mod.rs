use chrono::{DateTime, Duration as ChronoDuration, Local};
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyBinding, ParentElement, Render, SharedString, Styled, Window, actions, div,
    prelude::FluentBuilder,
};
use gpui_component::{
    menu::{PopupMenu, PopupMenuItem},
    v_flex,
};
use simple_core::AnyItem;

mod focus_view;
mod queue_view;
mod timeline_view;
use focus_view::*;
use queue_view::*;
use timeline_view::*;
use uuid::Uuid;

actions!([SwitchPipelineTab]);

/// Format a scheduled time compactly: "9:30am", "12pm", "1:45pm".
pub(super) fn format_item_time(t: DateTime<Local>) -> String {
    t.format("%-I:%M%P").to_string().replace(":00", "")
}

/// Format a duration compactly: "30m", "1h", "1h 30m".
pub(super) fn format_item_duration(d: ChronoDuration) -> String {
    let total_mins = d.num_minutes();
    if total_mins <= 0 {
        return String::new();
    }
    if total_mins % 60 == 0 {
        format!("{}h", total_mins / 60)
    } else if total_mins >= 60 {
        format!("{}h {}m", total_mins / 60, total_mins % 60)
    } else {
        format!("{}m", total_mins)
    }
}

/// Build a compact metadata string for an item, e.g. "9:30am · 30m".
pub(super) fn format_item_meta(item: &AnyItem) -> Option<SharedString> {
    let time_str = item.time_local().map(format_item_time);
    let dur_str = item
        .duration()
        .map(format_item_duration)
        .filter(|s| !s.is_empty());
    match (time_str, dur_str) {
        (Some(t), Some(d)) => Some(format!("{t} · {d}").into()),
        (Some(t), None) => Some(t.into()),
        (None, Some(d)) => Some(d.into()),
        (None, None) => None,
    }
}

pub struct DeleteItem {
    pub _item: AnyItem,
}

use crate::{
    AppIcon,
    stores::{AppDatabaseStore, DataChanged},
};

fn action_context_menu(
    action_id: Uuid,
    has_template: bool,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _window, _cx| {
        menu.item(
            PopupMenuItem::new("Complete")
                .icon(AppIcon::Check)
                .on_click(move |_event, _window, cx: &mut App| {
                    let db_store = AppDatabaseStore::global(cx);
                    db_store.update(cx, |store, cx| {
                        store.complete_action(action_id, cx);
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Demote to backlog")
                .icon(AppIcon::Minus)
                .on_click(move |_event, _window, cx: &mut App| {
                    let db_store = AppDatabaseStore::global(cx);
                    db_store.update(cx, |store, cx| {
                        store.backlog_action(action_id, cx);
                    });
                }),
        )
        .item(
            PopupMenuItem::new("Remove duration")
                .icon(AppIcon::CalendarClock)
                .on_click(move |_event, _window, cx: &mut App| {
                    let db_store = AppDatabaseStore::global(cx);
                    db_store.update(cx, |store, cx| {
                        store.clear_action_duration(action_id, cx);
                    });
                }),
        )
        .when(!has_template, |this| {
            this.item(
                PopupMenuItem::new("Save as template")
                    .icon(AppIcon::Save)
                    .on_click(move |_event, _window, cx: &mut App| {
                        let db_store = AppDatabaseStore::global(cx);
                        db_store.update(cx, |store, cx| {
                            store.save_action(action_id, cx);
                        });
                    }),
            )
        })
        .separator()
        .item(
            PopupMenuItem::new("Delete action")
                .icon(AppIcon::Trash)
                .on_click(move |_event, _window, cx: &mut App| {
                    let db_store = AppDatabaseStore::global(cx);
                    db_store.update(cx, |store, cx| {
                        store.delete_action(action_id, cx);
                    });
                }),
        )
    }
}

fn event_context_menu(
    event_id: Uuid,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _window, _cx| {
        menu.item(
            PopupMenuItem::new("Reschedule")
                .icon(AppIcon::CalendarClock)
                .on_click(|_, _, _cx| {}),
        )
        .separator()
        .item(
            PopupMenuItem::new("Delete event")
                .icon(AppIcon::Trash)
                .on_click(move |_event, _window, cx| {
                    let db_store = AppDatabaseStore::global(cx);
                    db_store.update(cx, |store, cx| {
                        store.delete_event(event_id, cx);
                    });
                }),
        )
    }
}

fn routine_context_menu(
    routine_id: Uuid,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _window, _cx| {
        menu.item(
            PopupMenuItem::new("Reschedule")
                .icon(AppIcon::CalendarClock)
                .on_click(|_, _, _cx| {}),
        )
        .separator()
        .item(
            PopupMenuItem::new("Delete routine")
                .icon(AppIcon::Trash)
                .on_click(move |_event, _window, cx| {
                    let db_store = AppDatabaseStore::global(cx);
                    db_store.update(cx, |store, cx| {
                        store.delete_routine(routine_id, cx);
                    });
                }),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedPipelineView {
    Timeline = 0,
    Queue = 1,
    Focus = 2,
}

pub struct PipelineView {
    pub(crate) focus_handle: FocusHandle,
    selected_view: SelectedPipelineView,
    timeline_view: Entity<TimelineView>,
    queue_view: Entity<QueueView>,
    focus_view: Entity<FocusView>,
}

impl PipelineView {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        cx.bind_keys([KeyBinding::new("ctrl-tab", SwitchPipelineTab, None)]);

        let timeline_view = cx.new(|cx| TimelineView::new(cx));
        let queue_view = cx.new(|cx| QueueView::new(cx));
        let focus_view = cx.new(|cx| FocusView::new(cx));

        let db_store = AppDatabaseStore::global(cx);
        cx.subscribe(&db_store, |view, store, _: &DataChanged, cx| {
            let queue = store.read(cx).sorted_queue();
            let all_items = store.read(cx).sorted_timeline();
            view.timeline_view.update(cx, |timeline_view, cx| {
                timeline_view.refresh_items(all_items, cx)
            });
            view.queue_view.update(cx, |queue_view, cx| {
                queue_view.refresh_items(queue.clone(), cx)
            });
            view.focus_view
                .update(cx, |focus_view, cx| focus_view.refresh_items(queue, cx));
        })
        .detach();

        Self {
            focus_handle,
            selected_view: SelectedPipelineView::Queue,
            timeline_view,
            queue_view,
            focus_view,
        }
    }

    pub fn selected_view(&self) -> SelectedPipelineView {
        self.selected_view
    }

    pub fn select_view(
        &mut self,
        view: SelectedPipelineView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_view = view;
        let fh = match view {
            SelectedPipelineView::Timeline => self.timeline_view.read(cx).focus_handle.clone(),
            SelectedPipelineView::Queue => self.queue_view.read(cx).focus_handle.clone(),
            SelectedPipelineView::Focus => self.focus_view.read(cx).focus_handle.clone(),
        };
        fh.focus(window, cx);
        cx.notify();
    }
}

impl Focusable for PipelineView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PipelineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            // .gap_2()
            .on_action(cx.listener(|view, _: &SwitchPipelineTab, window, cx| {
                let next = match view.selected_view {
                    SelectedPipelineView::Timeline => SelectedPipelineView::Queue,
                    SelectedPipelineView::Queue => SelectedPipelineView::Focus,
                    SelectedPipelineView::Focus => SelectedPipelineView::Timeline,
                };
                view.select_view(next, window, cx);
            }))
            // .child(
            //     TabBar::new("selected-view")
            //         .px_4()
            //         // .pill()
            //         // .outline()
            //         // .segmented()
            //         .underline()
            //         .selected_index(self.selected_view as usize)
            //         .on_click(cx.listener(|view, index, window, cx| {
            //             match index {
            //                 0 => view.select_view(SelectedPipelineView::Timeline, window, cx),
            //                 1 => view.select_view(SelectedPipelineView::Queue, window, cx),
            //                 _ => unreachable!(),
            //             };
            //         }))
            //         .child(Tab::new().label("Timeline").on_drag_move(cx.listener(
            //             |view, event: &DragMoveEvent<DragData<AnyItem>>, window, cx| {
            //                 let is_over = event.bounds.contains(&event.event.position);
            //                 if is_over {
            //                     view.select_view(SelectedPipelineView::Timeline, window, cx);
            //                 }
            //             },
            //         )))
            //         .child(Tab::new().label("Queue").on_drag_move(cx.listener(
            //             |view, event: &DragMoveEvent<DragData<AnyItem>>, window, cx| {
            //                 let is_over = event.bounds.contains(&event.event.position);
            //                 if is_over {
            //                     view.select_view(SelectedPipelineView::Queue, window, cx);
            //                 }
            //             },
            //         ))),
            // )
            .child(
                div()
                    .size_full()
                    // .border_1()
                    // .border_color(cx.theme().border)
                    // .rounded_xl()
                    .on_action(|_: &RefreshPipeline, _, cx| {
                        let db_store = AppDatabaseStore::global(cx);
                        db_store.update(cx, |store, cx| {
                            store.refresh_pipeline(cx);
                        });
                    })
                    .map(|this| match self.selected_view {
                        SelectedPipelineView::Timeline => this.child(self.timeline_view.clone()),
                        SelectedPipelineView::Queue => this.child(self.queue_view.clone()),
                        SelectedPipelineView::Focus => this.child(self.focus_view.clone()),
                    }),
            )
    }
}
