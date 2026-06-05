use std::cmp::max;

use gpui::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, InteractiveElement, IntoElement,
    KeyBinding, ParentElement, Render, Styled, Window, actions, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Colorize, Root, Sizable, StyledExt, WindowExt, h_flex,
    tab::{Tab, TabBar},
    v_flex,
};

use crate::{
    components::{
        CloseOverlay,
        panel_group::{
            CenterPanel, NavigationBar, PanelGroup, PanelGroupState, SidePanel, SidePanelState,
        },
    },
    views::{ActionCreator, BacklogView, EventCreator, PipelineView, RoutineCreator},
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
#[repr(u8)]
enum RightSidebarTab {
    Backlog = 0,
}

pub struct RootView {
    pipeline_view: Entity<PipelineView>,
    backlog_view: Entity<BacklogView>,
    layout_state: Entity<PanelGroupState>,
    right_sidebar_tab: RightSidebarTab,
    current_overlay: Option<(CurrentOverlay, Option<FocusHandle>)>,
}

impl RootView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let pipeline_view = cx.new(|cx| PipelineView::new(window, cx));
        let backlog_view = cx.new(|cx| BacklogView::new(cx));

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
            pipeline_view,
            backlog_view,
            layout_state,
            right_sidebar_tab: RightSidebarTab::Backlog,
            current_overlay: None,
        }
    }
}

impl EventEmitter<()> for RootView {}

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

        let navbar_left_pad = (px(24. * 4.) - left_panel_width).max(px(2. * 4.));

        let tabbar_height = px(48.);

        v_flex()
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
            .child(
                PanelGroup::new(self.layout_state.clone())
                    .absolute()
                    .inset_0()
                    // .top(navbar_height)
                    .left(
                        SidePanel::left()
                            // .width_range_open(px(140.)..px(220.))
                            // .initial_proportion(0.125)
                            .p_2()
                            .pr_0()
                            .child(
                                div()
                                    .size_full()
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().background.mix_oklab(gpui::black(), 0.95)),
                            ),
                    )
                    .center(
                        CenterPanel::new().child(
                            div()
                                .pt(NAVBAR_HEIGHT)
                                .size_full()
                                .overflow_hidden()
                                .child(self.pipeline_view.clone()),
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
                                                    .pill()
                                                    // .size_full()
                                                    .rounded_none()
                                                    // .underline()
                                                    // .w_full()
                                                    // .items_center()
                                                    .selected_index(selected_tab as usize)
                                                    .child(Tab::new().flex_1().label("Backlog"))
                                                    .child(Tab::new().flex_1().label("Routines"))
                                                    .child(Tab::new().flex_1().label("Saved")),
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
                                            .child(div().flex_1().min_h_0().w_full().when(
                                                selected_tab == RightSidebarTab::Backlog,
                                                |this| this.child(self.backlog_view.clone()),
                                            )),
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
                    .gap_2()
                    .child(
                        h_flex()
                            .size_full()
                            .gap_2()
                            .p_2()
                            .child(
                                div()
                                    .size_full()
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().background.mix_oklab(gpui::black(), 0.95)),
                            )
                            .opacity(0.5),
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
