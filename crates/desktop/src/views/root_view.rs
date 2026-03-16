use uuid::Uuid;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window,
    actions, div, px,
};
use gpui::{KeyBinding, prelude::*};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::notification::{Notification, NotificationType};
use gpui_component::{
    ActiveTheme, IconName, PixelsExt as _, Root, Sizable, ThemeMode, WindowExt, h_flex,
};

use crate::app::ResultExt;
use crate::components::action_creator::ActionCreator;
use crate::components::command_palette::{
    CloseCommandPalette, Command, CommandPalette, CommandPaletteExt, CommandPaletteState,
    SelectCommand,
};
use crate::components::event_creator::EventCreator;
use crate::components::panel_group::{
    CenterPanel, NavigationBar, PanelGroup, PanelGroupState, SidePanel, SidePanelState,
};
use crate::components::popover::CloseOverlay;
use crate::stores::DatabaseStore;
use crate::themes::SwitchThemeMode;
use crate::views::{
    MainView, MainViewMode, PipelineSidebarView, RoutineEditor, action_editor::StartActionEditor,
    event_editor::StartEventEditor, pipeline::StartQueueEventEditor,
    routines_view::StartRoutineEditor,
};

actions!(
    root_view,
    [
        StartCommandPalette,
        StartActionCreator,
        StartEventCreator,
        StartNewRoutine,
        ToggleSideBar,
        ToggleRightPanel,
        ExpeditePipelineActions
    ]
);

pub enum CurrentOverlay {
    CommandPalette(Entity<CommandPaletteState>),
    ActionCreator(Entity<ActionCreator>),
    ActionEditor(Entity<crate::views::ActionEditor>),
    EventCreator(Entity<EventCreator>),
    EventEditor(Entity<crate::views::EventEditor>),
    RoutineEditor(Entity<RoutineEditor>),
}

pub struct RootView {
    database_store: Entity<DatabaseStore>,
    main_view: Entity<MainView>,
    left_sidebar: Entity<PipelineSidebarView>,
    layout_state: Entity<PanelGroupState>,
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
            KeyBinding::new("cmd-n", StartActionCreator, None),
            KeyBinding::new("cmd-e", StartEventCreator, None),
            KeyBinding::new("cmd-shift-r", StartNewRoutine, None),
            KeyBinding::new("alt-[", ToggleSideBar, None),
            KeyBinding::new("alt-]", ToggleRightPanel, None),
        ]);

        let main_view = cx.new(|cx| MainView::new(database_store.clone(), window, cx));
        let list_state = main_view.read(cx).action_list.read(cx).list_state.clone();

        let left_sidebar =
            cx.new(|cx| PipelineSidebarView::new(database_store.clone(), window, cx));

        cx.subscribe_in(
            &list_state,
            window,
            |this, _list, event: &StartActionEditor, window, cx| {
                this.open_action_editor(event.action_id, window, cx);
            },
        )
        .detach();

        cx.subscribe_in(
            &main_view,
            window,
            |this, _main_view, event: &StartRoutineEditor, window, cx| {
                this.open_routine_editor(event.routine_id, window, cx);
            },
        )
        .detach();

        let pipeline = left_sidebar.read(cx).pipeline.clone();

        cx.subscribe_in(
            &pipeline,
            window,
            |this, _pipeline, event: &StartActionEditor, window, cx| {
                this.open_action_editor(event.action_id, window, cx);
            },
        )
        .detach();

        cx.subscribe_in(
            &pipeline,
            window,
            |this, _pipeline, event: &StartEventEditor, window, cx| {
                this.open_event_editor(event.event_id, window, cx);
            },
        )
        .detach();

        cx.subscribe_in(
            &pipeline,
            window,
            |this, _pipeline, event: &StartQueueEventEditor, window, cx| {
                this.open_queue_event_editor(event.event_id, window, cx);
            },
        )
        .detach();

        let focus_handle = cx.focus_handle();
        cx.focus_self(window);

        let layout_state = cx.new(|_| {
            let mut state = PanelGroupState::default();
            state.left_panel = Some(SidePanelState {
                proportion_range: 0.1..0.5,
                opened_proportion: 0.25,
                open: true,
            });
            state.right_panel = Some(SidePanelState {
                proportion_range: 0.1..0.5,
                opened_proportion: 0.25,
                open: false,
            });
            state
        });

        Self {
            database_store,
            main_view,
            left_sidebar,
            layout_state,
            focus_handle,
            overlay: None,
        }
    }

    fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = self.command_palette(window, cx);
        self.overlay = Some(CurrentOverlay::CommandPalette(entity));
    }

    fn open_action_creator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let database_store = self.database_store.clone();
        let entity = cx.new(|cx| ActionCreator::new(database_store, window, cx));
        self.overlay = Some(CurrentOverlay::ActionCreator(entity));
    }

    fn open_event_creator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let database_store = self.database_store.clone();
        let entity = cx.new(|cx| EventCreator::new(database_store, window, cx));
        self.overlay = Some(CurrentOverlay::EventCreator(entity));
    }

    fn open_action_editor(
        &mut self,
        action_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let database_store = self.database_store.clone();
        let editor = cx.new(|cx| crate::views::ActionEditor::new(database_store, window, cx));
        if let Some(id) = action_id {
            editor.update(cx, |editor, cx| {
                editor.load_action(id, cx);
            });
        }
        self.overlay = Some(CurrentOverlay::ActionEditor(editor));
    }

    fn open_queue_event_editor(
        &mut self,
        event_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let database_store = self.database_store.clone();
        let editor = cx.new(|cx| crate::views::EventEditor::new(database_store, window, cx));
        editor.update(cx, |editor, cx| {
            editor.load_queue_event(event_id, cx);
        });
        self.overlay = Some(CurrentOverlay::EventEditor(editor));
    }

    fn open_event_editor(
        &mut self,
        event_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let database_store = self.database_store.clone();
        let editor = cx.new(|cx| crate::views::EventEditor::new(database_store, window, cx));
        if let Some(id) = event_id {
            editor.update(cx, |editor, cx| {
                editor.load_event(id, cx);
            });
        }
        self.overlay = Some(CurrentOverlay::EventEditor(editor));
    }

    fn open_routine_editor(
        &mut self,
        routine_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let database_store = self.database_store.clone();
        let editor = cx.new(|cx| RoutineEditor::new(database_store, routine_id, window, cx));
        self.overlay = Some(CurrentOverlay::RoutineEditor(editor));
    }
}

