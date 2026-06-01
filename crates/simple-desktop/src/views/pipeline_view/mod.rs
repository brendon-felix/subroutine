use chrono::{DateTime, Duration as ChronoDuration, Local};
use gpui::{
    App, AppContext, Context, DragMoveEvent, Entity, InteractiveElement, IntoElement, KeyBinding,
    ParentElement, Render, SharedString, Styled, Subscription, Window, actions, div,
    prelude::FluentBuilder,
};
use gpui_component::{
    ActiveTheme,
    menu::{PopupMenu, PopupMenuItem},
    tab::{Tab, TabBar},
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
    components::DragData,
    stores::{AppDatabaseStore, DataChanged},
};

fn action_context_menu(
    action_id: Uuid,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedPipelineView {
    Timeline = 0,
    Queue = 1,
    Focus = 2,
}

pub struct PipelineView {
    selected_view: SelectedPipelineView,
    timeline_view: Entity<TimelineView>,
    queue_view: Entity<QueueView>,
    focus_view: Entity<FocusView>,
    _focus_lost_subscription: Subscription,
}

impl PipelineView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([KeyBinding::new("ctrl-tab", SwitchPipelineTab, None)]);

        let timeline_view = cx.new(|cx| TimelineView::new(cx));
        let queue_view = cx.new(|cx| QueueView::new(cx));
        let focus_view = cx.new(|cx| FocusView::new(cx));

        let selected_view = SelectedPipelineView::Timeline;
        cx.focus_view(&timeline_view, window);

        let db_store = AppDatabaseStore::global(cx);
        cx.subscribe(&db_store, |view, store, _: &DataChanged, cx| {
            let queue = store.read(cx).sorted_queue();
            view.timeline_view.update(cx, |timeline_view, cx| {
                timeline_view.refresh_items(queue.clone(), cx)
            });
            view.queue_view.update(cx, |queue_view, cx| {
                queue_view.refresh_items(queue.clone(), cx)
            });
            view.focus_view
                .update(cx, |focus_view, cx| focus_view.refresh_items(queue, cx));
        })
        .detach();

        // When focus is dropped entirely (e.g. all items completed/removed),
        // immediately refocus the active pipeline view so actions keep working.
        let focus_lost_subscription =
            cx.on_focus_lost(window, |this, window, cx| match this.selected_view {
                SelectedPipelineView::Timeline => cx.focus_view(&this.timeline_view, window),
                SelectedPipelineView::Queue => cx.focus_view(&this.queue_view, window),
                SelectedPipelineView::Focus => cx.focus_view(&this.focus_view, window),
            });

        Self {
            selected_view,
            timeline_view,
            queue_view,
            focus_view,
            _focus_lost_subscription: focus_lost_subscription,
        }
    }

    pub fn select_view(
        &mut self,
        view: SelectedPipelineView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_view = view;
        match view {
            SelectedPipelineView::Timeline => cx.focus_view(&self.timeline_view, window),
            SelectedPipelineView::Queue => cx.focus_view(&self.queue_view, window),
            SelectedPipelineView::Focus => cx.focus_view(&self.focus_view, window),
        }
        cx.notify();
    }
}

impl Render for PipelineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
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
