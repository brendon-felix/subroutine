use std::{
    ops::{Deref, Range},
    rc::Rc,
};

use gpui::{
    Along, AnyElement, App, AppContext, Axis, Bounds, Context, Element, ElementId, Empty, Entity,
    EventEmitter, InteractiveElement as _, IntoElement, IsZero as _, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Render, RenderOnce, Style, Styled, Window, canvas, div,
    prelude::FluentBuilder, px,
};

use gpui_component::{AxisExt, h_flex, v_flex};

use crate::components::resizable::ElementExt;

use super::{ResizableState, panel_slide_transition, resize_handle};

pub const PANEL_MIN_SIZE: Pixels = px(50.);

pub enum ResizablePanelEvent {
    Resized,
}

#[derive(Clone)]
pub(crate) struct DragPanel;
impl Render for DragPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        Empty
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResizablePanelState {
    pub size: Pixels,
    pub size_range: Range<Pixels>,
    pub(crate) bounds: Bounds<Pixels>,
    /// Whether the panel is in the open state. Transitions animate toward this.
    pub(crate) is_open: bool,
    /// The size the panel had when it was last open, used to restore after reopening.
    pub(crate) open_size: Pixels,
}

/// A resizable panel inside a [`ResizablePanelGroup`].
#[derive(IntoElement)]
pub struct ResizablePanel {
    parent_state: Entity<ResizableState>,
    children: Vec<AnyElement>,
    visible: bool,
    pub(super) panel_ix: usize,
    pub(super) initial_size: Option<Pixels>,
    size_range: Range<Pixels>,
    pub(super) collapsible: bool,
}

impl ResizablePanel {
    /// Create a new resizable panel.
    pub fn new(parent_state: Entity<ResizableState>, cx: &mut App) -> Self {
        let panel_state = ResizablePanelState {
            size: px(0.),
            size_range: PANEL_MIN_SIZE..Pixels::MAX,
            bounds: Bounds::default(),
            is_open: true,
            open_size: px(0.),
        };
        parent_state.update(cx, |state, _| {
            state.panels.push(panel_state);
        });
        Self {
            parent_state,
            children: vec![],
            visible: true,
            panel_ix: 0,
            initial_size: None,
            size_range: px(10.)..Pixels::MAX,
            collapsible: false,
        }
    }

    // /// Set the visibility of the panel, default is true.
    // pub fn visible(mut self, visible: bool) -> Self {
    //     self.visible = visible;
    //     self
    // }

    /// Set the initial size of the panel.
    pub fn initial_size(mut self, size: impl Into<Pixels>) -> Self {
        self.initial_size = Some(size.into());
        self
    }

    /// Set the size range to limit panel resize.
    ///
    /// Default is [`PANEL_MIN_SIZE`] to [`Pixels::MAX`].
    pub fn size_range(mut self, range: impl Into<Range<Pixels>>) -> Self {
        self.size_range = range.into();
        self
    }

    /// Allow this panel to be fully collapsed via [`ResizableState::toggle_panel`].
    ///
    /// This zeroes the minimum size constraint so the animated slide can reach zero.
    pub fn collapsible(mut self) -> Self {
        self.collapsible = true;
        self
    }
}

impl ParentElement for ResizablePanel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ResizablePanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if !self.visible {
            return div().id(("resizable-panel", self.panel_ix));
        }

        let parent_state = self.parent_state.read(cx);
        let panel_state = &parent_state.panels[self.panel_ix];

        let size_range = self.size_range;
        let axis = parent_state.axis;

        div()
            .id(("resizable-panel", self.panel_ix))
            .flex()
            .flex_grow()
            .size_full()
            .relative()
            .when(axis.is_vertical(), |this| {
                let min = if self.collapsible {
                    px(0.)
                } else {
                    size_range.start
                };
                this.min_h(min).max_h(size_range.end)
            })
            .when(axis.is_horizontal(), |this| {
                let min = if self.collapsible {
                    px(0.)
                } else {
                    size_range.start
                };
                this.min_w(min).max_w(size_range.end)
            })
            .when(self.initial_size.is_none(), |this| this.flex_shrink())
            .when_some(self.initial_size, |this, initial_size| {
                this.when(!initial_size.is_zero(), |this| {
                    this.flex_none().flex_basis(initial_size)
                })
            })
            .map(|this| {
                let clamped = if self.collapsible {
                    panel_state.size.min(size_range.end)
                } else {
                    panel_state.size.min(size_range.end).max(size_range.start)
                };
                this.flex_basis(clamped)
            })
            .on_prepaint({
                let state = self.parent_state.clone();
                move |bounds, _, cx| {
                    state.update(cx, |state, cx| {
                        state.update_panel_size(self.panel_ix, bounds, size_range, cx)
                    })
                }
            })
            .children(self.children)
            .when(self.panel_ix > 0, |this| {
                let ix = self.panel_ix - 1;
                this.child(resize_handle(("resizable-handle", ix), axis).on_drag(
                    DragPanel,
                    move |drag_panel, _, _, cx| {
                        cx.stop_propagation();
                        // Set current resizing panel ix
                        self.parent_state.update(cx, |state, _| {
                            state.resizing_panel_ix = Some(ix);
                        });
                        cx.new(|_| drag_panel.deref().clone())
                    },
                ))
            })
    }
}
