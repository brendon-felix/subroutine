use std::time::Duration;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Pixels, Render,
    Window, actions, div, px,
};
use gpui::{KeyBinding, prelude::*};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::notification::{Notification, NotificationType};
use gpui_component::{ActiveTheme, IconName, Root, ThemeMode, WindowExt};

use crate::app::ResultExt;
use crate::components::command_palette::{
    CloseCommandPalette, Command, CommandPalette, CommandPaletteExt, CommandPaletteState,
    SelectCommand,
};
use crate::components::popover::CloseOverlay;
use crate::components::resizable::{h_resizable, resizable_panel};
use crate::components::task_creator::TaskCreator;
use crate::stores::{DatabaseStore, DragDropStore};
use crate::themes::SwitchThemeMode;
use crate::views::{
    ActionEditor, MainView, MainViewMode, RightSidebarView, RoutineEditor,
    routines_view::StartRoutineEditor,
};

actions!(
    root_view,
    [
        StartCommandPalette,
        StartTaskCreator,
        // StartActionEditor,
        ToggleSideBar
    ]
);

pub enum CurrentOverlay {
    CommandPalette(Entity<CommandPaletteState>),
    TaskCreator(Entity<TaskCreator>),
    ActionEditor(Entity<ActionEditor>),
    RoutineEditor(Entity<RoutineEditor>),
}

pub struct StartActionEditor {
    pub action_id: Option<String>,
}

pub struct RootView {
    database_store: Entity<DatabaseStore>,
    main_view: Entity<MainView>,
    right_sidebar: Entity<RightSidebarView>,
    // right_sidebar_collapsed: bool,
    focus_handle: FocusHandle,
    overlay: Option<CurrentOverlay>,
}

impl RootView {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.bind_keys([
            KeyBinding::new("cmd-p", StartCommandPalette, None),
            KeyBinding::new("cmd-t", StartTaskCreator, None),
            KeyBinding::new("alt-]", ToggleSideBar, None),
        ]);

        let drag_drop_store = cx.new(|cx| DragDropStore::new(cx));
        let main_view =
            cx.new(|cx| MainView::new(database_store.clone(), drag_drop_store.clone(), window, cx));

        let list_state = main_view.read(cx).action_list.read(cx).list_state.clone();

        let right_sidebar = cx.new(|cx| {
            RightSidebarView::new(database_store.clone(), drag_drop_store.clone(), window, cx)
        });

        let pipeline = right_sidebar.read(cx).pipeline.clone();

        cx.subscribe_in(
            &list_state,
            window,
            |this, list, event: &StartActionEditor, window, cx| {
                let action_id = event.action_id.clone();
                this.open_action_editor(action_id, window, cx);
            },
        )
        .detach();

        cx.subscribe_in(
            &pipeline,
            window,
            |this, list, event: &StartActionEditor, window, cx| {
                let action_id = event.action_id.clone();
                this.open_action_editor(action_id, window, cx);
            },
        )
        .detach();

        cx.subscribe_in(
            &main_view,
            window,
            |this, _main_view, event: &StartRoutineEditor, window, cx| {
                let routine_id = event.routine_id.clone();
                this.open_routine_editor(routine_id, window, cx);
            },
        )
        .detach();

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

    fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = self.command_palette(window, cx);
        self.overlay = Some(CurrentOverlay::CommandPalette(entity));
    }

    fn open_task_creator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let database_store = self.database_store.clone();
        let entity = cx.new(|cx| TaskCreator::new(database_store, window, cx));
        self.overlay = Some(CurrentOverlay::TaskCreator(entity));
    }

    fn open_action_editor(
        &mut self,
        action_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let database_store = self.database_store.clone();
        let editor = cx.new(|cx| ActionEditor::new(database_store, window, cx));
        if let Some(action_id) = action_id {
            editor.update(cx, |editor, cx| {
                editor.load_action(&action_id, cx);
            });
        }
        self.overlay = Some(CurrentOverlay::ActionEditor(editor));
    }

    fn open_routine_editor(
        &mut self,
        routine_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let database_store = self.database_store.clone();
        let editor = cx.new(|cx| RoutineEditor::new(database_store, routine_id, window, cx));
        self.overlay = Some(CurrentOverlay::RoutineEditor(editor));
    }

    fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = None;
        cx.focus_self(window);
    }
}

