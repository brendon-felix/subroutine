use gpui::prelude::*;
use gpui::{
    Context, Entity, EventEmitter, IntoElement, MouseButton, Render, Styled, Subscription, Window,
    div, px, rgb,
};

use crate::stores::ui_store::{UiStateChanged, UiStateStore, ViewType};

pub struct SidebarView {
    ui_store: Entity<UiStateStore>,
    _subscriptions: Vec<Subscription>,
}

impl SidebarView {
    pub fn new(ui_store: Entity<UiStateStore>, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.subscribe(
            &ui_store,
            |_this, _ui_store, _event: &UiStateChanged, cx| {
                cx.notify();
            },
        )];

        Self {
            ui_store,
            _subscriptions: subscriptions,
        }
    }

    fn render_nav_item(
        &self,
        view_type: ViewType,
        label: &str,
        icon: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let ui_state = self.ui_store.read(cx);
        let is_selected = ui_state.current_view == view_type;

        div()
            .flex()
            .items_center()
            .px_3()
            .py_2()
            .rounded(px(6.0))
            .cursor_pointer()
            .when(is_selected, |element| {
                element.bg(rgb(0x303030)).text_color(rgb(0xC8C8C8))
            })
            .when(!is_selected, |element| {
                element.hover(|element| element.bg(rgb(0x252525)))
            })
            .on_mouse_down(MouseButton::Left, {
                let ui_store = self.ui_store.clone();
                let view_type_copy = view_type.clone();
                cx.listener(move |_this, _event, _window, cx| {
                    ui_store.update(cx, |ui_state, cx| {
                        ui_state.set_current_view(view_type_copy.clone());
                        cx.emit(UiStateChanged);
                        cx.notify();
                    });
                })
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().text_sm().child(icon.to_string()))
                    .child(div().text_sm().child(label.to_string())),
            )
    }
}

impl EventEmitter<()> for SidebarView {}

impl Render for SidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sidebar")
            .h_full()
            .flex()
            .flex_col()
            .p_4()
            .gap_2()
            .child(
                div().mb_4().child(
                    div()
                        .text_lg()
                        .text_color(rgb(0xC8C8C8))
                        .font_family("Firacode Nerd Font")
                        .child(div().flex().justify_center().child("Subroutine \u{f0134}")),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(self.render_nav_item(ViewType::TaskList, "All Tasks", "\u{f096a}", cx))
                    .child(self.render_nav_item(ViewType::Today, "Today", "\u{f00f6}", cx))
                    .child(self.render_nav_item(ViewType::Upcoming, "Upcoming", "\u{f0a33}", cx)),
                // .child(self.render_nav_item(ViewType::Completed, "Completed", "\u{f012c}", cx)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .mt_auto()
                    .pt_4()
                    .border_t_1()
                    .border_color(rgb(0x404040))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x888888))
                            .child("Press ⌘+P for commands"),
                    ),
            )
    }
}
