use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use gpui::{
    App, Entity, FocusHandle, Focusable, KeyBinding, SharedString, StyleRefinement, Window,
    actions, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, StyledExt,
    input::{Input, InputEvent, InputState},
    label::Label,
    v_flex,
};

mod command;
mod list;

pub use command::*;
pub use list::*;

use crate::components::{
    custom_list::{List, ListEvent, ListOptions, ListState},
    popover::popover,
};

// pub fn kbd(keystroke: &'static str) -> Kbd {
//     Kbd::new(Keystroke::parse(keystroke).unwrap())
// }

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

pub struct CommandPaletteOptions {
    pub max_visible_items: usize,
    // pub item_height: Pixels,
    pub fuzzy_search: bool,
    // pub show_recent_commands: bool,
    // pub placeholder_text: String,
    // pub execute_on_select: bool,
    // pub close_on_select: bool,
}

impl Default for CommandPaletteOptions {
    fn default() -> Self {
        Self {
            max_visible_items: 7,
            // item_height: px(48.0),
            fuzzy_search: true,
            // show_recent_commands: true,
            // placeholder_text: "Type a command or search...".to_string(),
            // execute_on_select: true,
            // close_on_select: true,
        }
    }
}

#[allow(unused)]
pub struct CommandPaletteState {
    pub focus_handle: FocusHandle,
    pub search_query: Option<String>,
    pub input_state: Entity<InputState>,
    pub list_state: Entity<ListState<CommandList>>,
    // pub recent_commands: Vec<SharedString>,
    pub options: CommandPaletteOptions,
}

impl Focusable for CommandPaletteState {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input_state.focus_handle(cx)
    }
}

impl CommandPaletteState {
    pub fn new(commands: Vec<Command>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("Search or type a command...");
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
            let cmd_list = CommandList::new(commands.clone());
            let paddings = cmd_list.options.paddings.clone();
            let item_size = cmd_list.options.item_size;
            let gap = cmd_list.options.gap;
            let mut state = ListState::new(cmd_list, window, cx).options(ListOptions {
                paddings,
                item_size,
                gap,
                ..Default::default()
            });
            state.set_selected_index(Some(0));
            state
        });
        cx.subscribe(&list_state, |_this, list_state, event: &ListEvent, cx| {
            match event {
                ListEvent::Select(ix) => {
                    if let Some(selected_cmd) = list_state
                        .read(cx)
                        .delegate()
                        .filtered_commands()
                        .get(*ix)
                        .cloned()
                    {
                        println!("Selected command: {}", selected_cmd.label);
                    }
                }
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
                _ => {}
            }
            cx.notify();
        })
        .detach();

        let options = CommandPaletteOptions::default();

        Self {
            focus_handle: cx.focus_handle(),
            search_query: None,
            input_state,
            list_state,
            // recent_commands: Vec::new(),
            options,
        }
    }

    // pub fn options(mut self, options: CommandPaletteOptions) -> Self {
    //     self.options = options;
    //     self
    // }

    pub fn update_search(&mut self, cx: &mut Context<Self>) {
        if let Some(query) = self.search_query.as_ref() {
            // println!("Updating search with query: {}", query);
            let list_state = self.list_state.read(cx);
            let commands = list_state.delegate().commands().clone();

            // let filtered_commands = if query.is_empty() {
            //     commands
            // } else {
            //     let mut matches: Vec<(Command, i32)> = commands
            //         .iter()
            //         .filter(|cmd| cmd.matches(&query))
            //         .map(|cmd| (cmd.clone(), cmd.match_score(&query)))
            //         .collect();
            //     matches.sort_by(|a, b| b.1.cmp(&a.1));
            //     matches.into_iter().map(|(cmd, _)| cmd).collect()
            // };

            let score_fn = if self.options.fuzzy_search {
                |cmd: &Command, query: &str| {
                    let query = query.to_lowercase();
                    let label = cmd.label.to_lowercase();
                    let search_terms: Vec<SharedString> = cmd
                        .search_terms
                        .iter()
                        .map(|s| s.to_lowercase().into())
                        .collect();

                    let matcher = SkimMatcherV2::default();

                    let label_score = matcher.fuzzy_match(&label, &query).unwrap_or(-1000);

                    let terms_score = search_terms
                        .iter()
                        .filter_map(|term| matcher.fuzzy_match(term, &query))
                        .max()
                        .unwrap_or(-1000);

                    (label_score * 2).max(terms_score)
                }
            } else {
                |cmd: &Command, query: &str| {
                    let query = query.to_lowercase();
                    let label = cmd.label.to_lowercase();
                    let search_terms: Vec<SharedString> = cmd
                        .search_terms
                        .iter()
                        .map(|s| s.to_lowercase().into())
                        .collect();

                    let label_score = if label.contains(&query) { 100 } else { -1000 };

                    let terms_score = if search_terms.iter().any(|term| term.contains(&query)) {
                        100
                    } else {
                        -1000
                    };

                    label_score.max(terms_score)
                }
            };

            let filtered_commands = if query.is_empty() {
                commands
            } else {
                let mut matches: Vec<(Command, i64)> = commands
                    .iter()
                    .map(|cmd| (cmd.clone(), score_fn(cmd, query)))
                    .filter(|(_cmd, score)| *score > -1000)
                    .collect();
                matches.sort_by(|a, b| b.1.cmp(&a.1));
                matches.into_iter().map(|(cmd, _)| cmd).collect()
            };

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
                println!("Executing command: {}", selected_cmd.label);
                if let Some(handler) = &selected_cmd.on_select {
                    handler(window, cx);
                    return true;
                }
            }
        }
        false
    }

    // pub fn render_help_bar(&mut self, cx: &Context<Self>) -> impl IntoElement {
    //     let theme = cx.theme();

    //     div()
    //         .flex_none()
    //         .px_4()
    //         .py_2()
    //         .border_t_1()
    //         .border_color(theme.border)
    //         .child(
    //             h_flex()
    //                 .w_full()
    //                 .gap_4()
    //                 .text_xs()
    //                 .justify_center()
    //                 .child(
    //                     h_flex()
    //                         .gap_2()
    //                         .child(
    //                             h_flex()
    //                                 .child(kbd("up").pr_0().rounded_r_none())
    //                                 .child(kbd("down").pl_0().rounded_l_none()),
    //                         )
    //                         .child(Label::new("to navigate").text_color(theme.muted_foreground)),
    //                 )
    //                 .child(
    //                     h_flex()
    //                         .gap_2()
    //                         .child(kbd("enter"))
    //                         .child(Label::new("to use").text_color(theme.muted_foreground)),
    //                 )
    //                 .child(
    //                     h_flex()
    //                         .gap_2()
    //                         .child(kbd("esc"))
    //                         .child(Label::new("to dismiss").text_color(theme.muted_foreground)),
    //                 ),
    //         )
    // }
}

