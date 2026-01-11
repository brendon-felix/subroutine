use std::time::Duration;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Window, actions, div, px,
};
use gpui::{KeyBinding, prelude::*};
use gpui_component::divider::Divider;
use gpui_component::label::Label;
use gpui_component::{
    ActiveTheme,
    Root,
    WindowExt,
    // resizable::{h_resizable, resizable_panel},
    switch::Switch,
    v_flex,
};
use gpui_transitions::WindowUseTransition;
// use gpui_transitions::WindowUseTransition;

// use crate::app::ToggleCommandPalette;
use crate::components::command_palette::{
    CloseCommandPalette, Command, CommandPalette, CommandPaletteExt, CommandPaletteState,
    SelectCommand,
};
use crate::components::resizable::{h_resizable, resizable_panel};
// use crate::stores::ui_store::{TaskSelected, ViewChanged};
use crate::stores::TaskStore;
use crate::transitions::ease_out;
// use crate::transitions::ease_out;
use crate::views::MainView;
use crate::views::main_view::MainViewMode;

actions!(root_view, [ToggleCommandPalette, ToggleSideBar]);

pub struct RootView {
    main_view: Entity<MainView>,
    // right_sidebar: Entity<RightSidebarView>,
    right_sidebar_collapsed: bool,
    focus_handle: FocusHandle,
    cmd_palette: Option<Entity<CommandPaletteState>>,
}

impl RootView {
    pub fn new(task_store: Entity<TaskStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([
            KeyBinding::new("cmd-p", ToggleCommandPalette, None),
            KeyBinding::new("alt-]", ToggleSideBar, None),
        ]);

        let main_view = cx.new(|cx| MainView::new(task_store.clone(), window, cx));
        // let right_sidebar = cx.new(|cx| RightSidebarView::new(window, cx));
        let focus_handle = cx.focus_handle();

        Self {
            main_view,
            // right_sidebar,
            right_sidebar_collapsed: false,
            focus_handle,
            cmd_palette: None,
        }
    }

    fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        println!("Toggling command palette");
        if self.cmd_palette.is_some() {
            self.cmd_palette = None;
            window.focus(&self.focus_handle);
        } else {
            let cmd_palette = self.command_palette(window, cx);
            self.cmd_palette = Some(cmd_palette);
        }
    }

    pub fn open_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.open_sheet(cx, |sheet, _, _| {
            sheet.title("Navigation").child("Sheet content goes here")
        })
    }
}

impl CommandPaletteExt for RootView {
    fn commands(&self, cx: &mut Context<Self>) -> Vec<Command> {
        // let entity = cx.entity().clone();
        vec![
            Command::new("test-view", "Switch to Test View").on_select({
                let entity = self.main_view.clone();
                move |_window, cx| {
                    cx.update_entity(&entity, |main_view, cx| {
                        main_view.set_mode(MainViewMode::Test, cx);
                    });
                }
            }),
            Command::new("test-view", "Switch to Task List View").on_select({
                let entity = self.main_view.clone();
                move |_window, cx| {
                    cx.update_entity(&entity, |main_view, cx| {
                        main_view.set_mode(MainViewMode::TaskList, cx);
                    });
                }
            }),
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
                .on_select(|window, cx| {
                    window.dispatch_action(Box::new(ToggleSideBar), cx);
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
        // let right_sidebar_collapsed = self.right_sidebar.read(cx).is_collapsed();
        // let right_sidebar_collapsed = self.right_sidebar.read(cx).is_collapsed();

        let slide_transition = Some(
            window
                .use_keyed_transition(
                    "right-sidebar-slide",
                    cx,
                    Duration::from_millis(200),
                    move |_window, _cx| px(200.),
                )
                .continuous(true)
                .with_easing(ease_out),
        );

        let content = div().size_full().flex().child(
            div()
                .size_full()
                .flex()
                .track_focus(&self.focus_handle)
                .on_action(
                    cx.listener(|this: &mut RootView, _: &ToggleSideBar, window, cx| {
                        // // this.right_sidebar.update(cx, |sidebar, cx| {
                        // //     sidebar.toggle_collapsed(cx);
                        // // });
                        // this.open_sheet(window, cx);
                        this.right_sidebar_collapsed = !this.right_sidebar_collapsed;
                        cx.notify();
                    }),
                )
                .on_action(cx.listener(|this, _: &ToggleCommandPalette, window, cx| {
                    this.toggle_command_palette(window, cx);
                }))
                .on_action(cx.listener(|this, _: &CloseCommandPalette, window, cx| {
                    this.cmd_palette = None;
                    window.focus(&this.focus_handle);
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &SelectCommand, window, cx| {
                    println!("Command selected, closing palette.");
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
                }))
                .child(
                    h_resizable("root-layout")
                        .on_resize(cx.listener(|_this, _state, _window, cx| {
                            cx.notify();
                        }))
                        .child(
                            // Center content panel
                            resizable_panel().child(
                                div()
                                    .size_full()
                                    .p_2()
                                    .when(!self.right_sidebar_collapsed, |div| div.pr_1())
                                    .bg(cx.theme().group_box)
                                    // .bg(gpui::red())
                                    .child(
                                        div()
                                            .size_full()
                                            .bg(cx.theme().background)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .rounded_lg()
                                            .child(self.main_view.clone()),
                                    ),
                            ),
                        )
                        // .child(
                        //     // Right sidebar panel
                        //     resizable_panel()
                        //         .size(px(350.0))
                        //         .size_range(px(250.0)..px(500.0))
                        //         // .visible(!right_sidebar_collapsed && has_selected_task)
                        //         .visible(!right_sidebar_collapsed)
                        //         .child(self.right_sidebar.clone()),
                        // ),
                        .child(
                            // Right sidebar panel
                            resizable_panel()
                                .size(px(200.0))
                                .size_range(px(200.0)..px(500.0))
                                .visible(!self.right_sidebar_collapsed)
                                .child(
                                    div()
                                        .size_full()
                                        .p_2()
                                        .pl_1()
                                        .bg(cx.theme().group_box)
                                        .child(
                                            div()
                                                .size_full()
                                                .bg(cx.theme().background)
                                                .border_1()
                                                .border_color(cx.theme().border)
                                                .rounded_lg()
                                                .child(
                                                    // right sidebar content
                                                    v_flex()
                                                        .overflow_hidden()
                                                        .size_full()
                                                        .gap_3()
                                                        .p_4()
                                                        .child(Label::new("Settings").text_lg())
                                                        .child(Divider::horizontal())
                                                        .child(
                                                            Switch::new("dark-mode-switch")
                                                                .checked(cx.theme().is_dark())
                                                                .label("Dark mode")
                                                                .on_click(cx.listener(
                                                                    |_view, _checked, _, cx| {
                                                                        // view.is_enabled = *checked;
                                                                        cx.notify();
                                                                    },
                                                                )),
                                                        )
                                                        .child(
                                                            Switch::new("alerts-switch")
                                                                .checked(true)
                                                                .label("Enable alerts")
                                                                .on_click(cx.listener(
                                                                    |_view, _checked, _, cx| {
                                                                        // view.is_enabled = *checked;
                                                                        cx.notify();
                                                                    },
                                                                )),
                                                        ), // .child(Divider::horizontal()),
                                                ),
                                        ),
                                ),
                        ),
                )
                .when_some(self.cmd_palette.as_ref(), |content, cmd_palette| {
                    content.child(CommandPalette::new(cmd_palette.clone()))
                }),
        );

        content
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
