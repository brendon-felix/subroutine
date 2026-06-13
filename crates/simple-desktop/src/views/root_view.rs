use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, Pixels, Render, StatefulInteractiveElement, Styled,
    Window, actions, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Colorize, Icon, IconName, Root, Sizable, TITLE_BAR_HEIGHT, TitleBar,
    button::{Button, ButtonRounded, ButtonVariants, DropdownButton},
    h_flex,
    menu::AppMenuBar,
    tab::{Tab, TabBar},
    tooltip::Tooltip,
    v_flex,
};

use crate::{
    AppIcon,
    components::{
        CloseOverlay,
        panel_group::{
            CenterPanel, NavigationBar, PanelGroup, PanelGroupState, SidePanel, SidePanelState,
        },
    },
    views::{
        ActionCreator, BacklogView, EventCreator, PipelineView, RoutineCreator, RoutinesView,
        SavedItemsView, SelectedPipelineView,
    },
};

actions!(
    root_view,
    [
        // StartCommandPalette,
        StartActionCreator,
        StartEventCreator,
        StartRoutineCreator,
        ToggleLeftSidebar,
        ToggleRightSidebar,
    ]
);

pub const NAVBAR_HEIGHT: gpui::Pixels = px(48.);

pub enum CurrentOverlay {
    ActionCreator(Entity<ActionCreator>),
    EventCreator(Entity<EventCreator>),
    RoutineCreator(Entity<RoutineCreator>),
}

#[derive(Clone, Copy, PartialEq)]
enum RightSidebarTab {
    Backlog = 0,
    Routines = 1,
    SavedItems = 2,
}

#[derive(Clone, Copy, PartialEq)]
enum CurrentView {
    // Home = 0,
    Pipeline = 1,
}

pub struct RootView {
    app_menu_bar: Entity<AppMenuBar>,
    focus_handle: FocusHandle,
    // home_view: Entity<HomeView>,
    pipeline_view: Entity<PipelineView>,
    backlog_view: Entity<BacklogView>,
    routines_view: Entity<RoutinesView>,
    saved_items_view: Entity<SavedItemsView>,
    layout_state: Entity<PanelGroupState>,
    right_sidebar_tab: RightSidebarTab,
    current_view: CurrentView,
    current_overlay: Option<(CurrentOverlay, Option<FocusHandle>)>,
}

impl RootView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let app_menu_bar = AppMenuBar::new(cx);
        app_menu_bar.update(cx, |menu_bar, cx| {
            menu_bar.reload(cx);
        });
        let focus_handle = cx.focus_handle();
        // let home_view = cx.new(|cx| HomeView::new(cx));
        let pipeline_view = cx.new(|cx| PipelineView::new(window, cx));
        let backlog_view = cx.new(|cx| BacklogView::new(cx));
        let routines_view = cx.new(|cx| RoutinesView::new(cx));
        let saved_items_view = cx.new(|cx| SavedItemsView::new(window, cx));

        // Establish initial focus so keyboard actions dispatch immediately.
        let initial_fh = pipeline_view.read(cx).focus_handle.clone();
        window.focus(&initial_fh, cx);

        cx.bind_keys([
            KeyBinding::new("cmd-n", StartActionCreator, None),
            KeyBinding::new("cmd-shift-n", StartEventCreator, None),
            KeyBinding::new("cmd-alt-n", StartRoutineCreator, None),
            KeyBinding::new("alt-[", ToggleLeftSidebar, None),
            KeyBinding::new("alt-]", ToggleRightSidebar, None),
        ]);

        let layout_state = cx.new(|_| {
            let mut state = PanelGroupState::default();
            state.left_panel = Some(SidePanelState {
                open: false,
                ..Default::default()
            });
            state.right_panel = Some(SidePanelState {
                open: true,
                ..Default::default()
            });
            state
        });

        Self {
            app_menu_bar,
            focus_handle,
            // home_view,
            pipeline_view,
            backlog_view,
            routines_view,
            saved_items_view,
            layout_state,
            right_sidebar_tab: RightSidebarTab::Backlog,
            current_view: CurrentView::Pipeline,
            current_overlay: None,
        }
    }
}

impl EventEmitter<()> for RootView {}

