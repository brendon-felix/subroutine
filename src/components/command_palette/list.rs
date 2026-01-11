use gpui::{
    App, Context, IntoElement, Keystroke, ParentElement, Styled, Window, div,
    prelude::FluentBuilder,
};
use gpui_component::{StyledExt, h_flex, kbd::Kbd, label::Label};
// use gpui_component::list::{ListDelegate, ListState};

use crate::components::{
    command_palette::Command,
    custom_list::{ListDelegate, ListItem, ListState},
};

pub struct CommandList {
    commands: Vec<Command>,
    filtered_commands: Vec<Command>,
    selected_ix: Option<usize>,
}

impl CommandList {
    pub fn new(commands: Vec<Command>) -> Self {
        // let items = (1..=200).map(|i| format!("This is item {}", i)).collect();
        Self {
            commands: commands.clone(),
            filtered_commands: commands,
            selected_ix: None,
        }
    }

    pub fn commands(&self) -> &Vec<Command> {
        &self.commands
    }

    // pub fn update_commands(&mut self, commands: Vec<Command>) {
    //     self.commands = commands;
    //     self.filtered_commands = self.commands.clone();
    // }

    // pub fn filter_commands<F>(&mut self, filter_fn: F)
    // where
    //     F: Fn(&Command) -> bool,
    // {
    //     self.filtered_commands = self
    //         .commands
    //         .iter()
    //         .cloned()
    //         .filter(|cmd| filter_fn(cmd))
    //         .collect();
    // }

    pub fn set_filtered_commands(&mut self, commands: Vec<Command>) {
        self.filtered_commands = commands;
        self.selected_ix = self.filtered_commands.first().map(|_| 0);
    }

    pub fn filtered_commands(&self) -> &Vec<Command> {
        &self.filtered_commands
    }
}

impl ListDelegate for CommandList {
    fn render_item(
        &mut self,
        ix: usize,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let item = self.filtered_commands.get(ix)?;
        // let is_selected = Some(ix) == self.selected_ix;
        ListItem::new(ix)
            .rounded_sm()
            .child(
                h_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    // .py_4()
                    // .h_full()
                    // .bg(gpui::rgb(0xFF0000))
                    .px_4()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .min_w_0()
                            .flex_1()
                            .justify_between()
                            .child(Label::new(&item.name).font_semibold().truncate())
                            .when_some(item.shortcut.as_ref(), |this, shortcut| {
                                // if let Some(parsed) = Keystroke::parse(shortcut).ok() {
                                //     this.child(Kbd::new(parsed).text_sm())
                                // } else {
                                //     this
                                // }
                                let keystroke =
                                    Keystroke::parse(shortcut).unwrap_or(Keystroke::default());
                                this.child(Kbd::new(keystroke).text_sm())
                            }),
                    ),
            )
            // .selected(is_selected)
            // .on_click({
            //     cx.listener(move |list_state, _event, _window, cx| {
            //         cx.notify();
            //     })
            // })
            .into()
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
    }

    fn items_count(&self, _cx: &App) -> usize {
        self.filtered_commands.len()
    }

    fn set_selected_index(
        &mut self,
        ix: Option<usize>,
        // _window: &mut Window,
        // _cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_ix = ix;
    }
}
