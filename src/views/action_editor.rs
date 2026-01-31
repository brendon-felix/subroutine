use gpui::{AppContext, Context, Entity, ParentElement, Render, Styled, Window, div, px};
use gpui_component::{
    form::{field, v_form},
    input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    label::Label,
    v_flex,
};

use crate::stores::DatabaseStore;

pub struct ActionEditor {
    first_name: Entity<InputState>,
    last_name: Entity<InputState>,
    bio_input: Entity<InputState>,
    age_input: Entity<InputState>,
    age_value: Option<u32>,
    database_store: Entity<DatabaseStore>,
}

impl ActionEditor {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // let state = cx.new(|cx| ActionEditorState::new(window, cx));

        let first_name = cx.new(|cx| InputState::new(window, cx));
        let last_name = cx.new(|cx| InputState::new(window, cx));
        let bio_input = cx.new(|cx| InputState::new(window, cx));
        let age_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Enter number")
                .default_value("1")
                .pattern(regex::Regex::new(r"^\d+$").unwrap())
        });

        cx.subscribe_in(
            &age_input,
            window,
            |view, state, event, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value();
                    // if let Ok(new_value) = text.parse::<u32>() {
                    //     view.age_value = new_value;
                    // }
                    view.age_value = text.parse::<u32>().ok();
                }
                _ => {}
            },
        )
        .detach();

        // Subscribe to increment/decrement actions
        cx.subscribe_in(
            &age_input,
            window,
            |view, state, event, window, cx| match event {
                NumberInputEvent::Step(step_action) => match step_action {
                    StepAction::Increment => {
                        if let Some(current_age) = view.age_value {
                            view.age_value = Some(current_age.saturating_add(1));
                        } else {
                            view.age_value = Some(1);
                        }
                        state.update(cx, |input, cx| {
                            input.set_value(view.age_value.unwrap_or(0).to_string(), window, cx);
                        });
                    }
                    StepAction::Decrement => {
                        if let Some(current_age) = view.age_value {
                            view.age_value = Some(current_age.saturating_sub(1));
                        } else {
                            view.age_value = Some(0);
                        }
                        state.update(cx, |input, cx| {
                            input.set_value(view.age_value.unwrap_or(0).to_string(), window, cx);
                        });
                    }
                },
            },
        )
        .detach();

        Self {
            // state,
            first_name,
            last_name,
            bio_input,
            age_input,
            age_value: None,
            database_store,
        }
    }
}

impl Render for ActionEditor {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        v_flex()
            .size_full()
            .pt_8()
            .items_center()
            .gap_8()
            .child(
                Label::new("Action Editor")
                    .font_family("Georgia")
                    .text_2xl(),
            )
            // .child(self.state.clone())
            .child(
                div().max_w(px(1200.)).child(
                    v_form()
                        .w_full()
                        .columns(2) // Two-column layout
                        .child(
                            field()
                                .label("First Name")
                                .child(Input::new(&self.first_name)),
                        )
                        .child(
                            field()
                                .label("Last Name")
                                .child(Input::new(&self.last_name)),
                        )
                        .child(
                            field()
                                .label("Bio")
                                .col_span(2) // Span across both columns
                                .child(Input::new(&self.bio_input)),
                        )
                        .child(
                            field()
                                .label("Age")
                                .col_span(2)
                                .child(NumberInput::new(&self.age_input)),
                        ),
                ),
            )
    }
}
