use std::ops::Range;

use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, px,
};

#[derive(Clone)]
pub struct SidePanelState {
    /// The allowed range of pixel widths for this panel when open, used to clamp during resizing.
    pub width_range: Range<Pixels>,
    /// The proportion this panel occupies when it is open.
    pub opened_proportion: f32,
    /// Whether the panel is open right now (used to drive the slide transition goal).
    pub open: bool,
}

impl Default for SidePanelState {
    fn default() -> Self {
        Self {
            width_range: px(10.)..Pixels::MAX,
            opened_proportion: 0.25,
            open: true,
        }
    }
}

#[derive(IntoElement)]
pub struct SidePanel {
    pub base: gpui::Stateful<gpui::Div>,
    pub width_range: Range<Pixels>,
    pub initial_proportion: f32,
    // start_open: bool,
}

impl SidePanel {
    fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: div().id(id),
            width_range: px(10.)..Pixels::MAX,
            initial_proportion: 0.25,
            // start_open: true,
        }
    }

    pub fn left() -> Self {
        Self::new("left-panel")
    }

    pub fn right() -> Self {
        Self::new("right-panel")
    }

    pub fn width_range_open(mut self, range: Range<Pixels>) -> Self {
        self.width_range = range;
        self
    }

    // pub fn initial_proportion(mut self, width: f32) -> Self {
    //     self.initial_proportion = width;
    //     self
    // }

    // pub fn start_open(mut self, open: bool) -> Self {
    //     self.start_open = open;
    //     self
    // }
}

impl Styled for SidePanel {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for SidePanel {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for SidePanel {}

impl ParentElement for SidePanel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for SidePanel {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.base.size_full()
    }
}
