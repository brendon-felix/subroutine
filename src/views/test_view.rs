use gpui::{
    Context, IntoElement, KeyBinding, ParentElement, Render, StyleRefinement, Styled, Window,
    actions, div, relative,
};
use gpui_component::{
    ActiveTheme, StyledExt,
    button::{Button, ButtonVariants},
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    switch::Switch,
    v_flex,
};

use crate::components::checkbox::Checkbox;

actions!(test_view, [ToggleSheet]);

pub struct TestView {
    // sheet_visible: bool,
}

impl TestView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        cx.bind_keys([KeyBinding::new("cmd-b", ToggleSheet, None)]);
        Self {
            // sheet_visible: false,
        }
    }
}

impl Render for TestView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .gap_8()
            .px_32()
            .children(vec![
                GroupBox::new()
                    .child("Subscriptions")
                    .child(Checkbox::new("all").label("All"))
                    .child(Checkbox::new("newsletter").label("Newsletter"))
                    .child(Button::new("save").primary().label("Save")),
                GroupBox::new().child("Content without visual container"),
                GroupBox::new()
                    .fill()
                    .title("Settings")
                    .child("Content with background"),
                GroupBox::new()
                    .outline()
                    .title("Preferences")
                    .child("Content with border"),
            ])
            .child(
                GroupBox::new()
                    .id("notification-settings")
                    .outline()
                    .bg(cx.theme().group_box)
                    .rounded_xl()
                    .p_5()
                    .title("Notification Preferences")
                    .title_style(
                        StyleRefinement::default()
                            .font_semibold()
                            .line_height(relative(1.0))
                            .px_3(),
                    )
                    .content_style(
                        StyleRefinement::default()
                            .rounded_xl()
                            .py_3()
                            .px_4()
                            .border_2(),
                    )
                    .child(
                        v_flex()
                            .gap_3()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child("Email notifications")
                                    .child(Switch::new("email").checked(true)),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child("Push notifications")
                                    .child(Switch::new("push").checked(false)),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child("SMS notifications")
                                    .child(Switch::new("sms").checked(false)),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_end()
                            .gap_2()
                            .child(Button::new("cancel").label("Cancel"))
                            .child(Button::new("save").primary().label("Save Settings")),
                    ),
            )
    }
}
