//! Virtual List for render a large number of items with uniform size.
//!
//! > NOTE: This must ensure each column width or row height.
//!
//! Only visible range are rendered for performance reasons.
//!
//! Inspired by `gpui::uniform_list`.
//! https://github.com/zed-industries/zed/blob/0ae1603610ab6b265bdfbee7b8dbc23c5ab06edc/crates/gpui/src/elements/uniform_list.rs
//!
//! Unlike the `uniform_list`, this implementation supports both horizontal and vertical layouts.
use std::{
    cell::RefCell,
    cmp,
    ops::{Deref, Range},
    rc::Rc,
    time::Duration,
};

use gpui::{
    Along, AnyElement, App, AvailableSpace, Axis, Bounds, ContentMask, Context,
    DeferredScrollToItem, Div, Element, ElementId, Entity, GlobalElementId, Half, Hitbox,
    InteractiveElement, IntoElement, IsZero as _, ListSizingBehavior, Pixels, Point, Render,
    ScrollHandle, ScrollStrategy, Size, Stateful, StatefulInteractiveElement, StyleRefinement,
    Styled, Window, div, point, px, size,
};
use gpui_component::{AxisExt, scroll::ScrollbarHandle};
use gpui_transitions::{Transition, WindowUseTransition};
use smallvec::SmallVec;

use crate::transitions;

struct ScrollAnimation {
    id: ElementId,
    from: Point<Pixels>,
    duration: Duration,
}

impl ScrollAnimation {
    fn new(id: ElementId, from: Point<Pixels>, duration: Duration) -> Self {
        Self { id, from, duration }
    }

    /// Can only be used during prepaint, paint, or render methonds.
    fn transition(&self, window: &mut Window, cx: &mut App) -> Transition<Point<Pixels>> {
        window
            .use_keyed_transition(self.id.clone(), cx, self.duration, |_window, _cx| self.from)
            .with_easing(transitions::ease_out_cubic)
    }
}

struct VirtualListScrollHandleState {
    axis: Axis,
    items_count: usize,
    pub deferred_scroll_to_item: Option<DeferredScrollToItem>,
    pub active_animation: Option<ScrollAnimation>,
}

/// A scroll handle for [`VirtualList`].
///
/// See also [`ScrollHandle`].
#[derive(Clone)]
pub struct VirtualListScrollHandle {
    state: Rc<RefCell<VirtualListScrollHandleState>>,
    base_handle: ScrollHandle,
}

impl From<ScrollHandle> for VirtualListScrollHandle {
    fn from(handle: ScrollHandle) -> Self {
        let mut this = VirtualListScrollHandle::new();
        this.base_handle = handle;
        this
    }
}

impl AsRef<ScrollHandle> for VirtualListScrollHandle {
    fn as_ref(&self) -> &ScrollHandle {
        &self.base_handle
    }
}

impl ScrollbarHandle for VirtualListScrollHandle {
    fn offset(&self) -> Point<Pixels> {
        self.base_handle.offset()
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        self.base_handle.set_offset(offset);
    }

    fn content_size(&self) -> Size<Pixels> {
        self.base_handle.content_size()
    }
}

impl Deref for VirtualListScrollHandle {
    type Target = ScrollHandle;

    fn deref(&self) -> &Self::Target {
        &self.base_handle
    }
}

#[allow(dead_code)]
impl VirtualListScrollHandle {
    /// Create a new VirtualListScrollHandle.
    pub fn new() -> Self {
        VirtualListScrollHandle {
            state: Rc::new(RefCell::new(VirtualListScrollHandleState {
                axis: Axis::Vertical,
                items_count: 0,
                deferred_scroll_to_item: None,
                active_animation: None,
            })),
            base_handle: ScrollHandle::default(),
        }
    }

    /// Get the base scroll handle.
    pub fn base_handle(&self) -> &ScrollHandle {
        &self.base_handle
    }

    /// Scroll to the item at the given index.
    pub fn scroll_to_item(&self, ix: usize, strategy: ScrollStrategy) {
        self.scroll_to_item_with_offset(ix, strategy, 0);
    }

    /// Scroll to the item at the given index, with an additional offset items.
    fn scroll_to_item_with_offset(&self, ix: usize, strategy: ScrollStrategy, offset: usize) {
        let mut state = self.state.borrow_mut();
        state.deferred_scroll_to_item = Some(DeferredScrollToItem {
            item_index: ix,
            strategy,
            offset,
            scroll_strict: false,
        });
    }

