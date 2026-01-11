use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    AnyElement, App, Axis, Element, ElementId, Entity, GlobalElementId, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
    Render, StatefulInteractiveElement, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use gpui_component::{ActiveTheme as _, AxisExt as _, dock::DockPlacement};
use gpui_transitions::WindowUseTransition;

use crate::transitions::ease_in_out;

const HOVER_FADE_STRENGTH: f32 = 0.8;
pub(crate) const HANDLE_PADDING: Pixels = px(4.);
pub(crate) const HANDLE_WIDTH: Pixels = px(4.);
pub(crate) const HANDLE_LENGTH: Pixels = px(42.);

/// Create a resize handle for a resizable panel.
pub(crate) fn resize_handle<T: 'static, E: 'static + Render>(
    id: impl Into<ElementId>,
    axis: Axis,
) -> ResizeHandle<T, E> {
    ResizeHandle::new(id, axis)
}

pub(crate) struct ResizeHandle<T: 'static, E: 'static + Render> {
    id: ElementId,
    axis: Axis,
    drag_value: Option<Rc<T>>,
    placement: Option<DockPlacement>,
    on_drag: Option<Rc<dyn Fn(&Point<Pixels>, &mut Window, &mut App) -> Entity<E>>>,
}

impl<T: 'static, E: 'static + Render> ResizeHandle<T, E> {
    fn new(id: impl Into<ElementId>, axis: Axis) -> Self {
        let id = id.into();
        Self {
            id: id.clone(),
            on_drag: None,
            drag_value: None,
            placement: None,
            axis,
        }
    }

    pub(crate) fn on_drag(
        mut self,
        value: T,
        f: impl Fn(Rc<T>, &Point<Pixels>, &mut Window, &mut App) -> Entity<E> + 'static,
    ) -> Self {
        let value = Rc::new(value);
        self.drag_value = Some(value.clone());
        self.on_drag = Some(Rc::new(move |p, window, cx| {
            f(value.clone(), p, window, cx)
        }));
        self
    }

    // pub(crate) fn placement(mut self, placement: DockPlacement) -> Self {
    //     self.placement = Some(placement);
    //     self
    // }
}

#[derive(Default, Debug, Clone)]
struct ResizeHandleState {
    active: Cell<bool>,
}

impl ResizeHandleState {
    fn set_active(&self, active: bool) {
        self.active.set(active);
    }

    fn is_active(&self) -> bool {
        self.active.get()
    }
}

impl<T: 'static, E: 'static + Render> IntoElement for ResizeHandle<T, E> {
    type Element = ResizeHandle<T, E>;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl<T: 'static, E: 'static + Render> Element for ResizeHandle<T, E> {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let neg_offset = -HANDLE_PADDING;
        let axis = self.axis;

        window.with_element_state(id.unwrap(), |state, window| {
            let state = state.unwrap_or(ResizeHandleState::default());

            // let bg_color = if state.is_active() {
            //     cx.theme().drag_border
            // } else {
            //     cx.theme().border
            // };

            let hover_transition = window
                .use_keyed_transition("hover", cx, Duration::from_millis(150), |_window, _cx| 0.)
                .with_easing(ease_in_out);

            let hover = *hover_transition.evaluate(window, cx);
            let hover_amount = hover * HOVER_FADE_STRENGTH;
            // let bg_color = gpui::hsla(0., 0., 1. - hover_amount, 1.);
            let bg_color = cx
                .theme()
                .group_box
                .blend(cx.theme().primary_foreground.alpha(hover_amount));
            // let border_color = gpui::hsla(0., 0.8, 0.8 - hover_amount * 0.5, 1.);

            let mut el = div()
                .id(self.id.clone())
                .occlude()
                .absolute()
                .flex_shrink_0()
                // .group("handle")
                .on_hover(move |hover, _window, cx| {
                    hover_transition.update(cx, |this, cx| {
                        *this = *hover as u8 as f32;
                        cx.notify();
                    });
                })
                .on_mouse_down(MouseButton::Left, |_, _window, _cx| {})
                .when_some(self.on_drag.clone(), |this, on_drag| {
                    this.on_drag(
                        self.drag_value.clone().unwrap(),
                        move |_, position, window, cx| on_drag(&position, window, cx),
                    )
                })
                .map(|this| match self.placement {
                    Some(DockPlacement::Left) => {
                        // Special for Left Dock
                        //  FIXME: Improve this to let the scroll bar have px(HANDLE_PADDING)
                        this.cursor_col_resize()
                            .top_0()
                            .right(px(1.))
                            .h_full()
                            // .h(HANDLE_LENGTH)
                            .w(HANDLE_WIDTH)
                            .pl(HANDLE_PADDING)
                    }
                    _ => this
                        .when(axis.is_horizontal(), |this| {
                            this.cursor_col_resize()
                                .top_0()
                                .left(neg_offset)
                                .h_full()
                                // .h(HANDLE_LENGTH)
                                .w(HANDLE_WIDTH)
                                .px(HANDLE_PADDING)
                        })
                        .when(axis.is_vertical(), |this| {
                            this.cursor_row_resize()
                                .top(neg_offset)
                                .left_0()
                                .w_full()
                                // .w(HANDLE_LENGTH)
                                .h(HANDLE_WIDTH)
                                .py(HANDLE_PADDING)
                        }),
                })
                .child(
                    div()
                        // .debug_below()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        // .bg(gpui::blue())
                        .child(
                            div()
                                .absolute()
                                .rounded_xs()
                                // .size_full()
                                // .group_hover("handle", |this| this.bg(gpui::white()))
                                .bg(bg_color)
                                // .border_color(bg_color)
                                .when(axis.is_horizontal(), |this| {
                                    this.h(HANDLE_LENGTH).w(HANDLE_WIDTH)
                                })
                                .when(axis.is_vertical(), |this| {
                                    this.w(HANDLE_LENGTH).h(HANDLE_WIDTH)
                                }),
                            // .h_9()
                            // .w_1(),
                        ),
                )
                .into_any_element();

            let layout_id = el.request_layout(window, cx);

            ((layout_id, el), state)
        })
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: gpui::Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        request_layout.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: gpui::Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        request_layout.paint(window, cx);

        window.with_element_state(id.unwrap(), |state: Option<ResizeHandleState>, window| {
            let state = state.unwrap_or(ResizeHandleState::default());

            window.on_mouse_event({
                let state = state.clone();
                move |ev: &MouseDownEvent, phase, window, _| {
                    if bounds.contains(&ev.position) && phase.bubble() {
                        state.set_active(true);
                        window.refresh();
                    }
                }
            });

            window.on_mouse_event({
                let state = state.clone();
                move |_: &MouseUpEvent, _, window, _| {
                    if state.is_active() {
                        state.set_active(false);
                        window.refresh();
                    }
                }
            });

            ((), state)
        });
    }
}
