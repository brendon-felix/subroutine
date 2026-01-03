use gpui::{Context, Entity, EventEmitter, IntoElement, Render, Styled, Subscription, Window, div};
use gpui::{prelude::*, rgb};
use gpui_component::sidebar::{
    Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem,
};
use gpui_component::{IconName, Side, v_flex};

use crate::stores::ui_store::{UiStateChanged, UiStateStore, ViewType};

pub struct LeftSidebarView {
    ui_store: Entity<UiStateStore>,
    collapsed: bool,
    _subscriptions: Vec<Subscription>,
}

impl LeftSidebarView {
    pub fn new(ui_store: Entity<UiStateStore>, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.subscribe(
            &ui_store,
            |_this, _ui_store, _event: &UiStateChanged, cx| {
                cx.notify();
            },
        )];

        Self {
            ui_store,
            collapsed: false,
            _subscriptions: subscriptions,
        }
    }

    fn create_nav_item(
        &self,
        view_type: ViewType,
        icon_name: IconName,
        cx: &Context<Self>,
    ) -> SidebarMenuItem {
        let ui_state = self.ui_store.read(cx);
        let is_active = ui_state.current_view == view_type;

        let ui_store = self.ui_store.clone();
        let view_type_copy = view_type.clone();

        SidebarMenuItem::new(view_type.label().to_string())
            .icon(icon_name)
            .active(is_active)
            .on_click(move |_event, _window, cx| {
                ui_store.update(cx, |ui_state, cx| {
                    ui_state.set_current_view(view_type_copy.clone(), cx);
                    cx.emit(UiStateChanged);
                    cx.notify();
                });
            })
    }

    pub fn toggle_collapsed(&mut self, cx: &mut Context<Self>) {
        self.collapsed = !self.collapsed;
        cx.notify();
    }

    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }
}

impl EventEmitter<()> for LeftSidebarView {}

impl Render for LeftSidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Sidebar::new(Side::Left)
            // .bg(rgb(0x191919))
            .collapsed(self.collapsed)
            .header(
                SidebarHeader::new()
                    // .child(
                    //     div()
                    //         .flex()
                    //         .items_center()
                    //         .justify_center()
                    //         .rounded_md()
                    //         .size_8()
                    //         .flex_shrink_0()
                    //         .text_lg()
                    //         .child("S"),
                    // )
                    .when(!self.collapsed, |header| {
                        header.child(
                            v_flex()
                                .gap_0()
                                .text_sm()
                                .flex_1()
                                .overflow_hidden()
                                .child("Subroutine")
                                .child(div().child("Task Manager").text_xs().opacity(0.7)),
                        )
                    }),
            )
            .child(
                SidebarGroup::new("Navigation").child(
                    SidebarMenu::new()
                        .child(self.create_nav_item(ViewType::Today, IconName::Calendar, cx))
                        .child(self.create_nav_item(ViewType::Upcoming, IconName::ArrowUp, cx))
                        .child(self.create_nav_item(ViewType::Inbox, IconName::Inbox, cx))
                        .child(self.create_nav_item(ViewType::AllTasks, IconName::Asterisk, cx)),
                ),
            )
            .when(!self.collapsed, |sidebar| {
                sidebar.footer(
                    SidebarFooter::new().child(div().text_xs().child("Press ⌘+P for commands")),
                )
            })
    }
}
