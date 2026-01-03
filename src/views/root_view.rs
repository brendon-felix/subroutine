use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Subscription,
    Window, actions, div, px,
};
use gpui::{KeyBinding, prelude::*};
use gpui_component::Root;
use gpui_component::resizable::{h_resizable, resizable_panel};
use gpui_component::sidebar::SidebarToggleButton;
use gpui_component::{h_flex, v_flex};

// use crate::app::ToggleCommandPalette;
use crate::components::command_palette::{
    CloseCommandPalette, Command, CommandPalette, CommandPaletteState, SelectCommand,
};
use crate::stores::task_store::ApiError;
// use crate::stores::ui_store::{TaskSelected, ViewChanged};
use crate::stores::TaskStore;
use crate::views::{RightSidebarView, TaskListView};

actions!(root_view, [ToggleCommandPalette, ToggleSideBar]);

pub struct RootView {
    // ui_store: Entity<UiStateStore>,
    // focus_view: Entity<FocusMode>,
    // left_sidebar: Entity<LeftSidebarView>,
    task_list: Entity<TaskListView>,
    right_sidebar: Entity<RightSidebarView>,
    focus_handle: FocusHandle,
    cmd_palette: Option<Entity<CommandPaletteState>>,
    _subscriptions: Vec<Subscription>,
}

impl RootView {
    pub fn new(
        task_store: Entity<TaskStore>,
        // ui_store: Entity<UiStateStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // let left_sidebar = cx.new(|cx| LeftSidebarView::new(ui_store.clone(), cx));
        // let task_list = cx.new(|cx| TaskListView::new(task_store.clone(), ui_store.clone(), cx));
        let task_list = cx.new(|cx| TaskListView::new(task_store.clone(), cx));
        // let right_sidebar = cx.new(|cx| RightSidebarView::new(ui_store.clone(), window, cx));
        let right_sidebar = cx.new(|cx| RightSidebarView::new(window, cx));

        let focus_handle = cx.focus_handle();
        let mut subscriptions = Vec::new();

        cx.bind_keys([
            KeyBinding::new("cmd-p", ToggleCommandPalette, None),
            KeyBinding::new("alt-]", ToggleSideBar, None),
        ]);

        subscriptions.push(cx.subscribe(
            &task_store,
            |_this, _task_store, event: &ApiError, cx| {
                eprintln!("API Error: {}", event.message);
                cx.notify();
            },
        ));

        // subscriptions.push(
        //     cx.subscribe(&left_sidebar, |_this, _left_sidebar, _event: &(), cx| {
        //         cx.notify();
        //     }),
        // );

        // subscriptions.push(cx.subscribe(
        //     &right_sidebar,
        //     |_this, _right_sidebar, _event: &(), cx| {
        //         // cx.notify();
        //     },
        // ));

        // subscriptions.push(cx.subscribe(
        //     &ui_store,
        //     |this, _ui_store, _event: &TaskSelected, cx| {
        //         // Auto-open right sidebar when task is selected
        //         this.right_sidebar.update(cx, |sidebar, cx| {
        //             if sidebar.is_collapsed() {
        //                 sidebar.set_collapsed(false, cx);
        //             }
        //         });
        //         cx.notify();
        //     },
        // ));

        // subscriptions.push(
        //     cx.subscribe(&ui_store, |this, _ui_store, _event: &ViewChanged, cx| {
        //         // When view changes, update task list and trigger re-render
        //         cx.notify();
        //         this.task_list.update(cx, |_task_list, cx| {
        //             cx.notify();
        //         });
        //     }),
        // );

        Self {
            // ui_store,
            // left_sidebar,
            task_list,
            right_sidebar,
            focus_handle,
            cmd_palette: None,
            _subscriptions: subscriptions,
        }
    }

    fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        println!("Toggling command palette");
        if self.cmd_palette.is_some() {
            self.cmd_palette = None;
            window.focus(&self.focus_handle);
        } else {
            let commands = self.create_commands(cx);
            // let entity = cx.entity().clone();
            let state = cx.new(|cx| CommandPaletteState::new(commands, window, cx));
            // let cmd_palette = cx.new(|cx| {
            //     CommandPalette::new(state, window, cx).on_close(move |_, cx| {
            //         cx.update_entity(&entity, |this: &mut CommandPaletteStory, cx| {
            //             this.cmd_palette = None;
            //             cx.notify();
            //             cx.activate(true);
            //         });
            //     })
            // });
            self.cmd_palette = Some(state);
        }
    }

    fn create_commands(&self, _cx: &mut Context<Self>) -> Vec<Command> {
        // let entity = cx.entity().clone();
        vec![
            // Command::new("file-new", "New File")
            //     .description("Create a new file")
            //     // .icon("file-plus")
            //     .shortcut("Cmd+N")
            //     .on_select(|_window, _cx| {}),
            // Command::new("file-open", "Open File")
            //     .description("Open an existing file")
            //     // .icon("folder-open")
            //     .shortcut("Cmd+O")
            //     .on_select(|_window, _cx| {}),
            // Command::new("file-save", "Save File")
            //     .description("Save the current file")
            //     // .icon("save")
            //     .shortcut("Cmd+S")
            //     .on_select(|_window, _cx| {}),
            // Command::new("edit-copy", "Copy")
            //     .description("Copy selected text")
            //     // .icon("copy")
            //     .shortcut("Cmd+C")
            //     .on_select(|_window, _cx| {}),
            // Command::new("edit-paste", "Paste")
            //     .description("Paste from clipboard")
            //     // .icon("clipboard")
            //     .shortcut("Cmd+V")
            //     .on_select(|_window, _cx| {}),
            // Command::new("edit-find", "Find")
            //     .description("Search in current file")
            //     // .icon("search")
            //     .shortcut("Cmd+F")
            //     .on_select(|_window, _cx| {}),
            Command::new("view-toggle", "Toggle Right Sidebar")
                .description("Show or hide the sidebar")
                // .icon("sidebar")
                .shortcut("alt-]")
                .on_select({
                    let entity = self.right_sidebar.clone();
                    move |_, cx| {
                        cx.update_entity(&entity, |sidebar, cx| {
                            sidebar.toggle_collapsed(cx);
                        });
                    }
                }),
            Command::new("app-quit", "Quit Application")
                .description("Exit the application")
                // .icon("power")
                .shortcut("cmd-q")
                .on_select(|_window, cx| {
                    cx.quit();
                }),
        ]
    }
}

