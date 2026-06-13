use std::time::Duration;

use gpui::{
    App, AppContext as _, Bounds, Context, Element, ElementId, Empty, Entity, IntoElement,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, RenderOnce, Style,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{ElementExt, StyledExt, h_flex};

use crate::transitions::ease_in_out;

mod center_panel;
mod navigation_bar;
mod resize_handle;
mod side_panel;
pub use center_panel::*;
pub use navigation_bar::*;
use resize_handle::*;
pub use side_panel::*;

const SLIDE_DURATION_MS: u64 = 100;

/// Which side panel (if any) is currently being dragged by the user.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ResizingSide {
    Left,
    Right,
}

#[derive(Clone, Default)]
pub struct PanelGroupState {
    pub center_panel: CenterPanelState,
    pub left_panel: Option<SidePanelState>,
    pub right_panel: Option<SidePanelState>,
    /// Tracks which side is being dragged, and the container pixel width at drag start.
    resizing: Option<(ResizingSide, Pixels, Bounds<Pixels>)>,
    /// Current pixel bounds of the whole group, written each frame during prepaint.
    bounds: Bounds<Pixels>,
    /// Container width from the previous rendered frame, used to detect window resizes.
    prev_container_width: Pixels,
    /// Animated pixel width of the left panel, written every render frame.
    pub animated_left_px: Pixels,
    /// Animated pixel width of the right panel, written every render frame.
    pub animated_right_px: Pixels,
}

impl PanelGroupState {
    pub fn toggle_left(&mut self) {
        if let Some(panel) = self.left_panel.as_mut() {
            panel.open = !panel.open;
        }
    }

    pub fn toggle_right(&mut self) {
        if let Some(panel) = self.right_panel.as_mut() {
            panel.open = !panel.open;
        }
    }

    // pub fn container_width(&self) -> Pixels {
    //     self.bounds.size.width
    // }

    // fn total_side_proportion(&self) -> f32 {
    //     let left = self
    //         .left_panel
    //         .as_ref()
    //         .filter(|p| p.open)
    //         .map(|p| p.opened_proportion)
    //         .unwrap_or(0.0);
    //     let right = self
    //         .right_panel
    //         .as_ref()
    //         .filter(|p| p.open)
    //         .map(|p| p.opened_proportion)
    //         .unwrap_or(0.0);
    //     left + right
    // }

    /// Resize the left panel given the new mouse x position in window coordinates.
    fn resize_left(&mut self, mouse_x: Pixels) {
        let container_width = self.bounds.size.width;
        if container_width <= px(0.) {
            return;
        }
        let new_proportion = (mouse_x - self.bounds.left()) / container_width;
        let panel = match self.left_panel.as_mut() {
            Some(p) => p,
            None => return,
        };
        // let right_proportion = self
        //     .right_panel
        //     .as_ref()
        //     .filter(|p| p.open)
        //     .map(|p| p.opened_proportion)
        //     .unwrap_or(0.0);
        // let max = (1.0 - self.center_panel.min_proportion - right_proportion)
        //     .max(panel.proportion_range.start);
        let min_proportion = panel.width_range.start / container_width;
        let max_proportion = panel.width_range.end / container_width;
        let clamped = new_proportion.max(min_proportion).min(max_proportion);
        panel.opened_proportion = clamped;
    }

    /// Resize the right panel given the new mouse x position in window coordinates.
    fn resize_right(&mut self, mouse_x: Pixels) {
        let container_width = self.bounds.size.width;
        if container_width <= px(0.) {
            return;
        }
        let mouse_from_right = self.bounds.right() - mouse_x;
        let new_proportion = mouse_from_right / container_width;
        let panel = match self.right_panel.as_mut() {
            Some(p) => p,
            None => return,
        };
        let left_proportion = self
            .left_panel
            .as_ref()
            .filter(|p| p.open)
            .map(|p| p.opened_proportion)
            .unwrap_or(0.0);
        let center_proportion = self.center_panel.min_width / container_width;
        let min_proportion = panel.width_range.start / container_width;
        let max_proportion = panel.width_range.end / container_width;
        let max = (1.0 - center_proportion - left_proportion).max(min_proportion);
        let clamped = new_proportion
            .max(min_proportion)
            .min(max_proportion.min(max));
        panel.opened_proportion = clamped;
    }
}

// ── PanelGroup ────────────────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct PanelGroup {
    state: Entity<PanelGroupState>,
    style: StyleRefinement,
    center: CenterPanel,
    left: Option<SidePanel>,
    right: Option<SidePanel>,
}

impl PanelGroup {
    pub fn new(state: Entity<PanelGroupState>) -> Self {
        Self {
            state,
            style: StyleRefinement::default(),
            center: CenterPanel::new(),
            left: None,
            right: None,
        }
    }

    pub fn center(mut self, panel: CenterPanel) -> Self {
        self.center = panel;
        self
    }

