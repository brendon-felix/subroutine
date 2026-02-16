use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, ParentElement,
    Render, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, IconName,
    button::Button,
    form::{field, v_form},
    h_flex,
    input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    label::Label,
    progress::Progress,
    select::SelectState,
    v_flex,
};

pub struct PreferencesPage {
    pub focus_handle: FocusHandle,
    preferred_time_of_day_input: Entity<InputState>,
}

impl PreferencesPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let preferred_time_of_day_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Enter preferred time of day"));

        Self {
            focus_handle: cx.focus_handle(),
            preferred_time_of_day_input,
        }
    }
}

impl Focusable for PreferencesPage {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PreferencesPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme();
        v_flex().size_full().gap_8().child(
            field()
                .label("Preferred Time of Day")
                .child(Input::new(&self.preferred_time_of_day_input)),
        )
    }
}