impl Render for CommandPaletteState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        let num_items = self
            .list_state
            .read(cx)
            .delegate()
            .filtered_commands()
            .len();

        let num_visible = num_items.min(self.options.max_visible_items);
        let delegate = self.list_state.read(cx).delegate();
        let item_height = delegate.options.item_size;
        let gap = delegate.options.gap;
        let paddings = self.list_state.read(cx).delegate().options.paddings.clone();
        let p_top = paddings.top.unwrap_or_default();
        let p_bottom = paddings.bottom.unwrap_or_default();
        let list_height = num_visible as f32 * item_height
            + p_top
            + p_bottom
            + (num_visible.saturating_sub(1) * gap);

        let inner = v_flex() // command palette container (inner)
            .key_context("CommandPalette")
            .track_focus(&self.focus_handle)
            .w(px(600.0))
            .bg(theme.group_box)
            .text_color(theme.group_box_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .shadow_xl()
            .occlude()
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
            }))
            .child(
                div()
                    .px_3()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Input::new(&self.input_state).size_full()),
            )
            .when_else(
                // if
                num_items > 0,
                // then
                |this| {
                    this.child(
                        v_flex()
                            .bg(theme.group_box)
                            .flex_basis(list_height)
                            .child(List::new(&self.list_state)),
                    )
                },
                // else
                |this| {
                    this.child(
                        div()
                            .w_full()
                            .flex_basis(item_height + p_top + p_bottom)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(theme.muted_foreground)
                            .child(Label::new("No commands found")),
                    )
                },
            );
        // .child(self.render_help_bar(cx));

        // Use a div wrapper so we can attach action handlers (InteractiveElement) while
        // reusing the overlay shell for visual chrome and backdrop behaviour.
        popover(inner, cx)
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