impl Focusable for RootView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<()> for RootView {}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // let ui_state = self.ui_store.read(cx);
        // let has_selected_task = ui_state.selected_task.is_some();
        // let left_sidebar_collapsed = self.left_sidebar.read(cx).is_collapsed();
        let right_sidebar_collapsed = self.right_sidebar.read(cx).is_collapsed();

        let content = div()
            .size_full()
            // .bg(rgb(0x191919))
            // .text_color(rgb(0xC8C8C8))
            .flex()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(
                |this: &mut RootView, _action: &ToggleSideBar, _window, cx| {
                    this.right_sidebar.update(cx, |sidebar, cx| {
                        sidebar.toggle_collapsed(cx);
                    });
                },
            ))
            .on_action(cx.listener(
                |this: &mut RootView, _action: &ToggleCommandPalette, window, cx| {
                    this.toggle_command_palette(window, cx);
                },
            ))
            .on_action(cx.listener(
                |this: &mut RootView, _action: &CloseCommandPalette, window, cx| {
                    this.cmd_palette = None;
                    // this.focus_handle.focus(window);
                    window.focus(&this.focus_handle);
                    cx.notify();
                },
            ))
            .on_action(
                cx.listener(|this: &mut RootView, _action: &SelectCommand, window, cx| {
                    // println!("Command selected, closing palette.");
                    if let Some(cmd_palette) = &this.cmd_palette {
                        let executed = cx.update_entity(
                            cmd_palette,
                            |cmd_palette: &mut CommandPaletteState, cx| {
                                cmd_palette.execute_selected(window, cx)
                            },
                        );
                        if executed {
                            this.cmd_palette = None;
                            // this.focus_handle.focus(window);
                            window.focus(&this.focus_handle);
                        }
                    }
                    cx.notify();
                }),
            )
            .child(
                // Main content area with left sidebar overlay
                div()
                    .size_full()
                    .flex()
                    // .child(self.left_sidebar.clone())
                    .child(
                        // Two-panel resizable layout for center and right
                        h_resizable("main-layout")
                            .on_resize(cx.listener(|_this, _state, _window, cx| {
                                cx.notify();
                            }))
                            .child(
                                // Center content panel
                                resizable_panel().child(
                                    v_flex()
                                        .size_full()
                                        .child(
                                            // Header area with toggle buttons
                                            h_flex()
                                                .p_2()
                                                .gap_2()
                                                // .child(
                                                //     SidebarToggleButton::left()
                                                //         .collapsed(left_sidebar_collapsed)
                                                //         .on_click(cx.listener(
                                                //             |this, _event, _window, cx| {
                                                //                 this.left_sidebar.update(
                                                //                     cx,
                                                //                     |sidebar, cx| {
                                                //                         sidebar
                                                //                             .toggle_collapsed(cx);
                                                //                     },
                                                //                 );
                                                //             },
                                                //         )),
                                                // )
                                                .child(div().flex_1()) // Spacer
                                                .child(
                                                    SidebarToggleButton::right()
                                                        .collapsed(right_sidebar_collapsed)
                                                        .on_click(cx.listener(
                                                            |this, _event, _window, cx| {
                                                                this.right_sidebar.update(
                                                                    cx,
                                                                    |sidebar, cx| {
                                                                        sidebar
                                                                            .toggle_collapsed(cx);
                                                                    },
                                                                );
                                                            },
                                                        )),
                                                ),
                                        )
                                        .child(
                                            // Task list content
                                            div()
                                                .flex_1()
                                                .overflow_hidden()
                                                .child(self.task_list.clone()),
                                        ),
                                ),
                            )
                            .child(
                                // Right sidebar panel
                                resizable_panel()
                                    .size(px(350.0))
                                    .size_range(px(250.0)..px(500.0))
                                    // .visible(!right_sidebar_collapsed && has_selected_task)
                                    .visible(!right_sidebar_collapsed)
                                    .child(self.right_sidebar.clone()),
                            ),
                    ),
            )
            .when(self.cmd_palette.is_some(), |content| {
                content.child(CommandPalette::new(self.cmd_palette.clone().unwrap()))
            });

        content
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