    /// Scrolls to the bottom of the list.
    pub fn scroll_to_bottom(&self) {
        let items_count = self.state.borrow().items_count;
        self.scroll_to_item(items_count.saturating_sub(1), ScrollStrategy::Top);
    }
}

/// Create a [`VirtualList`] in vertical direction.
///
/// This is like `uniform_list` in GPUI, but support two axis.
///
/// The `item_size` is the size of each item,
/// only the `height` is used for vertical lists, `width` is ignored and VirtualList will measure the first item width.
///
/// See also [`h_virtual_list`]
#[inline]
pub fn v_virtual_list<R, V>(
    view: Entity<V>,
    id: impl Into<ElementId>,
    items_count: usize,
    item_size: Size<Pixels>,
    f: impl 'static + Fn(&mut V, Range<usize>, &mut Window, &mut Context<V>) -> Vec<R>,
) -> VirtualList
where
    R: IntoElement,
    V: Render,
{
    virtual_list(view, id, Axis::Vertical, items_count, item_size, f)
}

/// Create a [`VirtualList`] in horizontal direction.
///
/// The `item_size` is the size of each item,
/// only the `width` is used for horizontal lists, `height` is ignored and VirtualList will measure the first item height.
///
/// See also [`v_virtual_list`]
#[inline]
#[allow(dead_code)]
pub fn h_virtual_list<R, V>(
    view: Entity<V>,
    id: impl Into<ElementId>,
    items_count: usize,
    item_size: Size<Pixels>,
    f: impl 'static + Fn(&mut V, Range<usize>, &mut Window, &mut Context<V>) -> Vec<R>,
) -> VirtualList
where
    R: IntoElement,
    V: Render,
{
    virtual_list(view, id, Axis::Horizontal, items_count, item_size, f)
}

pub(crate) fn virtual_list<R, V>(
    view: Entity<V>,
    id: impl Into<ElementId>,
    axis: Axis,
    items_count: usize,
    item_size: Size<Pixels>,
    f: impl 'static + Fn(&mut V, Range<usize>, &mut Window, &mut Context<V>) -> Vec<R>,
) -> VirtualList
where
    R: IntoElement,
    V: Render,
{
    let id: ElementId = id.into();
    let scroll_handle = VirtualListScrollHandle::new();
    let render_range = move |visible_range, window: &mut Window, cx: &mut App| {
        view.update(cx, |this, cx| {
            f(this, visible_range, window, cx)
                .into_iter()
                .map(|component| component.into_any_element())
                .collect()
        })
    };

    VirtualList {
        id: id.clone(),
        axis,
        base: div()
            .id(id)
            .size_full()
            .overflow_scroll()
            .track_scroll(&scroll_handle),
        scroll_handle,
        items_count,
        item_size,
        render_items: Box::new(render_range),
        sizing_behavior: ListSizingBehavior::default(),
    }
}

/// VirtualList component for rendering a large number of uniformly sized items.
pub struct VirtualList {
    id: ElementId,
    axis: Axis,
    base: Stateful<Div>,
    scroll_handle: VirtualListScrollHandle,
    items_count: usize,
    item_size: Size<Pixels>,
    render_items: Box<
        dyn for<'a> Fn(Range<usize>, &'a mut Window, &'a mut App) -> SmallVec<[AnyElement; 64]>,
    >,
    sizing_behavior: ListSizingBehavior,
}

impl Styled for VirtualList {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

#[allow(dead_code)]
impl VirtualList {
    pub fn track_scroll(mut self, scroll_handle: &VirtualListScrollHandle) -> Self {
        self.base = self.base.track_scroll(&scroll_handle);
        self.scroll_handle = scroll_handle.clone();
        self
    }

    /// Set the sizing behavior for the list.
    pub fn with_sizing_behavior(mut self, behavior: ListSizingBehavior) -> Self {
        self.sizing_behavior = behavior;
        self
    }

    /// Specify for table.
    ///
    /// Table is special, because the `scroll_handle` is based on Table head (That is not a virtual list).
    // pub(crate) fn with_scroll_handle(mut self, scroll_handle: &VirtualListScrollHandle) -> Self {
    //     self.base = div().id(self.id.clone()).size_full();
    //     self.scroll_handle = scroll_handle.clone();
    //     self
    // }

