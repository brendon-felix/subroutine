use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::{ActiveTheme, button::Button, h_flex, label::Label, v_flex};

use crate::components::Divider;

pub struct HomeView {}

impl HomeView {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for HomeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .gap_4()
                    .child(Label::new("Subroutine"))
                    .child(Divider::horizontal().w_full().color(cx.theme().border))
                    .child(
                        v_flex()
                            .gap_4()
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_4()
                                    .child(Button::new("1").label("analysis paralysis"))
                                    .child(Button::new("2").label("overstimulated")),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_4()
                                    .child(Button::new("3").label("hyperfocused"))
                                    .child(Button::new("4").label("an intense emotion")),
                            ),
                    ),
            )
    }
}
