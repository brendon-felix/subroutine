use gpui::{
    App, Entity, FocusHandle, Focusable, KeyBinding, Keystroke, Pixels, StyleRefinement, Window,
    actions, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, StyledExt, h_flex,
    input::{Input, InputEvent, InputState},
    kbd::Kbd,
    label::Label,
    v_flex,
};

mod command;
mod list;

pub use command::*;
pub use list::*;

use crate::components::custom_list::{List, ListEvent, ListState};

actions!(
    command_palette,
    [NavigateUp, NavigateDown, SelectCommand, CloseCommandPalette]
);

pub fn init(cx: &mut App) {
    let context: Option<&str> = Some("CommandPalette");
    cx.bind_keys([
        KeyBinding::new("escape", CloseCommandPalette, context),
        KeyBinding::new("enter", SelectCommand, context),
        KeyBinding::new("up", NavigateUp, context),
        KeyBinding::new("down", NavigateDown, context),
    ]);
}

#[allow(unused)]
pub struct CommandPaletteState {
    pub focus_handle: FocusHandle,
    pub search_query: Option<String>,
    pub input_state: Entity<InputState>,
    pub list_state: Entity<ListState<CommandList>>,
    pub list_height: Pixels,
    // pub recent_commands: Vec<SharedString>,
}

impl Focusable for CommandPaletteState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input_state.focus_handle(cx)
    }
}

impl CommandPaletteState {
    pub fn new(commands: Vec<Command>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("Type a command or search...");
            state.focus(window, cx);
            state
        });

        cx.subscribe(&input_state, |this, _input, event, cx| {
            match event {
                InputEvent::Change => {
                    this.search_query = Some(this.input_state.read(cx).value().to_string());
                    this.update_search(cx);
                }
                _ => {}
            }
            cx.notify();
        })
        .detach();

        let list_state = cx.new(|cx| {
            let mut state = ListState::new(CommandList::new(commands.clone()), window, cx);
            state.set_selected_index(Some(0));
            state
        });
        cx.subscribe(&list_state, |_this, list_state, event: &ListEvent, cx| {
            match event {
                // ListEvent::Select(ix) => {
                //     if let Some(_selected_cmd) = list_state
                //         .read(cx)
                //         .delegate()
                //         .filtered_commands()
                //         .get(*ix)
                //         .cloned()
                //     {
                //         println!("Selected command: {}", selected_cmd.name);
                //     }
                // }
                ListEvent::Confirm(ix) => {
                    if let Some(_selected_cmd) = list_state
                        .read(cx)
                        .delegate()
                        .filtered_commands()
                        .get(*ix)
                        .cloned()
                    {
                        cx.dispatch_action(&SelectCommand);
                    }
                }
                // ListEvent::Cancel => {
                //     println!("Command selection cancelled");
                // }
                _ => {}
            }
            cx.notify();
        })
        .detach();

        let list_height = commands.len() as f32 * px(50.);

        Self {
            focus_handle: cx.focus_handle(),
            search_query: None,
            input_state,
            list_state,
            list_height,
            // recent_commands: Vec::new(),
        }
    }

    pub fn update_search(&mut self, cx: &mut Context<Self>) {
        if let Some(query) = self.search_query.as_ref() {
            // println!("Updating search with query: {}", query);
            let list_state = self.list_state.read(cx);
            let commands = list_state.delegate().commands().clone();

            let filtered_commands = if query.is_empty() {
                commands
            } else {
                let mut matches: Vec<(Command, i32)> = commands
                    .iter()
                    .filter(|cmd| cmd.matches(&query))
                    .map(|cmd| (cmd.clone(), cmd.match_score(&query)))
                    .collect();
                matches.sort_by(|a, b| b.1.cmp(&a.1));
                matches.into_iter().map(|(cmd, _)| cmd).collect()
            };

            self.list_height = filtered_commands.len() as f32 * px(50.);

            self.list_state.update(cx, |list_state, _cx| {
                let delegate = list_state.delegate_mut();
                let ix = if filtered_commands.is_empty() {
                    None
                } else {
                    Some(0)
                };
                delegate.set_filtered_commands(filtered_commands);
                list_state.set_selected_index(ix);
            });
        }
    }

    pub fn execute_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let list_state = self.list_state.read(cx);
        if let Some(selected_ix) = list_state.selected_index() {
            if let Some(selected_cmd) = list_state
                .delegate()
                .filtered_commands()
                .get(selected_ix)
                .cloned()
            {
                println!("Executing command: {}", selected_cmd.name);
                if let Some(handler) = &selected_cmd.on_select {
                    handler(window, cx);
                    return true;
                }
            }
        }
        false
    }
}