impl CommandPaletteExt for RootView {
    fn commands(&self, cx: &mut Context<Self>) -> Vec<Command> {
        let entity = cx.entity();
        vec![
            Command::new("What should I do next?")
                .icon(IconName::Star)
                .search_terms(["suggest", "recommendation", "next", "what", "do"])
                .on_select({
                    let store = self.database_store.clone();
                    move |_window, cx| {
                        cx.update_entity(&store, |store, cx| {
                            store.suggest_next(3, cx);
                        });
                    }
                }),
            Command::new("Refresh Pipeline")
                .icon(IconName::Redo)
                .search_terms([
                    "score", "reorder", "sort", "priority", "refresh", "pipeline",
                ])
                .on_select({
                    let store = self.database_store.clone();
                    move |_window, cx| {
                        cx.update_entity(&store, |store, cx| {
                            store.refresh_pipeline(cx);
                        });
                    }
                }),
            Command::new("Switch to Home View").on_select({
                let entity = self.main_view.clone();
                move |_window, cx| {
                    cx.update_entity(&entity, |main_view, cx| {
                        main_view.set_mode(MainViewMode::Home, cx);
                    });
                }
            }),
            Command::new("Switch to Routines")
                .icon(IconName::Play)
                .search_terms(["routine", "routines", "sequence", "steps"])
                .on_select({
                    let entity = self.main_view.clone();
                    move |_window, cx| {
                        cx.update_entity(&entity, |main_view, cx| {
                            main_view.set_mode(MainViewMode::Routines, cx);
                        });
                    }
                }),
            Command::new("New Routine")
                .icon(IconName::Plus)
                .search_terms(["create", "routine", "new", "add"])
                .on_select({
                    let entity = cx.entity();
                    move |window, cx| {
                        cx.update_entity(&entity, |root_view, cx| {
                            root_view.open_routine_editor(None, window, cx);
                            cx.notify();
                        });
                    }
                }),
            Command::new("Switch to Test View").on_select({
                let entity = self.main_view.clone();
                move |_window, cx| {
                    cx.update_entity(&entity, |main_view, cx| {
                        main_view.set_mode(MainViewMode::Test, cx);
                    });
                }
            }),
            Command::new("Cause an error (test)").on_select({
                |window, cx| {
                    window.push_notification((NotificationType::Error, "This is an error"), cx);
                }
            }),
            Command::new("Cause a warning (test)").on_select({
                |window, cx| {
                    window.push_notification((NotificationType::Warning, "This is a warning"), cx);
                }
            }),
            Command::new("Save notify").on_select({
                let entity = cx.entity();
                |window, cx| {
                    window.push_notification(
                        Notification::new()
                            .id::<SaveConfirmation>()
                            .title("Unsaved Changes")
                            .message("You have unsaved changes. Save before leaving?")
                            .autohide(false)
                            .action(|_, window, cx| {
                                Button::new("save")
                                    .primary()
                                    .label("Save")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.dismiss(window, cx);
                                    }))
                            }),
                        cx,
                    );
                }
            }),
            Command::new("Switch to Action List").on_select({
                let entity = self.main_view.clone();
                move |_window, cx| {
                    cx.update_entity(&entity, |main_view, cx| {
                        main_view.set_mode(MainViewMode::ActionList, cx);
                    });
                }
            }),
            Command::new("Copy")
                .shortcut("cmd-c")
                .icon(IconName::Copy)
                .search_terms(["duplicate", "clip", "clipboard", "cut"])
                .on_select(|_window, _cx| {}),
            Command::new("Paste")
                .shortcut("cmd-v")
                .icon(IconName::Copy)
                .search_terms(["insert", "clip", "clipboard", "cut"])
                .on_select(|_window, _cx| {}),
            Command::new("Find")
                .shortcut("cmd-f")
                .icon(IconName::Search)
                .search_terms(["search", "lookup", "find"])
                .on_select(|_window, _cx| {}),
            Command::new("Toggle Right Sidebar")
                .shortcut("alt-]")
                .icon(IconName::PanelRight)
                .search_terms(["sidebar", "panel", "toggle"])
                .on_select(|window, cx| {
                    window.dispatch_action(Box::new(ToggleSideBar), cx);
                }),
            Command::new("Quit Application")
                .shortcut("cmd-q")
                .icon(IconName::CircleX)
                .search_terms(["exit", "close", "quit", "leave"])
                .on_select(|_window, cx| {
                    cx.quit();
                }),
            {
                if cx.theme().mode == ThemeMode::Dark {
                    Command::new("Switch to Light Mode")
                        .icon(IconName::Sun)
                        .search_terms(["theme", "light", "dark", "appearance"])
                        .on_select(|window, cx| {
                            let action = SwitchThemeMode(ThemeMode::Light);
                            window.dispatch_action(Box::new(action), cx);
                        })
                } else {
                    Command::new("Switch to Dark Mode")
                        .icon(IconName::Moon)
                        .search_terms(["theme", "light", "dark", "appearance"])
                        .on_select(|window, cx| {
                            let action = SwitchThemeMode(ThemeMode::Dark);
                            window.dispatch_action(Box::new(action), cx);
                        })
                }
            },
        ]
    }
}

struct SaveConfirmation;

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
                .on_action(cx.listener(|this, _: &StartCommandPalette, window, cx| {
                    this.open_command_palette(window, cx);
                }))
                .on_action(cx.listener(|this, _: &StartTaskCreator, window, cx| {
                    this.open_task_creator(window, cx);
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
                    if let Some(CurrentOverlay::CommandPalette(entity)) = &this.overlay {
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
                    CurrentOverlay::CommandPalette(state) => {
                        content.child(CommandPalette::new(state.clone()))
                    }
                    CurrentOverlay::TaskCreator(creator) => content.child(creator.clone()),
                    CurrentOverlay::ActionEditor(editor) => content.child(editor.clone()),
                    CurrentOverlay::RoutineEditor(editor) => content.child(editor.clone()),
                }),
        );

        content
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
