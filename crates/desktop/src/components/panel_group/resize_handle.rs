use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    AnyElement, App, Axis, Element, ElementId, Entity, GlobalElementId, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
    Render, StatefulInteractiveElement, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use gpui_component::{ActiveTheme as _, AxisExt as _};
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
    /// Explicit left offset for when the handle is rendered as an absolute overlay
    /// on a parent container rather than inside one of the panels.
    offset: Option<Pixels>,
    /// When set, the handle is positioned as an absolute overlay measured from the
    /// right edge of the parent container (for the right panel boundary).
    offset_from_right: Option<Pixels>,
    /// When true, the visual indicator is drawn on the left side (for right panels).
    left_side: bool,
    drag_value: Option<Rc<T>>,
    on_drag: Option<Rc<dyn Fn(&Point<Pixels>, &mut Window, &mut App) -> Entity<E>>>,
}

impl<T: 'static, E: 'static + Render> ResizeHandle<T, E> {
    fn new(id: impl Into<ElementId>, axis: Axis) -> Self {
        let id = id.into();
        Self {
            id,
            axis,
            offset: None,
            offset_from_right: None,
            left_side: false,
            on_drag: None,
            drag_value: None,
        }
    }

    /// Render the handle as an absolute overlay at the given left offset within
    /// the parent container. Use this when the handle is a child of the group
    /// container rather than of a panel, so it paints on top of both panels.
    pub(crate) fn left(mut self, offset: Pixels) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Render the handle as an absolute overlay at the given offset from the right
    /// edge of the parent container. Use this for the right panel boundary so the
    /// handle straddles the center/right panel edge and paints on top of both.
    pub(crate) fn right(mut self, offset_from_right: Pixels) -> Self {
        self.offset_from_right = Some(offset_from_right);
        self.left_side = true;
        self
    }

    /// Position the visual indicator on the left side of the hit area.
    /// Used for the right-side panel whose handle sits on its left boundary.
    pub(crate) fn left_side(mut self) -> Self {
        self.left_side = true;
        self
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
        let axis = self.axis;
        let left_side = self.left_side;

        window.with_element_state(id.unwrap(), |state, window| {
            let state = state.unwrap_or(ResizeHandleState::default());

            let hover_transition = window
                .use_keyed_transition("hover", cx, Duration::from_millis(150), |_, _| 0.0_f32)
                .with_easing(ease_in_out);

            let hover = *hover_transition.evaluate(window, cx);
            let bg_color = cx
                .theme()
                .primary_foreground
                .alpha(hover * HOVER_FADE_STRENGTH);

            let mut el = div()
                .id(self.id.clone())
                .occlude()
                .absolute()
                .flex_shrink_0()
                .on_hover(move |is_hovered, _window, cx| {
                    hover_transition.update(cx, |goal, cx| {
                        *goal = *is_hovered as u8 as f32;
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
                .when(axis.is_horizontal(), |this| {
                    this.cursor_col_resize()
                        .top_0()
                        .h_full()
                        // When an explicit offset is given the handle is an overlay on the
                        // group container, so widen the hit area symmetrically around the
                        // boundary. Otherwise it sits flush inside the panel.
                        .map(|d| match (self.offset, self.offset_from_right) {
                            (Some(offset), _) => d
                                .absolute()
                                .left(offset - HANDLE_PADDING)
                                .w(HANDLE_PADDING * 2.),
                            (_, Some(offset_from_right)) => d
                                .absolute()
                                .right(offset_from_right - HANDLE_PADDING)
                                .w(HANDLE_PADDING * 2.),
                            (None, None) => d
                                .w(HANDLE_WIDTH)
                                .px(HANDLE_PADDING)
                                .when(left_side, |d| d.left_0())
                                .when(!left_side, |d| d.right_0()),
                        })
                })
                .when(axis.is_vertical(), |this| {
                    this.cursor_row_resize()
                        .top(-HANDLE_PADDING)
                        .left_0()
                        .w_full()
                        .h(HANDLE_WIDTH)
                        .py(HANDLE_PADDING)
                })
                .child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .absolute()
                                .rounded_xs()
                                .bg(bg_color)
                                .when(axis.is_horizontal(), |this| {
                                    this.h(HANDLE_LENGTH).w(HANDLE_WIDTH)
                                })
                                .when(axis.is_vertical(), |this| {
                                    this.w(HANDLE_LENGTH).h(HANDLE_WIDTH)
                                }),
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
