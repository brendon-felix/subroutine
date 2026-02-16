use gpui::{
    App, Context, EdgesRefinement, IntoElement, Keystroke, ParentElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{h_flex, kbd::Kbd, label::Label};

use crate::components::{
    command_palette::Command,
    custom_list::{ListDelegate, ListItem, ListOptions, ListState},
};

pub struct CommandList {
    pub commands: Vec<Command>,
    pub filtered_commands: Vec<Command>,
    pub selected_ix: Option<usize>,
    pub options: ListOptions,
}

impl CommandList {
    pub fn new(commands: Vec<Command>) -> Self {
        let options = ListOptions {
            item_size: px(48.0),
            gap: px(4.0),
            scrollbar_visible: true,
            paddings: EdgesRefinement {
                top: Some(px(12.)),
                right: Some(px(12.)),
                bottom: Some(px(12.)),
                left: Some(px(12.)),
            },
        };

        Self {
            commands: commands.clone(),
            filtered_commands: commands,
            selected_ix: None,
            options,
        }
    }

    pub fn commands(&self) -> &Vec<Command> {
        &self.commands
    }

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
        ListItem::new(ix)
            .rounded_sm()
            .child(
                h_flex().size_full().items_center().justify_center().child(
                    h_flex()
                        .items_center()
                        .gap_3()
                        .min_w_0()
                        .flex_1()
                        .justify_between()
                        .child(
                            h_flex()
                                .gap_3()
                                .when_some(item.icon.as_ref(), |this, icon| {
                                    this.child(icon.clone())
                                })
                                .child(Label::new(&item.label).truncate()),
                        )
                        .when_some(item.shortcut.as_ref(), |this, shortcut| {
                            if let Some(keystroke) = Keystroke::parse(shortcut).ok() {
                                this.child(Kbd::new(keystroke).text_sm())
                            } else {
                                this
                            }
                        }),
                ),
            )
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
