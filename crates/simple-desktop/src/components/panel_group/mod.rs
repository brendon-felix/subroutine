use std::{ops::Range, rc::Rc, time::Duration};

use gpui::{
    AnyElement, App, AppContext as _, Bounds, Context, Element, ElementId, Empty, Entity,
    InteractiveElement, IntoElement, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render,
    RenderOnce, StatefulInteractiveElement, Style, StyleRefinement, Styled, Window, canvas, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{
    IconName, StyledExt,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use gpui_transitions::WindowUseTransition;
use smallvec::SmallVec;

use crate::transitions::ease_in_out;

mod resize_handle;
use resize_handle::resize_handle;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Extension trait that adds `on_prepaint` to any `ParentElement`.
trait ElementExt: ParentElement + Sized {
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

// ── constants ─────────────────────────────────────────────────────────────────

const SLIDE_DURATION_MS: u64 = 100;

// ── NavigationBar ─────────────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct NavigationBar {
    base: gpui::Div,
    // style: StyleRefinement,
    left_panel_open: Option<bool>,
    right_panel_open: Option<bool>,
    on_toggle_left: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_toggle_right: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    /// Extra left padding to add when the left panel is closed, to avoid the
    /// toggle button being obscured by the macOS traffic light controls.
    /// Interpolated smoothly via the panel open/close transition.
    traffic_light_padding: Pixels,
    children: SmallVec<[AnyElement; 8]>,
}

impl NavigationBar {
    pub fn new() -> Self {
        Self {
            base: h_flex(),
            // style: StyleRefinement::default(),
            left_panel_open: None,
            right_panel_open: None,
            on_toggle_left: None,
            on_toggle_right: None,
            traffic_light_padding: px(0.),
            children: SmallVec::new(),
        }
    }

    /// Sets the animated extra left padding for the toggle button to clear the
    /// macOS traffic light controls when the left panel is closed.
    pub fn traffic_light_padding(mut self, padding: Pixels) -> Self {
        self.traffic_light_padding = padding;
        self
    }

    pub fn left_panel_open(mut self, open: bool) -> Self {
        self.left_panel_open = Some(open);
        self
    }

    pub fn right_panel_open(mut self, open: bool) -> Self {
        self.right_panel_open = Some(open);
        self
    }

    pub fn on_toggle_left(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_left = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_right(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_right = Some(Rc::new(f));
        self
    }
}

impl Styled for NavigationBar {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for NavigationBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for NavigationBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_toggle_left = self.on_toggle_left;
        let on_toggle_right = self.on_toggle_right;
        let traffic_light_padding = self.traffic_light_padding;

        self.base
            .items_center()
            // .justify_between()
            .when_some(self.left_panel_open, |this, open| {
                let on_toggle = on_toggle_left.clone();
                this.child(
                    div().pl(traffic_light_padding).child(
                        Button::new("left-panel-button")
                            .size_6()
                            .ghost()
                            .when_else(
                                open,
                                |btn| btn.icon(IconName::PanelLeftClose),
                                |btn| btn.icon(IconName::PanelLeftOpen),
                            )
                            .when_some(on_toggle, |btn, callback| {
                                btn.on_click(move |_, window, cx| callback(window, cx))
                            }),
                    ),
                )
            })
            .child(
                h_flex()
                    .h_full()
                    .items_center()
                    .flex_1()
                    .children(self.children),
            )
            .when_some(self.right_panel_open, |this, open| {
                let on_toggle = on_toggle_right.clone();
                this.child(
                    Button::new("right-panel-button")
                        .size_6()
                        .ghost()
                        .when_else(
                            open,
                            |btn| btn.icon(IconName::PanelRightClose),
                            |btn| btn.icon(IconName::PanelRightOpen),
                        )
                        .when_some(on_toggle, |btn, callback| {
                            btn.on_click(move |_, window, cx| callback(window, cx))
                        }),
                )
            })
    }
}

// ── CenterPanel ───────────────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct CenterPanel {
    base: gpui::Stateful<gpui::Div>,
    navigation_bar: Option<NavigationBar>,
    children: SmallVec<[AnyElement; 8]>,
}

impl CenterPanel {
    pub fn new() -> Self {
        Self {
            base: div().id("center-panel"),
            navigation_bar: None,
            children: SmallVec::new(),
        }
    }

    // pub fn navigation_bar(mut self) -> Self {
    //     self.navigation_bar = Some(NavigationBar::new());
    //     self
    // }
}

impl Styled for CenterPanel {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for CenterPanel {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for CenterPanel {}

impl ParentElement for CenterPanel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for CenterPanel {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.base.size_full().child(
            v_flex()
                .size_full()
                .when_some(self.navigation_bar, |this, nav| this.child(nav))
                .children(self.children),
        )
    }
}

// ── SidePanel ────────────────────────────────────────────────────────────────

#[derive(IntoElement)]
pub struct SidePanel {
    base: gpui::Stateful<gpui::Div>,
    width_range: Range<Pixels>,
    initial_proportion: f32,
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

    pub fn initial_proportion(mut self, width: f32) -> Self {
        self.initial_proportion = width;
        self
    }

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

// ── State ─────────────────────────────────────────────────────────────────────

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

#[derive(Clone)]
pub struct CenterPanelState {
    // pub min_proportion: f32,
    pub min_width: Pixels,
}

impl Default for CenterPanelState {
    fn default() -> Self {
        Self {
            // min_proportion: 0.2,
            min_width: px(100.),
        }
    }
}

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

    pub fn container_width(&self) -> Pixels {
        self.bounds.size.width
    }

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
        let right_proportion = self
            .right_panel
            .as_ref()
            .filter(|p| p.open)
            .map(|p| p.opened_proportion)
            .unwrap_or(0.0);
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
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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

        let is_resizing_left = matches!(group_state.resizing, Some((ResizingSide::Left, _, _)));
        let is_resizing_right = matches!(group_state.resizing, Some((ResizingSide::Right, _, _)));

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

        // On window resize: scale the transition's start/end goals proportionally so
        // the animation progress is preserved and no frame-coalescing can cause a
        // jump_to to fire on the same frame as a toggle action.
        // On the very first frame (prev=0): jump_to so the panel starts at the correct
        // position with no spurious animation from 0.
        // During a drag: jump_to so the handle tracks the cursor with no lag.
        let scale = if !is_first_frame && prev_width > px(0.) && container_width != prev_width {
            Some((container_width / prev_width).into())
        } else {
            None
        };

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
            if is_first_frame || is_resizing_left {
                transition.jump_to(left_target, cx);
            } else {
                if let Some(ratio) = scale {
                    transition.scale_by(ratio, cx);
                }
                transition.update(cx, |goal, cx| {
                    *goal = left_target;
                    cx.notify();
                });
            }
            px(*transition.evaluate(window, cx))
        } else {
            px(0.)
        };

        state.update(cx, |s, cx| {
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
            if is_first_frame || is_resizing_right {
                transition.jump_to(right_target, cx);
            } else {
                if let Some(ratio) = scale {
                    transition.scale_by(ratio, cx);
                }
                transition.update(cx, |goal, cx| {
                    *goal = right_target;
                    cx.notify();
                });
            }
            px(*transition.evaluate(window, cx))
        } else {
            px(0.)
        };

        state.update(cx, |s, _| {
            s.animated_right_px = right_px;
            s.prev_container_width = container_width;
        });

        window.request_animation_frame();

        // ── wire navigation bar toggle callbacks ─────────────────────────────

        if let Some(nav) = self.center.navigation_bar.as_mut() {
            // Reflect current open state in the icons.
            if let Some(panel) = group_state.left_panel.as_ref() {
                nav.left_panel_open = Some(panel.open);
            }
            if let Some(panel) = group_state.right_panel.as_ref() {
                nav.right_panel_open = Some(panel.open);
            }

            if self.left.is_some() {
                let weak = weak_state.clone();
                nav.on_toggle_left = Some(Rc::new(move |_window, cx| {
                    weak.update(cx, |state, cx| {
                        state.toggle_left();
                        cx.notify();
                    })
                    .ok();
                }));
            }

            if self.right.is_some() {
                let weak = weak_state.clone();
                nav.on_toggle_right = Some(Rc::new(move |_window, cx| {
                    weak.update(cx, |state, cx| {
                        state.toggle_right();
                        cx.notify();
                    })
                    .ok();
                }));
            }
        }

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
