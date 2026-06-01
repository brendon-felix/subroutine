use std::time::Duration;

use gpui::{
    AppContext, Context, DragMoveEvent, Entity, EventEmitter, FocusHandle, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, Pixels, Render, StatefulInteractiveElement, Styled,
    Window, actions, div, prelude::FluentBuilder, px,
};
use gpui_component::{Root, StyledExt, animation::ease_out_cubic, h_flex, v_flex};
use gpui_transitions::WindowUseTransition;
use simple_core::AnyItem;

use crate::{
    components::{CloseOverlay, DragData},
    views::{ActionCreator, BacklogView, EventCreator, PipelineView, RoutineCreator},
};

actions!(
    root_view,
    [
        // StartCommandPalette,
        StartActionCreator,
        StartEventCreator,
        StartRoutineCreator,
        // ToggleLeftSidebar,
        // ToggleRightSidebar,
    ]
);

pub enum CurrentOverlay {
    // CommandPalette(Entity<CommandPalette>),
    ActionCreator(Entity<ActionCreator>),
    // ActionEditor(Entity<crate::views::ActionEditor>),
    EventCreator(Entity<EventCreator>),
    // EventEditor(Entity<crate::views::EventEditor>),
    RoutineCreator(Entity<RoutineCreator>),
    // RoutineEditor(Entity<RoutineEditor>),
}

pub struct RootView {
    pipeline_view: Entity<PipelineView>,
    backlog_view: Entity<BacklogView>,
    backlog_hover: Option<bool>,
    current_overlay: Option<(CurrentOverlay, Option<FocusHandle>)>,
}

impl RootView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let pipeline_view = cx.new(|cx| PipelineView::new(window, cx));
        let backlog_view = cx.new(|cx| BacklogView::new(cx));
        let backlog_hover = None;
        let current_overlay = None;

        cx.bind_keys([
            KeyBinding::new("cmd-n", StartActionCreator, None),
            KeyBinding::new("cmd-shift-n", StartEventCreator, None),
            KeyBinding::new("cmd-alt-n", StartRoutineCreator, None),
        ]);

        Self {
            pipeline_view,
            backlog_view,
            backlog_hover,
            current_overlay,
        }
    }

    pub fn render_backlog(
        &self,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .h_full()
            .w(width)
            // spacer with the same height as the tab bar
            .child(div().h_10().w(width))
            .child(
                div()
                    .id("backlog")
                    .flex_1()
                    .rounded_xl()
                    .on_hover(cx.listener(move |view, hovered, _window, cx| {
                        view.backlog_hover = Some(*hovered);
                        cx.notify();
                    }))
                    .on_drag_move(cx.listener(
                        move |view, event: &DragMoveEvent<DragData<AnyItem>>, _window, cx| {
                            let is_over = event.bounds.contains(&event.event.position);
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
                    ),
            )
    }
}

impl EventEmitter<()> for RootView {}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let backlog_transition = window
            .use_keyed_transition("backlog-width", cx, Duration::from_millis(150), |_, _| {
                px(24.)
            })
            .with_easing(ease_out_cubic);
        let backlog_width = *backlog_transition.evaluate(window, cx);
        if let Some(hovered) = self.backlog_hover.take() {
            backlog_transition.update(cx, |value, _cx| {
                *value = if hovered { px(256.) } else { px(24.) }
            });
            window.request_animation_frame();
        }

        v_flex()
            .absolute()
            .inset_0()
            // .pt_8()
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
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .size_full()
                    // .w_3_4()
                    // .h_3_4()
                    .child(
                        h_flex()
                            .flex_1()
                            .child(self.pipeline_view.clone())
                            // .child(self.render_backlog(backlog_width, window, cx))
                            .when(false, |this| this),
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