impl Render for CommandPaletteState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // Build inner dialog card and let the shared overlay shell handle backdrop,
        // occlusion, centering and shared key context/close behaviour.
        let inner = v_flex() // command palette container (inner)
            .key_context("CommandPalette")
            .track_focus(&self.focus_handle)
            // .w(px(640.0))
            .w_128()
            .max_h(px(402.0))
            .bg(theme.group_box)
            .text_color(theme.group_box_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .shadow_xl()
            .on_any_mouse_down(|_event, _window, cx| {
                cx.stop_propagation();
            })
            .child(
                div()
                    .flex_none()
                    // .items_center()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(Input::new(&self.input_state).size_full()), // .font_family("monospace"),
            )
            .child(
                v_flex()
                    .h(self.list_height)
                    .child(List::new(&self.list_state)),
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
                            .w_full()
                            .gap_4()
                            .text_color(theme.muted_foreground)
                            .text_sm()
                            .justify_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        h_flex()
                                            .child(
                                                Kbd::new(Keystroke::parse("up").unwrap())
                                                    .pr_0()
                                                    .rounded_r_none(),
                                            )
                                            .child(
                                                Kbd::new(Keystroke::parse("down").unwrap())
                                                    .pl_0()
                                                    .rounded_l_none(),
                                            ),
                                    )
                                    .child(Label::new("Navigate")),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(Kbd::new(Keystroke::parse("enter").unwrap()))
                                    .child(Label::new("Select")),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(Kbd::new(Keystroke::parse("esc").unwrap()))
                                    .child(Label::new("Close")),
                            ),
                    ),
            )
            .on_action(cx.listener(|this, NavigateUp, window, cx| {
                this.list_state.update(cx, |list_state, cx| {
                    list_state.select_prev(window, cx);
                });
                cx.notify();
            }))
            .on_action(cx.listener(|this, NavigateDown, window, cx| {
                this.list_state.update(cx, |list_state, cx| {
                    list_state.select_next(window, cx);
                });
                cx.notify();
            }));

        // Use a div wrapper so we can attach action handlers (InteractiveElement) while
        // reusing the overlay shell for visual chrome and backdrop behaviour.
        crate::components::overlay::shell(theme, inner)
    }
}

// impl EventEmitter<CommandPaletteEvent> for CommandPaletteState {}

#[derive(IntoElement)]
pub struct CommandPalette {
    state: Entity<CommandPaletteState>,
    // on_close: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    style: StyleRefinement,
}

impl CommandPalette {
    pub fn new(
        state: Entity<CommandPaletteState>,
        // _window: &mut Window,
        // cx: &mut Context<Self>,
    ) -> Self {
        // cx.bind_keys([
        //     // KeyBinding::new("up", NavigateUp, None),
        //     // KeyBinding::new("down", NavigateDown, None),
        //     KeyBinding::new("enter", SelectCommand, None),
        //     KeyBinding::new("escape", CloseCommandPalette, None),
        // ]);

        Self {
            state,
            // on_close: None,
            style: StyleRefinement::default(),
        }
    }

    // pub fn on_close<F>(mut self, handler: F) -> Self
    // where
    //     F: Fn(&mut Window, &mut App) + 'static,
    // {
    //     self.on_close = Some(Rc::new(handler));
    //     self
    // }
}

impl Styled for CommandPalette {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for CommandPalette {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("command-palette")
            .size_full()
            .refine_style(&self.style)
            .absolute()
            .inset_0()
            .child(self.state.clone())
    }
}

pub trait CommandPaletteExt: Sized {
    fn commands(&self, cx: &mut Context<Self>) -> Vec<Command>;

    fn command_palette(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<CommandPaletteState> {
        let commands = self.commands(cx);
        cx.new(|cx| CommandPaletteState::new(commands, window, cx))
    }
}
