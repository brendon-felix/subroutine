use std::time::Duration;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Pixels, Render,
    Window, actions, div, px,
};
use gpui::{KeyBinding, prelude::*};
use gpui_component::{ActiveTheme, Root, ThemeMode};

use crate::app::ResultExt;
use crate::components::command_palette::{
    CloseCommandPalette, Command, CommandPalette, CommandPaletteExt, CommandPaletteState,
    SelectCommand,
};
use crate::components::overlay::CloseOverlay;
use crate::components::resizable::{h_resizable, resizable_panel};
use crate::components::task_creator::TaskCreator;
use crate::stores::{DatabaseStore, DragDropStore};
use crate::themes::SwitchThemeMode;
use crate::views::{MainView, MainViewMode, RightSidebarView};

actions!(
    root_view,
    [ToggleCommandPalette, ToggleTaskCreator, ToggleSideBar]
);

pub enum Overlay {
    CommandPalette(Entity<CommandPaletteState>),
    TaskCreator(Entity<TaskCreator>),
}

pub struct RootView {
    database_store: Entity<DatabaseStore>,
    main_view: Entity<MainView>,
    right_sidebar: Entity<RightSidebarView>,
    // right_sidebar_collapsed: bool,
    focus_handle: FocusHandle,
    overlay: Option<Overlay>,
}

impl RootView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.bind_keys([
            KeyBinding::new("cmd-p", ToggleCommandPalette, None),
            KeyBinding::new("cmd-t", ToggleTaskCreator, None),
            KeyBinding::new("alt-]", ToggleSideBar, None),
        ]);

        let drag_drop_store = cx.new(|cx| DragDropStore::new(cx));
        let main_view =
            cx.new(|cx| MainView::new(database_store.clone(), drag_drop_store.clone(), window, cx));
        let right_sidebar = cx.new(|cx| {
            RightSidebarView::new(database_store.clone(), drag_drop_store.clone(), window, cx)
        });
        let focus_handle = cx.focus_handle();
        cx.focus_self(window);

        Self {
            database_store,
            main_view,
            right_sidebar,
            // right_sidebar_collapsed: false,
            focus_handle,
            overlay: None,
        }
    }

    fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::CommandPalette(_))) {
            self.overlay = None;
            cx.focus_self(window);
        } else {
            // `command_palette` is provided by `CommandPaletteExt` impl below
            let entity = self.command_palette(window, cx);
            self.overlay = Some(Overlay::CommandPalette(entity));
        }
    }

    fn toggle_task_creator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::TaskCreator(_))) {
            self.overlay = None;
            cx.focus_self(window);
        } else {
            let database_store = self.database_store.clone();
            let entity = cx.new(|cx| TaskCreator::new(database_store, window, cx));
            self.overlay = Some(Overlay::TaskCreator(entity));
        }
    }
}

