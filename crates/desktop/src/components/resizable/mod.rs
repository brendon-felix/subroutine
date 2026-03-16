use std::{ops::Range, rc::Rc, time::Duration};

use gpui::{
    Along, App, Axis, Bounds, Context, Element, ElementId, Entity, EventEmitter,
    InteractiveElement, IntoElement, IsZero, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    RenderOnce, Style, Styled, Window, canvas, px,
};

use gpui_component::{AxisExt, PixelsExt, h_flex, v_flex};
use gpui_transitions::WindowUseTransition;

use crate::transitions::ease_in_out;

mod panel;
mod resize_handle;
pub use panel::*;
pub(crate) use resize_handle::*;

/// A trait to extend [`gpui::Element`] with additional functionality.
pub trait ElementExt: ParentElement + Sized {
    /// Add a prepaint callback to the element.
    ///
    /// This is a helper method to get the bounds of the element after paint.
    ///
    /// The first argument is the bounds of the element in pixels.
    ///
    /// See also [`gpui::canvas`].
    fn on_prepaint<F>(self, f: F) -> Self
    where
        F: FnOnce(Bounds<Pixels>, &mut Window, &mut App) + 'static,
    {
        self.child(
            canvas(
                move |bounds, window, cx| f(bounds, window, cx),
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
    }
}

impl<T: ParentElement> ElementExt for T {}

/// State for a [`ResizablePanel`]
#[derive(Debug, Clone)]
pub struct ResizableState {
    /// The `axis` will sync to actual axis of the ResizablePanelGroup in use.
    axis: Axis,
    pub(crate) panels: Vec<ResizablePanelState>,
    pub(crate) resizing_panel_ix: Option<usize>,
    bounds: Bounds<Pixels>,
}

// impl Default for ResizableState {
//     fn default() -> Self {
//         Self {
//             axis: Axis::Horizontal,
//             panels: vec![],
//             resizing_panel_ix: None,
//             bounds: Bounds::default(),
//         }
//     }
// }

impl ResizableState {
    pub(crate) fn new(axis: Axis) -> Self {
        Self {
            axis,
            panels: vec![],
            resizing_panel_ix: None,
            bounds: Bounds::default(),
        }
    }

    /// Get the size of the panels.
    // pub fn sizes(&self) -> &Vec<Pixels> {
    //     &self.sizes
    // }

    // pub(crate) fn insert_panel(
    //     &mut self,
    //     size: Option<Pixels>,
    //     ix: Option<usize>,
    //     cx: &mut Context<Self>,
    // ) {
    //     let panel_state = ResizablePanelState {
    //         size,
    //         ..Default::default()
    //     };

    //     let size = size.unwrap_or(PANEL_MIN_SIZE);

    //     // We make sure that the size always sums up to the container size
    //     // by reducing the size of all other panels first.
    //     let container_size = self.container_size().max(px(1.));
    //     let total_leftover_size = (container_size - size).max(px(1.));

    //     for (i, panel) in self.panels.iter_mut().enumerate() {
    //         let ratio = self.sizes[i] / container_size;
    //         self.sizes[i] = total_leftover_size * ratio;
    //         panel.size = Some(self.sizes[i]);
    //     }

    //     if let Some(ix) = ix {
    //         self.panels.insert(ix, panel_state);
    //         self.sizes.insert(ix, size);
    //     } else {
    //         self.panels.push(panel_state);
    //         self.sizes.push(size);
    //     };

    //     cx.notify();
    // }

    // pub(crate) fn sync_panels_count(
    //     &mut self,
    //     axis: Axis,
    //     panels_count: usize,
    //     cx: &mut Context<Self>,
    // ) {
    //     let mut changed = self.axis != axis;
    //     self.axis = axis;

    //     if panels_count > self.panels.len() {
    //         let diff = panels_count - self.panels.len();
    //         self.panels
    //             .extend(vec![ResizablePanelState::default(); diff]);
    //         self.sizes.extend(vec![PANEL_MIN_SIZE; diff]);
    //         changed = true;
    //     }

    //     if panels_count < self.panels.len() {
    //         self.panels.truncate(panels_count);
    //         self.sizes.truncate(panels_count);
    //         changed = true;
    //     }

    //     if changed {
    //         // We need to make sure the total size is in line with the container size.
    //         self.adjust_to_container_size(cx);
    //     }
    // }

    pub(crate) fn update_panel_size(
        &mut self,
        panel_ix: usize,
        bounds: Bounds<Pixels>,
        size_range: Range<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let size = bounds.size.along(self.axis);
        // This check is only necessary to stop the very first panel from resizing on its own
        // it needs to be passed when the panel is freshly created so we get the initial size,
        // but its also fine when it sometimes passes later.
        if self.panels[panel_ix].size.as_f32() == PANEL_MIN_SIZE.as_f32() {
            self.panels[panel_ix].size = size;
        }
        self.panels[panel_ix].bounds = bounds;
        self.panels[panel_ix].size_range = size_range;
        cx.notify();
    }

    // pub(crate) fn remove_panel(&mut self, panel_ix: usize, cx: &mut Context<Self>) {
    //     self.panels.remove(panel_ix);
    //     self.sizes.remove(panel_ix);
    //     if let Some(resizing_panel_ix) = self.resizing_panel_ix {
    //         if resizing_panel_ix > panel_ix {
    //             self.resizing_panel_ix = Some(resizing_panel_ix - 1);
    //         }
    //     }
    //     self.adjust_to_container_size(cx);
    // }

    // pub(crate) fn replace_panel(
    //     &mut self,
    //     panel_ix: usize,
    //     panel: ResizablePanelState,
    //     cx: &mut Context<Self>,
    // ) {
    //     let old_size = self.sizes[panel_ix];

    //     self.panels[panel_ix] = panel;
    //     self.sizes[panel_ix] = old_size;
    //     self.adjust_to_container_size(cx);
    // }

    // pub(crate) fn clear(&mut self) {
    //     self.panels.clear();
    //     self.sizes.clear();
    // }

    pub(crate) fn container_size(&self) -> Pixels {
        self.bounds.size.along(self.axis)
    }

    pub(crate) fn start_resizing(&mut self, panel_ix: usize) {
        self.resizing_panel_ix = Some(panel_ix);
    }

    pub(crate) fn done_resizing(&mut self, cx: &mut Context<Self>) {
        self.resizing_panel_ix = None;
        cx.emit(ResizablePanelEvent::Resized);
    }

    // fn sync_real_panel_sizes(&mut self, _: &App) {
    //     for (i, panel) in self.panels.iter().enumerate() {
    //         self.sizes[i] = panel.bounds.size.along(self.axis);
    //     }
    // }

    /// The `ix`` is the index of the panel to resize,
    /// and the `size` is the new size for the panel.
    fn resize_panel(&mut self, ix: usize, size: Pixels, _: &mut Window, cx: &mut Context<Self>) {
        // let old_sizes = self.panels.iter().map(|p| p.size).clone();
        let old_size = self.panels[ix].size;
        let new_size = size;

        // don't resize the last panel, it should just take up the remaining space
        if ix >= self.panels.len() - 1 {
            return;
        }
        let container_size = self.container_size();
        // self.sync_real_panel_sizes(cx);

        let delta = new_size - old_size;
        if delta.is_zero() {
            return;
        }
    }

    /// Adjust panel sizes according to the container size.
    ///
    /// When the container size changes, the panels should take up the same percentage as they did before.
    fn adjust_to_container_size(&mut self, cx: &mut Context<Self>) {
        if self.container_size().is_zero() {
            return;
        }

        let container_size = self.container_size();
        let total_size = px(self.panels.iter().map(|p| p.size.as_f32()).sum::<f32>());

        for i in 0..self.panels.len() {
            let size = self.panels[i].size;
            let ratio = size / total_size;
            let new_size = container_size * ratio;

            self.panels[i].size = new_size;
        }
        cx.notify();
    }

    /// Toggle a panel open or closed, saving its current real size before closing
    /// so it can be restored when reopened.
    ///
    /// Returns the new `is_open` state so the caller can update the transition goal.
    pub fn toggle_panel(&mut self, panel_ix: usize, cx: &mut Context<Self>) -> bool {
        let Some(panel) = self.panels.get_mut(panel_ix) else {
            return true;
        };
        if panel.is_open {
            // // Save the real rendered size before the transition closes the panel.
            // if let Some(current_size) = self.sizes.get(panel_ix).copied() {
            //     if current_size > px(0.) {
            //         self.panels[panel_ix].open_size = current_size;
            //     }
            // }
            self.panels[panel_ix].is_open = false;
        } else {
            self.panels[panel_ix].is_open = true;
        }
        cx.notify();
        self.panels[panel_ix].is_open
    }
}

impl EventEmitter<ResizablePanelEvent> for ResizableState {}

// impl Default for ResizablePanelState {
//     fn default() -> Self {
//         Self {
//             size: None,
//             size_range: Range::default(),
//             bounds: Bounds::default(),
//             is_open: true,
//             open_size: None,
//         }
//     }
// }

/// A group of resizable panels.
#[derive(IntoElement)]
pub struct ResizablePanelGroup {
    id: ElementId,
    state: Entity<ResizableState>,
    // size: Option<Pixels>,
    children: Vec<ResizablePanel>,
    on_resize: Rc<dyn Fn(&Entity<ResizableState>, &mut Window, &mut App)>,
}

impl ResizablePanelGroup {
    /// Create a new resizable panel group.
    pub fn new(id: impl Into<ElementId>, state: Entity<ResizableState>) -> Self {
        Self {
            id: id.into(),
            children: vec![],
            state,
            // size: None,
            on_resize: Rc::new(|_, _, _| {}),
        }
    }

    pub fn child(mut self, panel: impl Into<ResizablePanel>) -> Self {
        self.children.push(panel.into());
        self
    }

    // /// Add multiple panels to the group.
    // pub fn children<I>(mut self, panels: impl IntoIterator<Item = I>) -> Self
    // where
    //     I: Into<ResizablePanel>,
    // {
    //     self.children = panels.into_iter().map(|panel| panel.into()).collect();
    //     self
    // }

    // /// Set size of the resizable panel group
    // ///
    // /// - When the axis is horizontal, the size is the height of the group.
    // /// - When the axis is vertical, the size is the width of the group.
    // pub fn size(mut self, size: Pixels) -> Self {
    //     self.size = Some(size);
    //     self
    // }

    /// Set the callback to be called when the panels are resized.
    ///
    /// ## Callback arguments
    ///
    /// - Entity<ResizableState>: The state of the ResizablePanelGroup.
    pub fn on_resize(
        mut self,
        on_resize: impl Fn(&Entity<ResizableState>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize = Rc::new(on_resize);
        self
    }
}

impl EventEmitter<ResizablePanelEvent> for ResizablePanelGroup {}

impl RenderOnce for ResizablePanelGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let axis = self.state.read(cx).axis;
        let container = if axis.is_horizontal() {
            h_flex()
        } else {
            v_flex()
        };

        // // Sync panels to the state
        // let panels_count = self.children.len();
        // state.update(cx, |state, cx| {
        //     state.sync_panels_count(self.axis, panels_count, cx);
        // });

        // For each collapsible panel, create its keyed transition and evaluate the
        // current animated size, then write it into ResizableState so the layout
        // sees it this frame.
        for (ix, panel) in self.children.iter().enumerate() {
            if panel.collapsible {
                let animated_size = panel_slide_transition(
                    &self.id,
                    ix,
                    panel.initial_size,
                    &self.state,
                    window,
                    cx,
                );
                self.state.update(cx, |state, _| {
                    if let Some(panel_state) = state.panels.get_mut(ix) {
                        panel_state.size = animated_size;
                    }
                });
            }
        }

        container
            .id(self.id)
            .size_full()
            .children(
                self.children
                    .into_iter()
                    .enumerate()
                    .map(|(ix, mut panel)| {
                        panel.panel_ix = ix;
                        panel
                    }),
            )
            .on_prepaint({
                let state = self.state.clone();
                move |bounds, _, cx| {
                    state.update(cx, |state, cx| {
                        let size_changed = state.bounds.size.along(axis) != bounds.size.along(axis);

                        state.bounds = bounds;

                        if size_changed {
                            state.adjust_to_container_size(cx);
                        }
                    })
                }
            })
            .child(ResizePanelGroupElement {
                state: self.state.clone(),
                axis: axis,
                on_resize: self.on_resize.clone(),
            })
    }
}

/// Create the slide transition for a collapsible panel, drive its goal from the
/// current `is_open` state, and return the current animated size.
///
/// This is called every render. It is the correct place to call `transition.update`
/// because `use_keyed_transition` is only valid during render / request_layout.
pub(crate) fn panel_slide_transition(
    group_id: &ElementId,
    panel_ix: usize,
    initial_size: Option<Pixels>,
    state: &Entity<ResizableState>,
    window: &mut Window,
    cx: &mut App,
) -> Pixels {
    let panel = &state.read(cx).panels[panel_ix];
    let is_open = panel.is_open;
    let open_size = panel.open_size;

    let slide_key: ElementId = (group_id.clone(), format!("{panel_ix}-slide")).into();
    let transition = window
        .use_keyed_transition(slide_key, cx, Duration::from_millis(200), |_, _| 1.0)
        .with_easing(ease_in_out);

    let goal: f32 = if is_open { 1.0 } else { 0.0 };
    transition.update(cx, |current_goal, cx| {
        *current_goal = goal;
        cx.notify();
    });

    let progress = *transition.evaluate(window, cx);
    open_size * progress
}

struct ResizePanelGroupElement {
    state: Entity<ResizableState>,
    on_resize: Rc<dyn Fn(&Entity<ResizableState>, &mut Window, &mut App)>,
    axis: Axis,
}

impl IntoElement for ResizePanelGroupElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ResizePanelGroupElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        (window.request_layout(Style::default(), None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        ()
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.on_mouse_event({
            let state = self.state.clone();
            let axis = self.axis;
            let current_ix = state.read(cx).resizing_panel_ix;
            move |e: &MouseMoveEvent, phase, window, cx| {
                if !phase.bubble() {
                    return;
                }
                let Some(ix) = current_ix else { return };

                state.update(cx, |state, cx| {
                    let panel = &state.panels[ix];

                    match axis {
                        Axis::Horizontal => {
                            state.resize_panel(ix, e.position.x - panel.bounds.left(), window, cx)
                        }
                        Axis::Vertical => {
                            state.resize_panel(ix, e.position.y - panel.bounds.top(), window, cx);
                        }
                    }
                    cx.notify();
                })
            }
        });

        // When any mouse up, stop dragging
        window.on_mouse_event({
            let state = self.state.clone();
            let current_ix = state.read(cx).resizing_panel_ix;
            let on_resize = self.on_resize.clone();
            move |_: &MouseUpEvent, phase, window, cx| {
                if current_ix.is_none() {
                    return;
                }
                if phase.bubble() {
                    state.update(cx, |state, cx| state.done_resizing(cx));
                    on_resize(&state, window, cx);
                }
            }
        })
    }
}