impl Focusable for RootView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout_state = self.layout_state.read(cx);
        let left_panel_open = layout_state
            .left_panel
            .as_ref()
            .map(|p| p.open)
            .unwrap_or(false);

        let right_panel_open = layout_state
            .right_panel
            .as_ref()
            .map(|p| p.open)
            .unwrap_or(false);

        let selected_tab = self.right_sidebar_tab;

        let left_panel_width = layout_state.animated_left_px;

        let selected_pipeline_view = self.pipeline_view.read(cx).selected_view();

        let is_macos = cfg!(target_os = "macos");

        let navbar_left_pad = if is_macos {
            (px(24. * 4.) - left_panel_width).max(px(2. * 4.))
        } else {
            px(2. * 4.)
        };

        // let tabbar_height = px(48.);

        v_flex()
            .track_focus(&self.focus_handle)
            .absolute()
            .inset_0()
            .on_action(cx.listener(|view, _: &StartActionCreator, window, cx| {
                let current_focus = window.focused(cx);
                let action_creator = cx.new(|cx| ActionCreator::new(window, cx));
                let overlay = CurrentOverlay::ActionCreator(action_creator);
                view.current_overlay = Some((overlay, current_focus));
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &StartEventCreator, window, cx| {
                let current_focus = window.focused(cx);
                let event_creator = cx.new(|cx| EventCreator::new(window, cx));
                let overlay = CurrentOverlay::EventCreator(event_creator);
                view.current_overlay = Some((overlay, current_focus));
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &StartRoutineCreator, window, cx| {
                let current_focus = window.focused(cx);
                let routine_creator = cx.new(|cx| RoutineCreator::new(window, cx));
                let overlay = CurrentOverlay::RoutineCreator(routine_creator);
                view.current_overlay = Some((overlay, current_focus));
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &CloseOverlay, window, cx| {
                if let Some(current_overlay) = view.current_overlay.take() {
                    if let Some(focus_handle) = current_overlay.1.as_ref() {
                        window.focus(focus_handle, cx);
                    }
                    view.current_overlay = None;
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|view, _: &ToggleLeftSidebar, _window, cx| {
                view.layout_state.update(cx, |state, cx| {
                    state.toggle_left();
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|view, _: &ToggleRightSidebar, _window, cx| {
                view.layout_state.update(cx, |state, cx| {
                    state.toggle_right();
                    cx.notify();
                });
            }))
            .when(!is_macos, |this| {
                this.child(
                    TitleBar::new()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top_0()
                        .child(h_flex().size_full().child(self.app_menu_bar.clone()))
                        .child(
                            h_flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    Button::new("settings")
                                        .ghost()
                                        .icon(Icon::new(IconName::Settings)),
                                )
                                .child(Button::new("info").ghost().icon(Icon::new(AppIcon::Info))),
                        ),
                )
            })
            .child(
                NavigationBar::new()
                    .absolute()
                    .when_else(
                        is_macos,
                        |this| this.top_0(),
                        |this| this.top(TITLE_BAR_HEIGHT),
                    )
                    .right_0()
                    .left(left_panel_width)
                    .pl(navbar_left_pad)
                    .pr_2()
                    .h(NAVBAR_HEIGHT)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().transparent.mix_oklab(cx.theme().background, 0.5))
                    .left_panel_open(left_panel_open)
                    .right_panel_open(right_panel_open)
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
                    .gap_4()
                    .child(
                        h_flex()
                            .size_full()
                            .gap_8()
                            .p_2()
                            .child(
                                Button::new("new-item")
                                    .outline()
                                    .icon(Icon::new(IconName::Plus))
                                    .rounded_full()
                                    .text_2xl(),
                            )
                            .child(
                                TabBar::new("main-tabbar")
                                    .outline()
                                    .selected_index(selected_pipeline_view as usize)
                                    .child(
                                        Tab::new()
                                            .icon(Icon::new(AppIcon::Timeline))
                                            // .label("Timeline")
                                            .tooltip(|window, cx| {
                                                Tooltip::new("Timeline view").build(window, cx)
                                            })
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.pipeline_view.update(cx, |pipeline, cx| {
                                                    pipeline.select_view(
                                                        SelectedPipelineView::Timeline,
                                                        window,
                                                        cx,
                                                    );
                                                })
                                            })),
                                    )
                                    .child(
                                        Tab::new()
                                            .icon(Icon::new(AppIcon::ListChecks))
                                            // .label("Queue")
                                            .tooltip(|window, cx| {
                                                Tooltip::new("Queue view").build(window, cx)
                                            })
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.pipeline_view.update(cx, |pipeline, cx| {
                                                    pipeline.select_view(
                                                        SelectedPipelineView::Queue,
                                                        window,
                                                        cx,
                                                    );
                                                })
                                            })),
                                    )
                                    .child(
                                        Tab::new()
                                            .icon(Icon::new(AppIcon::ScanEye))
                                            // .label("Focus")
                                            .tooltip(|window, cx| {
                                                Tooltip::new("Focus mode").build(window, cx)
                                            })
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.pipeline_view.update(cx, |pipeline, cx| {
                                                    pipeline.select_view(
                                                        SelectedPipelineView::Focus,
                                                        window,
                                                        cx,
                                                    );
                                                })
                                            })),
                                    ),
                            ),
                    ),
            )
            .child(
                PanelGroup::new(self.layout_state.clone())
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .when_else(
                        is_macos,
                        |this| this.top(NAVBAR_HEIGHT),
                        |this| this.top(TITLE_BAR_HEIGHT + NAVBAR_HEIGHT),
                    )
                    .left(
                        SidePanel::left()
                            // .width_range_open(px(140.)..px(220.))
                            // .initial_proportion(0.125)
                            .p_2()
                            .pr_0()
                            .child(div().size_full().when_else(
                                is_macos,
                                |this| {
                                    this.rounded_xl()
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .bg(cx.theme().background.mix_oklab(gpui::black(), 0.95))
                                },
                                |this| this.border_r_1().border_color(cx.theme().border),
                            )),
                    )
                    .center(
                        CenterPanel::new().child(
                            div()
                                .pt(NAVBAR_HEIGHT)
                                .size_full()
                                .overflow_hidden()
                                .map(|this| match self.current_view {
                                    // CurrentView::Home => this.child(self.home_view.clone()),
                                    CurrentView::Pipeline => this.child(self.pipeline_view.clone()),
                                }),
                        ),
                    )
                    .right(
                        SidePanel::right()
                            .width_range_open(px(200.)..px(250.))
                            // .initial_proportion(0.25)
                            .child(
                                v_flex()
                                    .size_full()
                                    .pt(NAVBAR_HEIGHT)
                                    .child(
                                        div()
                                            .flex()
                                            .border_l_1()
                                            .border_color(cx.theme().border)
                                            // .h(tabbar_height)
                                            .w_full()
                                            .p_1()
                                            .justify_center()
                                            // .border_b_1()
                                            // .border_color(cx.theme().border)
                                            .child(
                                                TabBar::new("right-sidebar-tabs")
                                                    .outline()
                                                    // .pill()
                                                    // .size_full()
                                                    .rounded_none()
                                                    // .underline()
                                                    // .w_full()
                                                    // .items_center()
                                                    .selected_index(selected_tab as usize)
                                                    .child(
                                                        Tab::new()
                                                            .flex_1()
                                                            .icon(Icon::new(AppIcon::Archive))
                                                            // .label("Backlog")
                                                            .tooltip(|window, cx| {
                                                                Tooltip::new("Backlog")
                                                                    .build(window, cx)
                                                            })
                                                            .on_click(cx.listener(
                                                                |view, _, _, cx| {
                                                                    view.right_sidebar_tab =
                                                                        RightSidebarTab::Backlog;
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        Tab::new()
                                                            .flex_1()
                                                            .icon(Icon::new(AppIcon::Repeat))
                                                            // .label("Routines")
                                                            .tooltip(|window, cx| {
                                                                Tooltip::new("Routines")
                                                                    .build(window, cx)
                                                            })
                                                            .on_click(cx.listener(
                                                                |view, _, _, cx| {
                                                                    view.right_sidebar_tab =
                                                                        RightSidebarTab::Routines;
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        Tab::new()
                                                            .flex_1()
                                                            .icon(Icon::new(AppIcon::Save))
                                                            // .label("Saved")
                                                            .tooltip(|window, cx| {
                                                                Tooltip::new("Saved items")
                                                                    .build(window, cx)
                                                            })
                                                            .on_click(cx.listener(
                                                                |view, _, _, cx| {
                                                                    view.right_sidebar_tab =
                                                                        RightSidebarTab::SavedItems;
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .border_l_1()
                                            .border_color(cx.theme().border)
                                            // .absolute()
                                            // .top(tabbar_height)
                                            // .bottom_0()
                                            // .left_0()
                                            // .right_0()
                                            .flex_1()
                                            .overflow_hidden()
                                            .child(div().flex_1().min_h_0().w_full().map(|this| {
                                                match selected_tab {
                                                    RightSidebarTab::Backlog => {
                                                        this.child(self.backlog_view.clone())
                                                    }
                                                    RightSidebarTab::Routines => {
                                                        this.child(self.routines_view.clone())
                                                    }
                                                    RightSidebarTab::SavedItems => {
                                                        this.child(self.saved_items_view.clone())
                                                    }
                                                }
                                            })),
                                    ),
                            ),
                    ),
            )
            .child(
                NavigationBar::new()
                    .absolute()
                    .top_0()
                    .right_0()
                    .left(left_panel_width)
                    .pl(navbar_left_pad)
                    .pr_2()
                    .h(NAVBAR_HEIGHT)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().transparent.mix_oklab(cx.theme().background, 0.5))
                    .left_panel_open(left_panel_open)
                    .right_panel_open(right_panel_open)
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
                    .gap_4()
                    .child(
                        h_flex()
                            .size_full()
                            .gap_8()
                            .p_2()
                            // .child(
                            // )
                            .child(
                                DropdownButton::new("new-item")
                                    .small()
                                    .outline()
                                    .rounded(ButtonRounded::Size(Pixels::MAX))
                                    // .button(
                                    //     Button::new("new-item")
                                    //         .small()
                                    //         .outline()
                                    //         .icon(Icon::new(IconName::Plus))
                                    //         .rounded_l_full()
                                    //         .text_2xl(),
                                    // )
                                    .dropdown_menu(|menu, _, _| {
                                        menu.menu("New action", Box::new(StartActionCreator))
                                            .menu("New event", Box::new(StartEventCreator))
                                            .menu("New routine", Box::new(StartRoutineCreator))
                                    }),
                            )
                            .child(
                                TabBar::new("main-tabbar")
                                    .outline()
                                    .selected_index(selected_pipeline_view as usize)
                                    .child(
                                        Tab::new()
                                            .icon(Icon::new(AppIcon::Timeline))
                                            // .label("Timeline")
                                            .tooltip(|window, cx| {
                                                Tooltip::new("Timeline view").build(window, cx)
                                            })
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.pipeline_view.update(cx, |pipeline, cx| {
                                                    pipeline.select_view(
                                                        SelectedPipelineView::Timeline,
                                                        window,
                                                        cx,
                                                    );
                                                })
                                            })),
                                    )
                                    .child(
                                        Tab::new()
                                            .icon(Icon::new(AppIcon::ListChecks))
                                            // .label("Queue")
                                            .tooltip(|window, cx| {
                                                Tooltip::new("Queue view").build(window, cx)
                                            })
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.pipeline_view.update(cx, |pipeline, cx| {
                                                    pipeline.select_view(
                                                        SelectedPipelineView::Queue,
                                                        window,
                                                        cx,
                                                    );
                                                })
                                            })),
                                    )
                                    .child(
                                        Tab::new()
                                            .icon(Icon::new(AppIcon::ScanEye))
                                            // .label("Focus")
                                            .tooltip(|window, cx| {
                                                Tooltip::new("Focus mode").build(window, cx)
                                            })
                                            .on_click(cx.listener(|view, _, window, cx| {
                                                view.pipeline_view.update(cx, |pipeline, cx| {
                                                    pipeline.select_view(
                                                        SelectedPipelineView::Focus,
                                                        window,
                                                        cx,
                                                    );
                                                })
                                            })),
                                    ),
                            ),
                    ),
            )
            .when_some(
                self.current_overlay.as_ref(),
                |this, overlay| match &overlay.0 {
                    CurrentOverlay::ActionCreator(action_creator) => {
                        this.child(action_creator.clone())
                    }
                    CurrentOverlay::EventCreator(event_creator) => {
                        this.child(event_creator.clone())
                    }
                    CurrentOverlay::RoutineCreator(routine_creator) => {
                        this.child(routine_creator.clone())
                    }
                },
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