impl CommandPaletteExt for RootView {
    fn commands(&self, cx: &mut Context<Self>) -> Vec<Command> {
        vec![
            Command::new("Go to Home View").on_select({
                let entity = self.main_view.clone();
                move |_window, cx| {
                    cx.update_entity(&entity, |main_view, cx| {
                        main_view.set_mode(MainViewMode::Home, cx);
                    });
                }
            }),
            Command::new("Go to Focus Mode")
                .icon(IconName::Star)
                .search_terms(["focus", "focus mode", "suggestions", "calm"])
                .on_select({
                    let entity = self.main_view.clone();
                    move |_window, cx| {
                        cx.update_entity(&entity, |main_view, cx| {
                            main_view.set_mode(MainViewMode::Focus, cx);
                        });
                    }
                }),
            Command::new("Go to Routines")
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
            Command::new("Go to Action List")
                .icon(IconName::Inbox)
                .search_terms(["actions", "list", "all"])
                .on_select({
                    let entity = self.main_view.clone();
                    move |_window, cx| {
                        cx.update_entity(&entity, |main_view, cx| {
                            main_view.set_mode(MainViewMode::ActionList, cx);
                        });
                    }
                }),
            Command::new("Go to backlog")
                .icon(IconName::TriangleAlert)
                .search_terms(["backlog", "history", "past", "completed"])
                .on_select({
                    let entity = self.main_view.clone();
                    move |_window, cx| {
                        cx.update_entity(&entity, |main_view, cx| {
                            main_view.set_mode(MainViewMode::ActionList, cx);
                        });
                    }
                }),
            Command::new("Create new action")
                .icon(IconName::Plus)
                .shortcut("cmd-t")
                .search_terms(["create", "action", "new", "add", "task"])
                .on_select({
                    move |window, cx| {
                        window.dispatch_action(Box::new(StartActionCreator), cx);
                    }
                }),
            Command::new("Create new event")
                .icon(IconName::Calendar)
                .shortcut("cmd-e")
                .search_terms(["create", "event", "new", "add", "schedule", "calendar"])
                .on_select({
                    move |window, cx| {
                        window.dispatch_action(Box::new(StartEventCreator), cx);
                    }
                }),
            Command::new("Create new routine")
                .icon(IconName::Plus)
                .search_terms(["create", "routine", "new", "add"])
                .on_select({
                    move |window, cx| {
                        window.dispatch_action(Box::new(StartNewRoutine), cx);
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
            Command::new("Toggle Left Sidebar")
                .shortcut("alt-[")
                .icon(IconName::PanelLeftOpen)
                .search_terms(["sidebar", "panel", "toggle", "left"])
                .on_select(|window, cx| {
                    window.dispatch_action(Box::new(ToggleSideBar), cx);
                }),
            Command::new("Toggle Right Sidebar")
                .shortcut("alt-]")
                .icon(IconName::PanelRightOpen)
                .search_terms(["sidebar", "panel", "toggle", "right"])
                .on_select(|window, cx| {
                    window.dispatch_action(Box::new(ToggleRightPanel), cx);
                }),
            Command::new("Expedite Actions")
                .icon(IconName::Play)
                .search_terms(["expedite", "reschedule", "now", "pipeline", "compress"])
                .on_select({
                    let database_store = self.database_store.clone();
                    move |_window, cx| {
                        database_store.update(cx, |store, cx| {
                            store.expedite_actions(cx);
                        });
                    }
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
        use gpui_component::v_flex;

        let (
            left_panel_open,
            animated_left_px,
            left_open_target,
            right_panel_open,
            animated_right_px,
            right_open_target,
        ) = {
            let layout = self.layout_state.read(cx);
            let container_width = layout.container_width();
            (
                layout.left_panel.as_ref().map(|p| p.open).unwrap_or(false),
                layout.animated_left_px,
                layout
                    .left_panel
                    .as_ref()
                    .map(|p| container_width * p.opened_proportion)
                    .unwrap_or(px(0.)),
                layout.right_panel.as_ref().map(|p| p.open).unwrap_or(false),
                layout.animated_right_px,
                layout
                    .right_panel
                    .as_ref()
                    .map(|p| container_width * p.opened_proportion)
                    .unwrap_or(px(0.)),
            )
        };

        let t_left = if left_open_target > px(0.) {
            (animated_left_px.as_f32() / left_open_target.as_f32()).min(1.0)
        } else {
            0.0
        };
        let left_pad = px(8.0 - t_left * 4.0);
        let traffic_light_pad = if window.is_fullscreen() {
            px(0.)
        } else {
            px(70.0 * (1.0 - t_left))
        };

        let t_right = if right_open_target > px(0.) {
            (animated_right_px.as_f32() / right_open_target.as_f32()).min(1.0)
        } else {
            0.0
        };
        let right_pad = px(8.0 - t_right * 4.0);

        let current_mode = self.main_view.read(cx).mode;
        let main_view = self.main_view.clone();

        let nav_bar = {
            let main_view = main_view.clone();
            NavigationBar::new()
                .h_8()
                .gap_3()
                .left_panel_open(left_panel_open)
                .right_panel_open(right_panel_open)
                .traffic_light_padding(traffic_light_pad)
                .on_toggle_left({
                    let layout_state = self.layout_state.clone();
                    move |_window, cx| {
                        layout_state.update(cx, |state, cx| {
                            state.toggle_left();
                            cx.notify();
                        });
                    }
                })
                .on_toggle_right({
                    let layout_state = self.layout_state.clone();
                    move |_window, cx| {
                        layout_state.update(cx, |state, cx| {
                            state.toggle_right();
                            cx.notify();
                        });
                    }
                })
            // .child(
            //     Button::new("nav-focus")
            //         .small()
            //         .rounded_r_none()
            //         .border_1()
            //         .border_r_0()
            //         .border_color(cx.theme().border)
            //         .ghost()
            //         .icon(IconName::Star)
            //         .label("Focus")
            //         .when(current_mode == MainViewMode::Focus, |b| b.primary())
            //         .on_click({
            //             let main_view = main_view.clone();
            //             move |_, _window, cx| {
            //                 main_view.update(cx, |v, cx| v.set_mode(MainViewMode::Focus, cx));
            //             }
            //         }),
            // )
            // .child(
            //     Button::new("nav-routines")
            //         .small()
            //         .rounded_none()
            //         .border_y_1()
            //         .border_color(cx.theme().border)
            //         .ghost()
            //         .icon(IconName::Palette)
            //         .label("Routines")
            //         .when(current_mode == MainViewMode::Routines, |b| b.primary())
            //         .on_click({
            //             let main_view = main_view.clone();
            //             move |_, _window, cx| {
            //                 main_view
            //                     .update(cx, |v, cx| v.set_mode(MainViewMode::Routines, cx));
            //             }
            //         }),
            // )
            // .child(
            //     Button::new("nav-backlog")
            //         .small()
            //         .rounded_l_none()
            //         .border_1()
            //         .border_l_0()
            //         .border_color(cx.theme().border)
            //         .ghost()
            //         .icon(IconName::GalleryVerticalEnd)
            //         .label("Backlog")
            //         .when(current_mode == MainViewMode::ActionList, |b| b.primary())
            //         .on_click({
            //             let main_view = main_view.clone();
            //             move |_, _window, cx| {
            //                 main_view
            //                     .update(cx, |v, cx| v.set_mode(MainViewMode::ActionList, cx));
            //             }
            //         }),
            // )
        };

        let main_area = PanelGroup::new(self.layout_state.clone())
            .left(
                SidePanel::left()
                    .width_range_open(px(180.)..px(500.))
                    .initial_width(px(320.))
                    .child(
                        v_flex()
                            .size_full()
                            .pt_0()
                            .pl_2()
                            .pb_2()
                            .pr_1()
                            .bg(cx.theme().secondary)
                            .child(h_flex().h_8())
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .size_full()
                                    .overflow_hidden()
                                    .bg(cx.theme().background)
                                    .rounded_lg()
                                    .child(self.left_sidebar.clone()),
                            ),
                    ),
            )
            .center(
                CenterPanel::new().child(
                    div()
                        .flex()
                        .flex_col()
                        .size_full()
                        .pt_0()
                        .pr(right_pad)
                        .pb_2()
                        .pl(left_pad)
                        .bg(cx.theme().secondary)
                        .child(nav_bar)
                        .child(
                            v_flex()
                                .size_full()
                                .bg(cx.theme().background)
                                .rounded_lg()
                                .child(div().flex_1().min_h(px(0.)).w_full().child(main_view)),
                        ),
                ),
            )
            .right(
                SidePanel::right()
                    .width_range_open(px(180.)..px(500.))
                    .child(
                        v_flex()
                            .size_full()
                            .pt_0()
                            .pl_1()
                            .pb_2()
                            .pr_2()
                            .bg(cx.theme().secondary)
                            .child(h_flex().h_8())
                            .child(
                                div()
                                    .size_full()
                                    .overflow_hidden()
                                    .bg(cx.theme().background)
                                    .rounded_lg(),
                            ),
                    ),
            );

        let content = div().size_full().flex().child(
            div()
                .size_full()
                .flex()
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(|this, _: &ToggleSideBar, _window, cx| {
                    this.layout_state.update(cx, |state, cx| {
                        state.toggle_left();
                        cx.notify();
                    });
                }))
                .on_action(cx.listener(|this, _: &ToggleRightPanel, _window, cx| {
                    this.layout_state.update(cx, |state, cx| {
                        state.toggle_right();
                        cx.notify();
                    });
                }))
                .on_action(cx.listener(|this, _: &StartCommandPalette, window, cx| {
                    this.open_command_palette(window, cx);
                }))
                .on_action(cx.listener(|this, _: &StartActionCreator, window, cx| {
                    this.open_action_creator(window, cx);
                }))
                .on_action(cx.listener(|this, _: &StartEventCreator, window, cx| {
                    this.open_event_creator(window, cx);
                }))
                .on_action(cx.listener(|this, _: &StartNewRoutine, window, cx| {
                    this.open_routine_editor(None, window, cx);
                }))
                .on_action(cx.listener(|this, _: &CloseOverlay, window, cx| {
                    this.overlay = None;
                    cx.focus_self(window);
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &CloseCommandPalette, window, cx| {
                    this.overlay = None;
                    cx.focus_self(window);
                    cx.notify();
                }))
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
                .when_some(self.overlay.as_ref(), |content, overlay| match overlay {
                    CurrentOverlay::CommandPalette(state) => {
                        content.child(CommandPalette::new(state.clone()))
                    }
                    CurrentOverlay::ActionCreator(creator) => content.child(creator.clone()),
                    CurrentOverlay::ActionEditor(editor) => content.child(editor.clone()),
                    CurrentOverlay::EventCreator(creator) => content.child(creator.clone()),
                    CurrentOverlay::EventEditor(editor) => content.child(editor.clone()),
                    CurrentOverlay::RoutineEditor(editor) => content.child(editor.clone()),
                }),
        );

        content
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
