use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, ParentElement,
    Render, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, IconName,
    button::Button,
    form::{Field, field, v_form},
    h_flex,
    input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    label::Label,
    progress::Progress,
    select::SelectState,
    v_flex,
};

pub struct GeneralPage {
    description_input: Entity<InputState>,
    // type: Entity<SelectState<&'static str>>,
}

impl GeneralPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let description_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Enter description"));

        Self { description_input }
    }

    pub fn fields(&self) -> Vec<Field> {
        vec![
            field()
                .label("Description")
                .child(Input::new(&self.description_input)),
        ]
    }
}

// impl Focusable for GeneralPage {
//     fn focus_handle(&self, _cx: &App) -> FocusHandle {
//         self.focus_handle.clone()
//     }
// }

// impl Render for GeneralPage {
//     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
//         let theme = cx.theme();
//         v_flex()
//             .size_full()
//             .gap_8()
//             .child(Label::new("What is involved?"))
//             .child()
//     }
// }
