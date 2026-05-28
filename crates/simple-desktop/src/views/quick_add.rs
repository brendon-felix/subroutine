use std::time::Duration;

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{ActiveTheme, Icon, animation::ease_out_cubic, h_flex};
use gpui_transitions::WindowUseTransition;

use crate::{
    AppIcon,
    utils::{ButtonColorizeExt, ButtonColors, fraction},
};

#[derive(Copy, Clone, PartialEq)]
enum InputState {
    Closed,
    Closing,
    Open,
    Opening,
}

pub struct QuickAdd {
    hovered: bool,
    input: InputState,
}

impl QuickAdd {
    pub fn new(cx: &Context<Self>) -> Self {
        let hovered = false;
        let input = InputState::Closed;
        Self { hovered, input }
    }

    pub fn open_action_creator(&self, cx: &Context<Self>) {}

    pub fn open_event_creator(&self, cx: &Context<Self>) {}
}

impl Render for QuickAdd {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total_width = px(26. * 4.);
        let width_transition = window
            .use_keyed_transition("width", cx, Duration::from_millis(150), |_, _| {
                total_width / 2.
            })
            .with_easing(ease_out_cubic);
        let radius_transition = window
            .use_keyed_transition("radius", cx, Duration::from_millis(150), |_, _| px(0.))
            .with_easing(ease_out_cubic);
        let icon_opacity = window
            .use_keyed_transition("opacity", cx, Duration::from_millis(150), |_, _| 0.0)
            .with_easing(ease_out_cubic);

        let input_width_transition = window
            .use_keyed_transition("input-width", cx, Duration::from_millis(150), |_, _| 0.1)
            .with_easing(ease_out_cubic);
        let button_opacity_transition = window
            .use_keyed_transition("button-opacity", cx, Duration::from_millis(150), |_, _| 1.0)
            .with_easing(ease_out_cubic);

        let width = *width_transition.evaluate(window, cx);
        let radius = *radius_transition.evaluate(window, cx);
        let opacity = *icon_opacity.evaluate(window, cx);

        let input_width = *input_width_transition.evaluate(window, cx);
        let button_opacity = *button_opacity_transition.evaluate(window, cx);

        match self.input {
            InputState::Closing => {
                println!("closing");
                input_width_transition.update(cx, |value, _cx| *value = 0.1);
                button_opacity_transition.update(cx, |value, cx| *value = 1.0);
                self.input = InputState::Closed;
                window.request_animation_frame();
            }
            InputState::Opening => {
                println!("opening");
                input_width_transition.update(cx, |value, _cx| *value = 1.0);
                button_opacity_transition.update(cx, |value, cx| *value = 0.0);
                self.input = InputState::Open;
                window.request_animation_frame();
            }
            _ => {}
        }

        let mut button_colors = ButtonColors::outline(cx.theme().secondary, cx);
        button_colors.border = Some(cx.theme().border);

        h_flex()
            .w_full()
            .h_12()
            .child(
                h_flex()
                    .id("quick-add")
                    .h_full()
                    .w(total_width)
                    .opacity(button_opacity)
                    .on_hover(cx.listener(move |view, hovered, _, cx| {
                        view.hovered = *hovered;
                        width_transition.update(cx, |value, cx| {
                            *value = (total_width / 2.) - (*hovered as u8 as f32) * px(4.);
                        });
                        radius_transition.update(cx, |value, cx| {
                            *value = *hovered as u8 as f32 * px(12.);
                        });
                        icon_opacity.update(cx, |value, cx| {
                            *value = *hovered as u8 as f32;
                        });
                    }))
                    // .gap_2()
                    .justify_between()
                    .child(
                        div()
                            .absolute()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(AppIcon::Plus).size_5().opacity(1.0 - opacity)),
                    )
                    .child(
                        div()
                            .id("quick-add-action")
                            .h_full()
                            .w(width)
                            // .border_1()
                            // .border_r_0()
                            .border_color(cx.theme().border)
                            .button_colors(button_colors)
                            .border_r(px(self.hovered as u8 as f32)) // must come after button_colors
                            .rounded_xl()
                            .rounded_r(radius)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(AppIcon::ListPlus).size_4().opacity(opacity))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.input = match view.input {
                                    InputState::Closed | InputState::Closing => InputState::Opening,
                                    InputState::Open | InputState::Opening => InputState::Closing,
                                };
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("quick-add-event")
                            .h_full()
                            .w(width)
                            // .border_1()
                            // .border_l_0()
                            .border_color(cx.theme().border)
                            .button_colors(button_colors)
                            .border_l(px(self.hovered as u8 as f32)) // must come after button_colors
                            .rounded_xl()
                            .rounded_l(radius)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(AppIcon::CalendarPlus).size_4().opacity(opacity))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.input = match view.input {
                                    InputState::Closed | InputState::Closing => InputState::Opening,
                                    InputState::Open | InputState::Opening => InputState::Closing,
                                };
                                cx.notify();
                            })),
                    ),
            )
            .when(input_width > 0.11, |this| {
                this.child(
                    div()
                        .id("quick-add-input")
                        .absolute()
                        // .button_colors(button_colors)
                        .border_1()
                        .border_color(cx.theme().border)
                        .h_full()
                        .rounded_xl()
                        .border_1()
                        .w(fraction(input_width)),
                )
            })
    }
}
