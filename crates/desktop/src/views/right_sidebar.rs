use gpui::{
    AppContext as _, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Window, div, prelude::FluentBuilder,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable,
    tab::{Tab, TabBar},
    v_flex,
};

use crate::{
    stores::DatabaseStore,
    views::{
        BacklogListView, saved_actions::SavedActionsListView, saved_events::SavedEventsListView,
    },
};

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
enum RightSidebarTab {
    Backlog = 0,
    SavedActions = 1,
    SavedEvents = 2,
}

pub struct RightSidebarView {
    selected_tab: RightSidebarTab,
    backlog: Entity<BacklogListView>,
    saved_actions: Entity<SavedActionsListView>,
    saved_events: Entity<SavedEventsListView>,
}

impl RightSidebarView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let backlog = cx.new(|cx| BacklogListView::new(database_store.clone(), window, cx));
        let saved_actions =
            cx.new(|cx| SavedActionsListView::new(database_store.clone(), window, cx));
        let saved_events =
            cx.new(|cx| SavedEventsListView::new(database_store.clone(), window, cx));
        Self {
            selected_tab: RightSidebarTab::Backlog,
            backlog,
            saved_actions,
            saved_events,
        }
    }
}

impl Render for RightSidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_tab = self.selected_tab;

        div().id("right-sidebar").size_full().child(
            v_flex()
                .size_full()
                .bg(cx.theme().background)
                .rounded_lg()
                .child(
                    TabBar::new("right-sidebar-tabs")
                        .small()
                        .underline()
                        .w_full()
                        .items_center()
                        .selected_index(selected_tab as usize)
                        .child(
                            Tab::new()
                                .flex_1()
                                .icon(IconName::Inbox)
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.selected_tab = RightSidebarTab::Backlog;
                                })),
                        )
                        .child(
                            Tab::new()
                                .flex_1()
                                .icon(IconName::Palette)
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.selected_tab = RightSidebarTab::SavedActions;
                                })),
                        )
                        .child(
                            Tab::new()
                                .flex_1()
                                .icon(IconName::Calendar)
                                .on_click(cx.listener(|this, _, _, _| {
                                    this.selected_tab = RightSidebarTab::SavedEvents;
                                })),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .bg(cx.theme().background)
                        .when(selected_tab == RightSidebarTab::Backlog, |this| {
                            this.child(self.backlog.clone())
                        })
                        .when(selected_tab == RightSidebarTab::SavedActions, |this| {
                            this.child(self.saved_actions.clone())
                        })
                        .when(selected_tab == RightSidebarTab::SavedEvents, |this| {
                            this.child(self.saved_events.clone())
                        }),
                ),
        )
    }
}
