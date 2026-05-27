use std::time::Duration;

use gpui::{
    AppContext, Context, DragMoveEvent, Entity, EventEmitter, FocusHandle, FocusId,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, Render, StatefulInteractiveElement,
    Styled, Window, actions, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Root, Sizable, StyledExt,
    animation::ease_out_cubic,
    button, h_flex,
    tab::{Tab, TabBar},
    v_flex,
};
use gpui_transitions::WindowUseTransition;
use simple_core::AnyItem;

use crate::{
    AppIcon,
    components::{
        Button, ButtonCustomVariant, ButtonVariant, ButtonVariants, CloseOverlay, DragData,
    },
    utils::{ButtonColorizeExt, ButtonColors},
    views::{ActionCreator, BacklogView, EventCreator, PipelineView, QuickAdd},
};

actions!(
    root_view,
    [
        // StartCommandPalette,
        StartActionCreator,
        StartEventCreator,
        // StartNewRoutine,
        // ToggleLeftSidebar,
        // ToggleRightSidebar,
        // ExpeditePipelineActions
    ]
);

pub enum CurrentOverlay {
    // CommandPalette(Entity<CommandPalette>),
    ActionCreator(Entity<ActionCreator>),
    // ActionEditor(Entity<crate::views::ActionEditor>),
    EventCreator(Entity<EventCreator>),
    // EventEditor(Entity<crate::views::EventEditor>),
    // RoutineEditor(Entity<RoutineEditor>),
}

pub struct RootView {
    quick_add: Entity<QuickAdd>,
    pipeline_view: Entity<PipelineView>,
    backlog_view: Entity<BacklogView>,
    backlog_hover: Option<bool>,
    backlog_open: bool,
    current_overlay: Option<(CurrentOverlay, Option<FocusHandle>)>,
}

impl RootView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let quick_add = cx.new(|cx| QuickAdd::new(cx));
        let pipeline_view = cx.new(|cx| PipelineView::new(window, cx));
        let backlog_view = cx.new(|cx| BacklogView::new(cx));
        let backlog_hover = None;
        let backlog_open = false;
        let current_overlay = None;

        cx.bind_keys([
            KeyBinding::new("cmd-n", StartActionCreator, None),
            KeyBinding::new("cmd-e", StartEventCreator, None),
        ]);

        Self {
            quick_add,
            pipeline_view,
            backlog_view,
            backlog_hover,
            backlog_open,
            current_overlay,
        }
    }
}

