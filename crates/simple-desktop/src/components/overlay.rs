use gpui::{
    App, InteractiveElement, IntoElement, KeyBinding, Length, MouseButton, ParentElement, Styled,
    actions, div, prelude::FluentBuilder,
};
use gpui_component::v_flex;

actions!(overlay, [CloseOverlay]);

pub fn init(cx: &mut App) {
    let context: Option<&str> = Some("Overlay");
    cx.bind_keys([KeyBinding::new("escape", CloseOverlay, context)]);
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(unused)]
pub enum OverlayPosition {
    Top(Length),
    Center,
}

pub fn overlay<T: IntoElement>(
    inner: T,
    position: OverlayPosition,
    _cx: &mut App,
) -> impl IntoElement {
    // let bg_color = cx
    //     .theme()
    //     .background
    //     .blend(gpui::black().opacity(0.15))
    //     .opacity(0.4);

    let spacer_height = match position {
        OverlayPosition::Top(top) => Some(top),
        OverlayPosition::Center => None,
    };

    v_flex()
        // .bg(bg_color)
        .absolute()
        .inset_0()
        .size_full()
        .occlude()
        .key_context("Overlay")
        .on_mouse_down(MouseButton::Left, |_event, window, cx| {
            window.dispatch_action(Box::new(CloseOverlay), cx);
        })
        .items_center()
        // .opacity(0.8)
        .when(position == OverlayPosition::Center, |this| {
            this.justify_center()
        })
        .when_some(spacer_height, |this, top| this.child(div().w_full().h(top)))
        .child(inner)
}
