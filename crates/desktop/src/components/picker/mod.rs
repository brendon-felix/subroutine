use gpui::{
    App, Entity, FocusHandle, Focusable, KeyBinding, Pixels, StyleRefinement, Window, actions, div,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme, StyledExt, h_flex,
    input::{Input, InputEvent, InputState},
    kbd::Kbd,
    label::Label,
    v_flex,
};

mod delegate;
mod list;

pub use delegate::*;

use crate::components::custom_list::{ListDelegate, ListItem, ListState};

actions!(picker, [NavigateUp, NavigateDown, Confirm, Cancel]);

pub fn init(cx: &mut App) {
    let context: Option<&str> = Some("Picker");
    cx.bind_keys([
        KeyBinding::new("escape", Cancel, context),
        KeyBinding::new("enter", Confirm, context),
        KeyBinding::new("up", NavigateUp, context),
        KeyBinding::new("down", NavigateDown, context),
    ]);
}

/// Events emitted by the picker
pub enum PickerEvent {
    Confirm(usize),
    Cancel,
}

/// State for a generic picker component
pub struct PickerState<D>
where
    D: PickerDelegate + ListDelegate,
{
    pub focus_handle: FocusHandle,
    pub search_query: Option<String>,
    pub input_state: Entity<InputState>,
    pub list_state: Entity<ListState<D>>,
    pub list_height: Pixels,
    max_height: Pixels,
    min_height: Pixels,
}

impl<D> Focusable for PickerState<D>
where
    D: PickerDelegate + ListDelegate,
{
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input_state.focus_handle(cx)
    }
}

#[allow(unused)]
impl<D: PickerDelegate> PickerState<D> {
    pub fn new(delegate: D, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = delegate.placeholder_text().to_string();
        let initial_items_count = delegate.items_count();

        let input_state = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder(&placeholder);
            state.focus(window, cx);
            state
        });

        // Subscribe to input changes
        cx.subscribe(&input_state, |this, _input, event, cx| {
            match event {
                InputEvent::Change => {
                    let query = this.input_state.read(cx).value().to_string();
                    this.search_query = if query.is_empty() { None } else { Some(query) };
                    this.update_search(cx);
                }
                _ => {}
            }
            cx.notify();
        })
        .detach();

        let list_delegate = PickerListDelegate::new(delegate);
        let list_state = cx.new(|cx| {
            let mut state = ListState::new(list_delegate, window, cx);
            if initial_items_count > 0 {
                state.set_selected_index(Some(0));
            }
            state
        });

        // Subscribe to list events
        cx.subscribe(&list_state, |_this, _list_state, event, cx| {
            match event {
                crate::components::custom_list::ListEvent::Confirm(ix) => {
                    cx.emit(PickerEvent::Confirm(*ix));
                }
                crate::components::custom_list::ListEvent::Cancel => {
                    cx.emit(PickerEvent::Cancel);
                }
                _ => {}
            }
            cx.notify();
        })
        .detach();

        let item_height = px(50.0);
        let max_visible_items = 8;
        let list_height = (initial_items_count.min(max_visible_items) as f32) * item_height;

        Self {
            focus_handle: cx.focus_handle(),
            search_query: None,
            input_state,
            list_state,
            list_height,
            max_height: (max_visible_items as f32) * item_height,
            min_height: px(50.0),
        }
    }

    /// Update search results based on current query
    fn update_search(&mut self, cx: &mut Context<Self>) {
        let query = self.search_query.as_deref().unwrap_or("");

        // Update the delegate's filter
        self.list_state.update(cx, |list_state, _cx| {
            list_state
                .delegate_mut()
                .picker_delegate
                .update_filter(query);
        });

        // Update list height and selection
        let items_count = self
            .list_state
            .read(cx)
            .delegate()
            .picker_delegate
            .items_count();
        let item_height = px(50.0);
        let max_visible_items = 8;
        let visible_items = items_count.min(max_visible_items);

        self.list_height = if items_count == 0 {
            self.min_height
        } else {
            (visible_items as f32) * item_height
        };

        // Reset selection
        self.list_state.update(cx, |list_state, _cx| {
            let selection = if items_count > 0 { Some(0) } else { None };
            list_state.set_selected_index(selection);
        });
    }

    /// Get the currently selected item index
    pub fn selected_index(&self, cx: &App) -> Option<usize> {
        self.list_state.read(cx).selected_index()
    }

    /// Get the currently selected item
    pub fn selected_item(&self, cx: &App) -> Option<D::Item> {
        let list_state = self.list_state.read(cx);
        if let Some(ix) = list_state.selected_index() {
            list_state
                .delegate()
                .picker_delegate
                .filtered_items()
                .get(ix)
                .cloned()
        } else {
            None
        }
    }
}