impl EventEmitter<()> for RootView {}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let backlog_transition = window
            .use_keyed_transition("backlog-width", cx, Duration::from_millis(150), |_, _| {
                px(32.)
            })
            .with_easing(ease_out_cubic);

        let entity = cx.entity();

        let cloned1 = backlog_transition.clone();
        let cloned2 = backlog_transition.clone();

        let backlog_width = *backlog_transition.evaluate(window, cx);
        // let backlog_open = backlog_width > px(32.);
        let mut button_colors = ButtonColors::outline(cx.theme().secondary, cx);
        button_colors.border = None;

        if let Some(hovered) = self.backlog_hover.take() {
            // if backlog_hover {
            //     button_colors.bg = cx.theme().hover;
            // }
            backlog_transition.update(cx, |value, cx| {
                *value = if hovered { px(256.) } else { px(48.) }
            });
            window.request_animation_frame();
        }

        let variant = ButtonCustomVariant::new(cx)
            .color(button_colors.bg)
            .active(button_colors.active)
            .hover(button_colors.hover);

        v_flex()
            .absolute()
            .inset_0()
            .pt_9()
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
            .on_action(cx.listener(|view, _: &CloseOverlay, window, cx| {
                if let Some(current_overlay) = view.current_overlay.take() {
                    if let Some(focus_handle) = current_overlay.1.as_ref() {
                        window.focus(focus_handle, cx);
                    }
                    view.current_overlay = None;
                    cx.notify();
                }
            }))
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w_3_4()
                    .h_3_4()
                    // .gap_2()
                    // .child(self.quick_add.clone())
                    .child(
                        h_flex()
                            .flex_1()
                            // .size_full()
                            .gap_2()
                            .child(self.pipeline_view.clone())
                            .child(
                                h_flex()
                                    .id("backlog")
                                    .h_full()
                                    .w(backlog_width)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_xl()
                                    .justify_between()
                                    .on_hover(cx.listener(move |view, hovered, window, cx| {
                                        // backlog_transition.update(cx, |value, cx| {
                                        //     *value = if *hovered { px(256.) } else { px(32.) }
                                        // });
                                        view.backlog_hover = Some(*hovered);
                                        cx.notify();
                                    }))
                                    .on_drag_move(cx.listener(
                                        move |view,
                                              event: &DragMoveEvent<DragData<AnyItem>>,
                                              _window,
                                              cx| {
                                            let is_over =
                                                event.bounds.contains(&event.event.position);
                                            view.backlog_hover = Some(is_over);
                                            cx.notify();
                                        },
                                    ))
                                    .child(
                                        div()
                                            .h_full()
                                            .flex_1()
                                            .overflow_hidden()
                                            .child(self.backlog_view.clone()),
                                    )
                                    // .when(backlog_open, |this| {
                                    //     this.child(
                                    //         div()
                                    //             .h_full()
                                    //             .flex_1()
                                    //             .overflow_hidden()
                                    //             .child(self.backlog_view.clone()),
                                    //     )
                                    // })
                                    // .child(
                                    //     div()
                                    //         .when_else(
                                    //             backlog_open,
                                    //             |this| this.rounded_xl(),
                                    //             |this| this.rounded_r_xl(),
                                    //         )
                                    //         .h_full()
                                    //         .w_8()
                                    //         .button_colors(button_colors)
                                    //         .border_0()
                                    //         .border_l_1()
                                    //         .border_color(cx.theme().border)
                                    //         .on_click(cx.listener(move |view, _, window, cx| {
                                    //             view.backlog_open = false;
                                    //             cloned1.update(cx, |value, cx| *value = px(32.0));
                                    //         })), // .icon(IconName::ChevronRight),
                                    // )
                                    // .when_else(
                                    //     backlog_open,
                                    //     |this| {
                                    //         this.justify_between()
                                    //             .child(
                                    //                 div()
                                    //                     .h_full()
                                    //                     .flex_1()
                                    //                     .overflow_hidden()
                                    //                     .child(self.backlog_view.clone()),
                                    //             )
                                    //             .child(
                                    //                 div()
                                    //                     .rounded_r_xl()
                                    //                     .id("collapse-backlog")
                                    //                     .h_full()
                                    //                     .w_8()
                                    //                     .button_colors(button_colors)
                                    //                     .border_0()
                                    //                     .border_l_1()
                                    //                     .border_color(cx.theme().border)
                                    //                     .on_click(cx.listener(
                                    //                         move |view, _, window, cx| {
                                    //                             view.backlog_open = false;
                                    //                             cloned1.update(cx, |value, cx| {
                                    //                                 *value = px(32.0)
                                    //                             });
                                    //                         },
                                    //                     )), // .icon(IconName::ChevronRight),
                                    //             )
                                    //     },
                                    //     |this| {
                                    //         this.justify_center().child(
                                    //             div()
                                    //                 .rounded_xl()
                                    //                 .id("expand-backlog")
                                    //                 .size_full()
                                    //                 .button_colors(button_colors)
                                    //                 .on_click(cx.listener(
                                    //                     move |view, _, window, cx| {
                                    //                         view.backlog_open = true;
                                    //                         cloned2.update(cx, |value, cx| {
                                    //                             *value = px(256.0)
                                    //                         });
                                    //                     },
                                    //                 )), // .icon(IconName::ChevronLeft),
                                    //         )
                                    //     },
                                    // )
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .rounded_xl(),
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
                },
            )
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