    fn scroll_to_deferred_item(
        &self,
        scroll_offset: Point<Pixels>,
        item_size_with_gap: Pixels,
        content_bounds: &Bounds<Pixels>,
        scroll_to_item: DeferredScrollToItem,
    ) -> Point<Pixels> {
        let item_index = scroll_to_item.item_index + scroll_to_item.offset;
        if item_index >= self.items_count {
            return scroll_offset;
        }

        let item_position = item_size_with_gap * item_index as f32;
        let item_bounds = match self.axis {
            Axis::Vertical => Bounds::new(
                point(px(0.), content_bounds.top() + item_position),
                size(content_bounds.size.width, self.item_size.height),
            ),
            Axis::Horizontal => Bounds::new(
                point(content_bounds.left() + item_position, px(0.)),
                size(self.item_size.width, content_bounds.size.height),
            ),
        };

        let mut scroll_offset = scroll_offset;
        match scroll_to_item.strategy {
            ScrollStrategy::Center => {
                if self.axis.is_vertical() {
                    scroll_offset.y = content_bounds.top() + content_bounds.size.height.half()
                        - item_bounds.top()
                        - item_bounds.size.height.half()
                } else {
                    scroll_offset.x = content_bounds.left() + content_bounds.size.width.half()
                        - item_bounds.left()
                        - item_bounds.size.width.half()
                }
            }
            _ => {
                if self.axis.is_vertical() {
                    if item_bounds.top() + scroll_offset.y < content_bounds.top() {
                        scroll_offset.y = content_bounds.top() - item_bounds.top()
                    } else if item_bounds.bottom() + scroll_offset.y > content_bounds.bottom() {
                        scroll_offset.y = content_bounds.bottom() - item_bounds.bottom();
                    }
                } else {
                    if item_bounds.left() + scroll_offset.x < content_bounds.left() {
                        scroll_offset.x = content_bounds.left() - item_bounds.left();
                    } else if item_bounds.right() + scroll_offset.x > content_bounds.right() {
                        scroll_offset.x = content_bounds.right() - item_bounds.right();
                    }
                }
            }
        }
        self.scroll_handle.set_offset(scroll_offset);
        scroll_offset
    }

    /// Ref from: https://github.com/zed-industries/zed/blob/83f9f9d9e3f5914392cab9a09e3472711a1d7b38/crates/gpui/src/elements/uniform_list.rs#L660
    fn measure_item(
        &self,
        list_width: Option<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Size<Pixels> {
        if self.items_count == 0 {
            return Size::default();
        }

        let item_ix = 0;
        let mut items = (self.render_items)(item_ix..item_ix + 1, window, cx);
        let Some(mut item_to_measure) = items.pop() else {
            return Size::default();
        };
        let available_space = size(
            list_width.map_or(AvailableSpace::MinContent, |width| {
                AvailableSpace::Definite(width)
            }),
            AvailableSpace::MinContent,
        );
        item_to_measure.layout_as_root(available_space, window, cx)
    }
}

/// Frame state used by the [VirtualList].
pub struct VirtualListFrameState {
    /// Visible items to be painted.
    items: SmallVec<[AnyElement; 32]>,
    content_size: Size<Pixels>,
    item_size_with_gap: Pixels,
}

impl IntoElement for VirtualList {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for VirtualList {
    type RequestLayoutState = VirtualListFrameState;
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let rem_size = window.rem_size();
        let font_size = window.text_style().font_size.to_pixels(rem_size);
        let longest_item_size = self.measure_item(None, window, cx);