impl<D: PickerDelegate> gpui::EventEmitter<PickerEvent> for PickerState<D> {}

impl<D: PickerDelegate> Render for PickerState<D> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("picker")
            .key_context("Picker")
            .track_focus(&self.focus_handle)
            .max_h(px(500.0))
            .bg(theme.group_box)
            .text_color(theme.group_box_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .shadow_xl()
            .on_action(cx.listener(|this, _: &NavigateUp, _window, cx| {
                this.list_state.update(cx, |state, cx| {
                    state.select_prev(_window, cx);
                });
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NavigateDown, _window, cx| {
                this.list_state.update(cx, |state, cx| {
                    state.select_next(_window, cx);
                });
                cx.notify();
            }))
            .on_action(cx.listener(|_this, _: &Confirm, _window, cx| {
                if let Some(selected_ix) = _this.list_state.read(cx).selected_index() {
                    cx.emit(PickerEvent::Confirm(selected_ix));
                }
            }))
            .on_action(cx.listener(|_this, _: &Cancel, _window, cx| {
                cx.emit(PickerEvent::Cancel);
            }))
            .child(
                div()
                    .flex_none()
                    .items_center()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(Input::new(&self.input_state).size_full()),
            )
            .child(
                v_flex()
                    .h(self.list_height)
                    .child(crate::components::custom_list::List::new(&self.list_state)),
            )
            .child(
                div()
                    .flex_none()
                    .px(px(16.0))
                    .py(px(8.0))
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        h_flex()
                            .gap_4()
                            .text_color(theme.muted_foreground)
                            .text_sm()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        h_flex()
                                            .child(Kbd::new(gpui::Keystroke::parse("up").unwrap()))
                                            .child(Kbd::new(
                                                gpui::Keystroke::parse("down").unwrap(),
                                            )),
                                    )
                                    .child(Label::new("Navigate")),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(Kbd::new(gpui::Keystroke::parse("enter").unwrap()))
                                    .child(Label::new("Select")),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(Kbd::new(gpui::Keystroke::parse("esc").unwrap()))
                                    .child(Label::new("Close")),
                            ),
                    ),
            )
    }
}

/// A generic picker component that can be configured with any PickerDelegate
#[derive(IntoElement)]
pub struct Picker<D: PickerDelegate> {
    state: Entity<PickerState<D>>,
    style: StyleRefinement,
}

impl<D: PickerDelegate> Picker<D> {
    pub fn new(state: Entity<PickerState<D>>) -> Self {
        Self {
            state,
            style: StyleRefinement::default(),
        }
    }
}

impl<D: PickerDelegate> Styled for Picker<D> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<D: PickerDelegate> RenderOnce for Picker<D> {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("picker-container")
            .size_full()
            .refine_style(&self.style)
            .child(self.state.clone())
    }
}

/// Utility trait for creating picker overlays
pub trait PickerOverlay<D: PickerDelegate> {
    fn picker_overlay(self, state: Entity<PickerState<D>>) -> impl IntoElement;
}

impl<D: PickerDelegate> PickerOverlay<D> for Picker<D> {
    fn picker_overlay(self, _state: Entity<PickerState<D>>) -> impl IntoElement {
        h_flex()
            .absolute()
            .inset_0()
            .size_full()
            .occlude()
            .justify_center()
            .child(v_flex().h_full().w(px(600.0)).pt_8().child(self))
    }
}