    pub fn left(mut self, panel: SidePanel) -> Self {
        self.left = Some(panel);
        self
    }

    pub fn right(mut self, panel: SidePanel) -> Self {
        self.right = Some(panel);
        self
    }
}

impl Styled for PanelGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for PanelGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.clone();
        let weak_state = state.downgrade();

        // Capture initial_proportion from panels before they're consumed.
        let left_initial_proportion = self.left.as_ref().map(|p| p.initial_proportion);
        let right_initial_proportion = self.right.as_ref().map(|p| p.initial_proportion);

        // Read current open/proportion state for layout calculations.
        let group_state = state.read(cx).clone();
        let container_width = group_state.bounds.size.width;

        // ── compute animated pixel widths via slide transitions ───────────────

        // open_px is the full width the panel occupies when open — always derived from
        // opened_proportion regardless of the current open/closed state. This is used
        // to size the inner content so it never resizes during a transition.
        let left_open_px = group_state
            .left_panel
            .as_ref()
            .map(|p| {
                if container_width > px(0.) {
                    container_width * p.opened_proportion
                } else {
                    px(p.opened_proportion * 200.)
                }
            })
            .unwrap_or(px(0.));

        let right_open_px = group_state
            .right_panel
            .as_ref()
            .map(|p| {
                if container_width > px(0.) {
                    container_width * p.opened_proportion
                } else {
                    px(p.opened_proportion * 200.)
                }
            })
            .unwrap_or(px(0.));

        // target_px is 0 when closed and open_px when open — drives the transition goal.
        let left_target_px = group_state
            .left_panel
            .as_ref()
            .map(|p| if p.open { left_open_px } else { px(0.) })
            .unwrap_or(px(0.));

        let right_target_px = group_state
            .right_panel
            .as_ref()
            .map(|p| if p.open { right_open_px } else { px(0.) })
            .unwrap_or(px(0.));

        let prev_width = group_state.prev_container_width;
        let is_first_frame = prev_width == px(0.) && container_width > px(0.);

        // On the first frame, seed opened_proportion from SidePanel::initial_proportion
        // so the builder method actually takes effect.
        if is_first_frame {
            state.update(cx, |s, _| {
                if let (Some(proportion), Some(panel)) =
                    (left_initial_proportion, s.left_panel.as_mut())
                {
                    panel.opened_proportion = proportion;
                }
                if let (Some(proportion), Some(panel)) =
                    (right_initial_proportion, s.right_panel.as_mut())
                {
                    panel.opened_proportion = proportion;
                }
            });
        }

        // Drive slide transitions for each side panel.
        let left_px = if self.left.is_some() {
            let left_target = left_target_px.as_f32();
            let transition = window
                .use_keyed_transition(
                    "left-panel-slide",
                    cx,
                    Duration::from_millis(SLIDE_DURATION_MS),
                    move |_, _| left_target,
                )
                .with_easing(ease_in_out);
            transition.update(cx, |goal, cx| {
                *goal = left_target;
                cx.notify();
            });
            px(*transition.evaluate(window, cx))
        } else {
            px(0.)
        };

        state.update(cx, |s, _| {
            s.animated_left_px = left_px;
        });

        let right_px = if self.right.is_some() {
            let right_target = right_target_px.as_f32();
            let transition = window
                .use_keyed_transition(
                    "right-panel-slide",
                    cx,
                    Duration::from_millis(SLIDE_DURATION_MS),
                    move |_, _| right_target,
                )
                .with_easing(ease_in_out);
            transition.update(cx, |goal, cx| {
                *goal = right_target;
                cx.notify();
            });
            px(*transition.evaluate(window, cx))
        } else {
            px(0.)
        };

        state.update(cx, |s, _| {
            s.animated_right_px = right_px;
            s.prev_container_width = container_width;
        });

        window.request_animation_frame();

        // // ── wire navigation bar toggle callbacks ─────────────────────────────

        // if let Some(nav) = self.center.navigation_bar.as_mut() {
        //     // Reflect current open state in the icons.
        //     if let Some(panel) = group_state.left_panel.as_ref() {
        //         nav.left_panel_open = Some(panel.open);
        //     }
        //     if let Some(panel) = group_state.right_panel.as_ref() {
        //         nav.right_panel_open = Some(panel.open);
        //     }

        //     if self.left.is_some() {
        //         let weak = weak_state.clone();
        //         nav.on_toggle_left = Some(Rc::new(move |_window, cx| {
        //             weak.update(cx, |state, cx| {
        //                 state.toggle_left();
        //                 cx.notify();
        //             })
        //             .ok();
        //         }));
        //     }

        //     if self.right.is_some() {
        //         let weak = weak_state.clone();
        //         nav.on_toggle_right = Some(Rc::new(move |_window, cx| {
        //             weak.update(cx, |state, cx| {
        //                 state.toggle_right();
        //                 cx.notify();
        //             })
        //             .ok();
        //         }));
        //     }
        // }

        // ── build layout ─────────────────────────────────────────────────────

        let left_visible = left_px > px(0.);
        let right_visible = right_px > px(0.);

        // Collect both handles separately so they can be added after their respective panels
        // — later children paint on top, guaranteeing they're never covered by the center.
        let left_handle = if self.left.is_some() && left_visible {
            let weak = weak_state.clone();
            Some(
                resize_handle("left-panel-handle", gpui::Axis::Horizontal)
                    .left(left_px)
                    .on_drag((), move |_, _, _, cx| {
                        cx.stop_propagation();
                        weak.update(cx, |s, _| {
                            s.resizing = Some((ResizingSide::Left, s.bounds.size.width, s.bounds));
                        })
                        .ok();
                        cx.new(|_| DragHandle)
                    }),
            )
        } else {
            None
        };

        let right_handle = if self.right.is_some() && right_visible {
            let weak = weak_state.clone();
            Some(
                resize_handle("right-panel-handle", gpui::Axis::Horizontal)
                    .right(right_px)
                    .on_drag((), move |_, _, _, cx| {
                        cx.stop_propagation();
                        weak.update(cx, |s, _| {
                            s.resizing = Some((ResizingSide::Right, s.bounds.size.width, s.bounds));
                        })
                        .ok();
                        cx.new(|_| DragHandle)
                    }),
            )
        } else {
            None
        };

        h_flex()
            .refine_style(&self.style)
            // Track container bounds each frame so resize math has the correct origin.
            .on_prepaint({
                let state = state.clone();
                move |bounds, _, cx| {
                    state.update(cx, |s, cx| {
                        if s.bounds != bounds {
                            s.bounds = bounds;
                            cx.notify();
                        }
                    });
                }
            })
            // Left side panel
            .when_some(self.left, |this, left| {
                // The open width is the fixed width the content always occupies.
                // The outer wrapper clips to the animated width, and the inner content
                // is anchored to the right (center-facing) edge so it slides in from
                // the left rather than squishing.
                let open_px = left_open_px;

                let inner = left
                    .base
                    .absolute()
                    .right_0()
                    .top_0()
                    .h_full()
                    .w(open_px)
                    .into_any_element();

                this.child(
                    div()
                        .relative()
                        .flex_shrink_0()
                        .h_full()
                        .overflow_hidden()
                        .w(left_px)
                        .child(inner),
                )
            })
            // Center panel — flex_1 so it takes all remaining space.
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .child(self.center),
            )
            // Left handle added after center so it paints on top of it.
            .when_some(left_handle, |this, handle| this.child(handle))
            // Right side panel
            .when_some(self.right, |this, right| {
                // The open width is the fixed width the content always occupies.
                // The outer wrapper clips to the animated width, and the inner content
                // is anchored to the left (center-facing) edge so it slides in from
                // the right rather than squishing.
                let open_px = right_open_px;

                let inner = right
                    .base
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(open_px)
                    .into_any_element();

                this.child(
                    div()
                        .relative()
                        .flex_shrink_0()
                        .h_full()
                        .overflow_hidden()
                        .w(right_px)
                        .child(inner),
                )
            })
            // Right handle added after the right panel so it paints on top of the center edge.
            .when_some(right_handle, |this, handle| this.child(handle))
            // The invisible element that handles global mouse-move / mouse-up during a drag.
            .child(PanelGroupDragElement {
                state: state.clone(),
            })
    }
}