        let layout_id = self.base.interactivity().request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| {
                // Calculate gap between items
                let gap = style
                    .gap
                    .along(self.axis)
                    .to_pixels(font_size.into(), rem_size);

                // Calculate item size including gap (except for last item)
                let item_size_with_gap = self.item_size.along(self.axis) + gap;

                // Calculate total content size
                let content_size = if self.axis.is_horizontal() {
                    Size {
                        width: if self.items_count > 0 {
                            item_size_with_gap * self.items_count.saturating_sub(1) as f32
                                + self.item_size.width
                        } else {
                            px(0.)
                        },
                        height: longest_item_size.height,
                    }
                } else {
                    Size {
                        width: longest_item_size.width,
                        height: if self.items_count > 0 {
                            item_size_with_gap * self.items_count.saturating_sub(1) as f32
                                + self.item_size.height
                        } else {
                            px(0.)
                        },
                    }
                };

                let axis = self.axis;
                let layout_id = match self.sizing_behavior {
                    ListSizingBehavior::Infer => {
                        window.with_text_style(style.text_style().cloned(), |window| {
                            let content_size = content_size.clone();

                            window.request_measured_layout(style, {
                                move |known_dimensions, available_space, _, _| {
                                    let mut size = Size::default();
                                    if axis.is_horizontal() {
                                        size.width =
                                            known_dimensions.width.unwrap_or(match available_space
                                                .width
                                            {
                                                AvailableSpace::Definite(x) => x,
                                                AvailableSpace::MinContent
                                                | AvailableSpace::MaxContent => content_size.width,
                                            });
                                        size.height = known_dimensions.height.unwrap_or(
                                            match available_space.height {
                                                AvailableSpace::Definite(x) => x,
                                                AvailableSpace::MinContent
                                                | AvailableSpace::MaxContent => content_size.height,
                                            },
                                        );
                                    } else {
                                        size.width =
                                            known_dimensions.width.unwrap_or(match available_space
                                                .width
                                            {
                                                AvailableSpace::Definite(x) => x,
                                                AvailableSpace::MinContent
                                                | AvailableSpace::MaxContent => content_size.width,
                                            });
                                        size.height = known_dimensions.height.unwrap_or(
                                            match available_space.height {
                                                AvailableSpace::Definite(x) => x,
                                                AvailableSpace::MinContent
                                                | AvailableSpace::MaxContent => content_size.height,
                                            },
                                        );
                                    }

                                    size
                                }
                            })
                        })
                    }
                    ListSizingBehavior::Auto => window
                        .with_text_style(style.text_style().cloned(), |window| {
                            window.request_layout(style, None, cx)
                        }),
                };

                layout_id
            },
        );

        let rem_size = window.rem_size();
        let font_size = window.text_style().font_size.to_pixels(rem_size);
        let style = self
            .base
            .interactivity()
            .compute_style(global_id, None, window, cx);
        let gap = style
            .gap
            .along(self.axis)
            .to_pixels(font_size.into(), rem_size);
        let item_size_with_gap = self.item_size.along(self.axis) + gap;
        let longest_item_size = self.measure_item(None, window, cx);

        let content_size = if self.axis.is_horizontal() {
            Size {
                width: if self.items_count > 0 {
                    item_size_with_gap * self.items_count.saturating_sub(1) as f32
                        + self.item_size.width
                } else {
                    px(0.)
                },
                height: longest_item_size.height,
            }
        } else {
            Size {
                width: longest_item_size.width,
                height: if self.items_count > 0 {
                    item_size_with_gap * self.items_count.saturating_sub(1) as f32
                        + self.item_size.height
                } else {
                    px(0.)
                },
            }
        };

        (
            layout_id,
            VirtualListFrameState {
                items: SmallVec::new(),
                content_size,
                item_size_with_gap,
            },
        )
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let style = self
            .base
            .interactivity()
            .compute_style(global_id, None, window, cx);
        let border_widths = style.border_widths.to_pixels(window.rem_size());
        let paddings = style
            .padding
            .to_pixels(bounds.size.into(), window.rem_size());

        let content_bounds = Bounds::from_corners(
            bounds.origin
                + point(
                    border_widths.left + paddings.left,
                    border_widths.top + paddings.top,
                ),
            bounds.bottom_right()
                - point(
                    border_widths.right + paddings.right,
                    border_widths.bottom + paddings.bottom,
                ),
        );

        let axis = self.axis;

        let mut scroll_state = self.scroll_handle.state.borrow_mut();
        scroll_state.axis = axis;
        scroll_state.items_count = self.items_count;

        let mut scroll_offset = self.scroll_handle.offset();
        if let Some(scroll_to_item) = scroll_state.deferred_scroll_to_item.take() {
            let final_offset = self.scroll_to_deferred_item(
                scroll_offset,
                layout.item_size_with_gap,
                &content_bounds,
                scroll_to_item,
            );
            let id = ElementId::NamedChild(
                Box::new(self.id.clone()),
                "virtual_list_scroll_animation".into(),
            );
            let duration = Duration::from_millis(400);
            let animation = ScrollAnimation::new(id, scroll_offset, duration);
            let transition = animation.transition(window, cx);
            transition.update(cx, |this, cx| {
                *this = final_offset;
                cx.notify();
            });
            scroll_state.active_animation = Some(animation);
        }

        if let Some(active_animation) = &scroll_state.active_animation {
            let transition = active_animation.transition(window, cx);
            scroll_offset = *transition.evaluate(window, cx);

            if transition.evaluate_delta(cx) >= 1.0 {
                scroll_state.active_animation = None;
            }
        }

        scroll_offset = scroll_offset
            .max(&point(
                content_bounds.size.width - layout.content_size.width,
                content_bounds.size.height - layout.content_size.height,
            ))
            .min(&point(px(0.), px(0.)));
        if scroll_offset != self.scroll_handle.offset() {
            self.scroll_handle.set_offset(scroll_offset);
        }

