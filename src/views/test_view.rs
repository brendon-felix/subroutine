use gpui::{
    Context, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    prelude::FluentBuilder,
};
use gpui_component::{
    ActiveTheme, IconName, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    clipboard::Clipboard,
    collapsible::Collapsible,
    h_flex,
    notification::Notification,
    v_flex,
};

pub struct TestView {
    open: bool,
}

impl TestView {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { open: false }
    }
}

impl Render for TestView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // let theme = cx.theme();
        v_flex()
            .size_full()
            .p_8()
            .pt_24()
            .gap_8()
            .items_center()
            .child(
                Collapsible::new()
                    .max_w_128()
                    .gap_1()
                    .open(self.open)
                    .child(
                        "This is a collapsible component. \
                    Click the header to expand or collapse the content.",
                    )
                    .content(
                        "This is the full content of the Collapsible component. \
                    It is only visible when the component is expanded. \n\
                    You can put any content you like here, including text, images, \
                    or other UI elements.",
                    )
                    .child(
                        h_flex().justify_center().child(
                            Button::new("toggle1")
                                .icon(IconName::ChevronDown)
                                .label("Show more")
                                .when(self.open, |this| {
                                    this.icon(IconName::ChevronUp).label("Show less")
                                })
                                .xsmall()
                                .link()
                                .on_click({
                                    cx.listener(move |this, _, _, cx| {
                                        this.open = !this.open;
                                        cx.notify();
                                    })
                                }),
                        ),
                    ),
            )
            .child(
                Clipboard::new("dynamic-clipboard")
                    .value_fn(move |_, cx| {
                        SharedString::from("This is some copied text".to_string())
                    })
                    .on_copied(|value, window, cx| {
                        window.push_notification(format!("Copied: {}", value), cx)
                    }),
            )
    }
}

struct SaveConfirmation;