// ── Drag-handle sentinel view ─────────────────────────────────────────────────

/// Invisible sentinel used as the drag view so GPUI recognises a drag is in progress.
struct DragHandle;
impl Render for DragHandle {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

// ── PanelGroupDragElement — global mouse listener ────────────────────────────

struct PanelGroupDragElement {
    state: Entity<PanelGroupState>,
}

impl IntoElement for PanelGroupDragElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for PanelGroupDragElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
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
    ) -> (gpui::LayoutId, ()) {
        (window.request_layout(Style::default(), None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut Window,
        _: &mut App,
    ) -> () {
        ()
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.on_mouse_event({
            let state = self.state.clone();
            let resizing = state.read(cx).resizing;
            move |e: &MouseMoveEvent, phase, _window, cx| {
                if !phase.bubble() {
                    return;
                }
                let Some((side, _, _)) = resizing else { return };
                state.update(cx, |s, cx| {
                    match side {
                        ResizingSide::Left => s.resize_left(e.position.x),
                        ResizingSide::Right => s.resize_right(e.position.x),
                    }
                    cx.notify();
                });
            }
        });

        window.on_mouse_event({
            let state = self.state.clone();
            let resizing = state.read(cx).resizing;
            move |_: &MouseUpEvent, phase, _window, cx| {
                if resizing.is_none() {
                    return;
                }
                if phase.bubble() {
                    state.update(cx, |s, cx| {
                        s.resizing = None;
                        cx.notify();
                    });
                }
            }
        });
    }
}
