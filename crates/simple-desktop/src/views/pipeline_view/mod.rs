use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Window, div, prelude::FluentBuilder,
};
use gpui_component::{
    ActiveTheme,
    menu::{PopupMenu, PopupMenuItem},
    tab::{Tab, TabBar},
    v_flex,
};
use simple_core::AnyItem;

mod queue_view;
mod timeline_view;
use queue_view::*;
use timeline_view::*;
use uuid::Uuid;

pub struct DeleteItem {
    pub _item: AnyItem,
}

use crate::{
    AppIcon,
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
}

pub struct PipelineView {
    selected_view: SelectedPipelineView,
    timeline_view: Entity<TimelineView>,
    queue_view: Entity<QueueView>,
}

impl PipelineView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let timeline_view = cx.new(|cx| TimelineView::new(cx));
        let queue_view = cx.new(|cx| QueueView::new(cx));

        let selected_view = SelectedPipelineView::Timeline;
        cx.focus_view(&timeline_view, window);

        let db_store = AppDatabaseStore::global(cx);
        cx.subscribe(&db_store, |view, store, _: &DataChanged, cx| {
            let queue = store.read(cx).sorted_queue();
            view.timeline_view.update(cx, |timeline_view, cx| {
                timeline_view.refresh_items(queue.clone(), cx)
            });
            view.queue_view
                .update(cx, |queue_view, cx| queue_view.refresh_items(queue, cx));
        })
        .detach();

        Self {
            selected_view,
            timeline_view,
            queue_view,
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
        }
        cx.notify();
    }
}

impl Render for PipelineView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_2()
            .child(
                TabBar::new("selected-view")
                    .pill()
                    .selected_index(self.selected_view as usize)
                    .on_click(cx.listener(|view, index, window, cx| {
                        match index {
                            0 => view.select_view(SelectedPipelineView::Timeline, window, cx),
                            1 => view.select_view(SelectedPipelineView::Queue, window, cx),
                            _ => unreachable!(),
                        };
                    }))
                    .child(Tab::new().label("Timeline"))
                    .child(Tab::new().label("Queue")),
            )
            .child(
                div()
                    .size_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_xl()
                    .on_action(|_: &RefreshPipeline, _, cx| {
                        let db_store = AppDatabaseStore::global(cx);
                        db_store.update(cx, |store, cx| {
                            store.refresh_pipeline(cx);
                        });
                    })
                    .when(
                        self.selected_view == SelectedPipelineView::Timeline,
                        |this| this.child(self.timeline_view.clone()),
                    )
                    .when(self.selected_view == SelectedPipelineView::Queue, |this| {
                        this.child(self.queue_view.clone())
                    }),
            )
    }
}