impl CommandPaletteExt for RootView {
    fn commands(&self, cx: &mut Context<Self>) -> Vec<Command> {
        vec![
            Command::new("home-view", "Switch to Home View").on_select({
                let entity = self.main_view.clone();
                move |_window, cx| {
                    cx.update_entity(&entity, |main_view, cx| {
                        main_view.set_mode(MainViewMode::Home, cx);
                    });
                }
            }),
            // Command::new("task-view", "Switch to Task List View").on_select({
            //     let entity = self.main_view.clone();
            //     move |_window, cx| {
            //         cx.update_entity(&entity, |main_view, cx| {
            //             main_view.set_mode(MainViewMode::TaskList, cx);
            //         });
            //     }
            // }),
            // Command::new("test-view", "Switch to Test View").on_select({
            //     let entity = self.main_view.clone();
            //     move |_window, cx| {
            //         cx.update_entity(&entity, |main_view, cx| {
            //             main_view.set_mode(MainViewMode::Test, cx);
            //         });
            //     }
            // }),
            Command::new("action-editor", "Switch to Action Editor").on_select({
                let entity = self.main_view.clone();
                move |_window, cx| {
                    cx.update_entity(&entity, |main_view, cx| {
                        main_view.set_mode(MainViewMode::ActionEditor, cx);
                    });
                }
            }),
            Command::new("action-list", "Switch to Action List").on_select({
                let entity = self.main_view.clone();
                move |_window, cx| {
                    cx.update_entity(&entity, |main_view, cx| {
                        main_view.set_mode(MainViewMode::ActionList, cx);
                    });
                }
            }),
            // Command::new("refresh-tasks", "Refresh task list").on_select({
            //     let entity = self.task_store.clone();
            //     move |_window, cx| {
            //         cx.update_entity(&entity, |tasks, cx| {
            //             tasks.refresh_tasks(cx).log_err();
            //         });
            //     }
            // }),
            Command::new("edit-copy", "Copy")
                .description("Copy selected text")
                .shortcut("Cmd+C")
                .on_select(|_window, _cx| {}),
            Command::new("edit-paste", "Paste")
                .description("Paste from clipboard")
                .shortcut("Cmd+V")
                .on_select(|_window, _cx| {}),
            Command::new("edit-find", "Find")
                .description("Search in current file")
                .shortcut("Cmd+F")
                .on_select(|_window, _cx| {}),
            Command::new("view-toggle", "Toggle Right Sidebar")
                .description("Show or hide the sidebar")
                .shortcut("alt-]")
                .on_select(|window, cx| {
                    window.dispatch_action(Box::new(ToggleSideBar), cx);
                }),
            Command::new("app-quit", "Quit Application")
                .description("Exit the application")
                .shortcut("cmd-q")
                .on_select(|_window, cx| {
                    cx.quit();
                }),
            {
                if cx.theme().mode == ThemeMode::Dark {
                    Command::new("light-mode", "Switch to Light Mode")
                        .description("Switch between light and dark themes")
                        .on_select(|window, cx| {
                            let action = SwitchThemeMode(ThemeMode::Light);
                            window.dispatch_action(Box::new(action), cx);
                        })
                } else {
                    Command::new("dark-mode", "Switch to Dark Mode")
                        .description("Switch between light and dark themes")
                        .on_select(|window, cx| {
                            let action = SwitchThemeMode(ThemeMode::Dark);
                            window.dispatch_action(Box::new(action), cx);
                        })
                }
            },
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
        let right_sidebar_collapsed = self.right_sidebar.read(cx).is_collapsed();

        let main_area = h_resizable("root-layout")
            .on_resize(cx.listener(|_this, _state, _window, cx| {
                cx.notify();
            }))
            .child(
                // Center content panel
                resizable_panel().size_range(px(380.0)..Pixels::MAX).child(
                    div()
                        .size_full()
                        .p_2()
                        .when(!right_sidebar_collapsed, |div| div.pr_1())
                        .bg(cx.theme().secondary)
                        .child(
                            div()
                                .size_full()
                                .bg(cx.theme().background)
                                .rounded_lg()
                                .child(self.main_view.clone()),
                        ),
                ),
            )
            .child(
                // Right sidebar panel
                resizable_panel()
                    .size(px(250.0))
                    .size_range(px(200.0)..px(500.0))
                    .visible(!right_sidebar_collapsed)
                    .child(self.right_sidebar.clone()),
            );

        let content = div().size_full().flex().child(
            div()
                .size_full()
                .flex()
                .track_focus(&self.focus_handle)
                // Sidebar toggle
                .on_action(
                    cx.listener(|this: &mut RootView, _: &ToggleSideBar, window, cx| {
                        let collapsed = this
                            .right_sidebar
                            .update(cx, |sidebar, cx| sidebar.toggle_collapsed(cx));
                        if collapsed {
                            cx.focus_self(window);
                        }
                        cx.notify();
                    }),
                )
                // Overlay toggles
                .on_action(cx.listener(|this, _: &ToggleCommandPalette, window, cx| {
                    this.toggle_command_palette(window, cx);
                }))
                .on_action(cx.listener(|this, _: &ToggleTaskCreator, window, cx| {
                    this.toggle_task_creator(window, cx);
                }))
                // Shared overlay close
                .on_action(cx.listener(|this, _: &CloseOverlay, window, cx| {
                    this.overlay = None;
                    cx.focus_self(window);
                    cx.notify();
                }))
                // Backwards compatibility for old close action
                .on_action(cx.listener(|this, _: &CloseCommandPalette, window, cx| {
                    this.overlay = None;
                    cx.focus_self(window);
                    cx.notify();
                }))
                // When SelectCommand is dispatched, forward it to the active command palette
                .on_action(cx.listener(|this, _: &SelectCommand, window, cx| {
                    if let Some(Overlay::CommandPalette(entity)) = &this.overlay {
                        let executed = cx.update_entity(
                            entity,
                            |cmd_palette: &mut CommandPaletteState, cx| {
                                cmd_palette.execute_selected(window, cx)
                            },
                        );
                        if executed {
                            this.overlay = None;
                            cx.focus_self(window);
                        }
                    }
                    cx.notify();
                }))
                .child(main_area)
                // Render overlay if present
                .when_some(self.overlay.as_ref(), |content, overlay| match overlay {
                    Overlay::CommandPalette(cmd) => content.child(CommandPalette::new(cmd.clone())),
                    Overlay::TaskCreator(entity) => content.child(entity.clone()),
                }),
        );

        content
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