        self.base.interactivity().prepaint(
            global_id,
            inspector_id,
            bounds,
            layout.content_size,
            window,
            cx,
            |_style, _, hitbox, window, cx| {
                if self.items_count > 0 {
                    let min_scroll_offset =
                        content_bounds.size.along(self.axis) - layout.content_size.along(self.axis);

                    let is_scrolled = !scroll_offset.along(self.axis).is_zero();
                    if is_scrolled {
                        match self.axis {
                            Axis::Horizontal if scroll_offset.x < min_scroll_offset => {
                                scroll_offset.x = min_scroll_offset;
                                self.scroll_handle.set_offset(scroll_offset);
                            }
                            Axis::Vertical if scroll_offset.y < min_scroll_offset => {
                                scroll_offset.y = min_scroll_offset;
                                self.scroll_handle.set_offset(scroll_offset);
                            }
                            _ => {}
                        }
                    }

                    // Calculate visible range with uniform item sizes
                    let (first_visible_element_ix, last_visible_element_ix) = match self.axis {
                        Axis::Horizontal => {
                            let scroll_position = -(scroll_offset.x + paddings.left);
                            let first_visible_element_ix = if layout.item_size_with_gap > px(0.) {
                                (scroll_position / layout.item_size_with_gap)
                                    .floor()
                                    .max(0.)
                                    .min(self.items_count.saturating_sub(1) as f32)
                                    as usize
                            } else {
                                0
                            };

                            let visible_width = content_bounds.size.width - scroll_offset.x;
                            let last_visible_element_ix = if layout.item_size_with_gap > px(0.) {
                                let end_position = scroll_position + visible_width;
                                ((end_position / layout.item_size_with_gap).ceil() as usize + 1)
                                    .min(self.items_count)
                            } else {
                                self.items_count
                            };

                            (first_visible_element_ix, last_visible_element_ix)
                        }
                        Axis::Vertical => {
                            let scroll_position = -(scroll_offset.y + paddings.top);
                            let first_visible_element_ix = if layout.item_size_with_gap > px(0.) {
                                (scroll_position / layout.item_size_with_gap)
                                    .floor()
                                    .max(0.)
                                    .min(self.items_count.saturating_sub(1) as f32)
                                    as usize
                            } else {
                                0
                            };

                            let visible_height = content_bounds.size.height - scroll_offset.y;
                            let last_visible_element_ix = if layout.item_size_with_gap > px(0.) {
                                let end_position = scroll_position + visible_height;
                                ((end_position / layout.item_size_with_gap).ceil() as usize + 1)
                                    .min(self.items_count)
                            } else {
                                self.items_count
                            };

                            (first_visible_element_ix, last_visible_element_ix)
                        }
                    };

                    let visible_range = first_visible_element_ix
                        ..cmp::min(last_visible_element_ix, self.items_count);

                    let items = (self.render_items)(visible_range.clone(), window, cx);

                    let content_mask = ContentMask { bounds };
                    window.with_content_mask(Some(content_mask), |window| {
                        for (mut item, ix) in items.into_iter().zip(visible_range.clone()) {
                            let item_position = layout.item_size_with_gap * ix as f32;
                            let item_origin = match self.axis {
                                Axis::Horizontal => {
                                    content_bounds.origin
                                        + point(item_position + scroll_offset.x, scroll_offset.y)
                                }
                                Axis::Vertical => {
                                    content_bounds.origin
                                        + point(scroll_offset.x, item_position + scroll_offset.y)
                                }
                            };

                            let available_space = match self.axis {
                                Axis::Horizontal => size(
                                    AvailableSpace::Definite(self.item_size.width),
                                    AvailableSpace::Definite(content_bounds.size.height),
                                ),
                                Axis::Vertical => size(
                                    AvailableSpace::Definite(content_bounds.size.width),
                                    AvailableSpace::Definite(self.item_size.height),
                                ),
                            };

                            item.layout_as_root(available_space, window, cx);
                            item.prepaint_at(item_origin, window, cx);
                            layout.items.push(item);
                        }
                    });
                }

                hitbox
            },
        )
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.base.interactivity().paint(
            global_id,
            inspector_id,
            bounds,
            hitbox.as_ref(),
            window,
            cx,
            |_, window, cx| {
                for item in &mut layout.items {
                    item.paint(window, cx);
                }
            },
        )
    }
}
