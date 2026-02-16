use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, ParentElement,
    Render, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, IconName,
    button::Button,
    form::{field, v_form},
    h_flex,
    input::{Input, InputState, NumberInput, NumberInputEvent, StepAction},
    v_flex,
};

use gpui_component::slider::{Slider, SliderEvent, SliderState};

pub struct MetadataPage {
    pub focus_handle: FocusHandle,
    // duration_input: Entity<InputState>,
    duration_slider: Entity<SliderState>,
    duration_value: f32,
    energy_input: Entity<InputState>,
}

impl MetadataPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // let duration_input = cx.new(|cx| InputState::new(window, cx).placeholder("Enter duration"));

        let duration_slider =
            cx.new(|cx| SliderState::new().min(1.0).max(144.0).default_value(5.0));

        cx.subscribe(
            &duration_slider,
            |this, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    this.duration_value = value.start();
                    println!("Duration changed to {}", this.duration_value);
                    cx.notify();
                }
            },
        )
        .detach();

        let energy_input = cx.new(|cx| InputState::new(window, cx).placeholder("Enter energy"));

        Self {
            focus_handle: cx.focus_handle(),
            // duration_input,
            duration_slider,
            duration_value: 5.0,
            energy_input,
        }
    }
}

impl Focusable for MetadataPage {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MetadataPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();
        v_flex()
            .size_full()
            .gap_8()
            .child(
                field()
                    .label("Duration")
                    // .child(Input::new(&self.duration_input)),
                    .child(Slider::new(&self.duration_slider)),
            )
            .child(
                field()
                    .label("Energy")
                    .child(Input::new(&self.energy_input)),
            )
    }
}
