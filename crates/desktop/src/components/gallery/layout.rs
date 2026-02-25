use std::{cmp, ops::Range};

use gpui::{
    Along, AnyElement, App, AvailableSpace, Axis, Bounds, ContentMask, Context, Div, Element,
    ElementId, Entity, GlobalElementId, Hitbox, InteractiveElement, IntoElement, ParentElement,
    Pixels, Point, Render, Size, Stateful, StyleRefinement, Styled, Window, div, point, px, size,
};
use gpui_component::AxisExt;
use smallvec::SmallVec;

use crate::components::gallery::GalleryTransitions;

#[allow(unused)]
pub fn v_gallery<R, V>(
    view: Entity<V>,
    id: impl Into<ElementId>,
    items_count: usize,
    item_size: Size<Pixels>,
    f: impl 'static + Fn(&mut V, Range<usize>, &mut Window, &mut Context<V>) -> Vec<R>,
) -> GalleryLayout
where
    R: IntoElement,
    V: Render,
{
    gallery(view, id, Axis::Vertical, items_count, item_size, f)
}

#[allow(unused)]
pub fn h_gallery<R, V>(
    view: Entity<V>,
    id: impl Into<ElementId>,
    items_count: usize,
    item_size: Size<Pixels>,
    f: impl 'static + Fn(&mut V, Range<usize>, &mut Window, &mut Context<V>) -> Vec<R>,
) -> GalleryLayout
where
    R: IntoElement,
    V: Render,
{
    gallery(view, id, Axis::Horizontal, items_count, item_size, f)
}

pub fn gallery<R, V>(
    view: Entity<V>,
    id: impl Into<ElementId>,
    axis: Axis,
    items_count: usize,
    item_size: Size<Pixels>,
    f: impl 'static + Fn(&mut V, Range<usize>, &mut Window, &mut Context<V>) -> Vec<R>,
) -> GalleryLayout
where
    R: IntoElement,
    V: Render,
{
    let id: ElementId = id.into();
    let render_range = move |visible_range, window: &mut Window, cx: &mut App| {
        view.update(cx, |this, cx| {
            f(this, visible_range, window, cx)
                .into_iter()
                .map(|component| component.into_any_element())
                .collect()
        })
    };

    let base = div().id(id.clone()).size_full();

    GalleryLayout {
        id: id.clone(),
        axis,
        base,
        items_count,
        item_size,
        focused_ix: 0,
        content_center: None,
        gap: None,
        render_items: Box::new(render_range),
        // item_transitions: Vec::new(),
        // slide_transition: None,
        transitions: None,
    }
}

pub struct GalleryLayout {
    id: ElementId,
    axis: Axis,
    base: Stateful<Div>,
    items_count: usize,
    item_size: Size<Pixels>,
    focused_ix: usize,
    content_center: Option<Point<Pixels>>,
    gap: Option<Pixels>,
    render_items: Box<
        dyn for<'a> Fn(Range<usize>, &'a mut Window, &'a mut App) -> SmallVec<[AnyElement; 64]>,
    >,
    // item_transitions: Vec<(usize, Transition<f32>)>,
    // slide_transition: Option<Transition<f32>>,
    transitions: Option<GalleryTransitions>,
}

impl Styled for GalleryLayout {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

// impl Default for GalleryLayout {
//     fn default() -> Self {
//         Self {
//             axis: Axis::Horizontal,
//             item_spacing: px(10.0),
//             max_item_size: size(px(250.0), px(500.0)),
//             focused_item_scale: 1.5,
//         }
//     }
// }

#[allow(unused)]
impl GalleryLayout {
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    pub fn focused_index(mut self, ix: usize) -> Self {
        self.focused_ix = ix;
        self
    }

    pub fn transitions(mut self, transitions: GalleryTransitions) -> Self {
        self.transitions = Some(transitions);
        self
    }

    // pub fn slide_transition(mut self, transition: Transition<f32>) -> Self {
    //     self.slide_transition = Some(transition);
    //     self
    // }

    // pub fn item_transitions(mut self, transitions: Vec<(usize, Transition<f32>)>) -> Self {
    //     self.item_transitions = transitions;
    //     self
    // }

    // pub fn item_spacing(mut self, spacing: Pixels) -> Self {
    //     self.item_spacing = spacing;
    //     self
    // }

    // pub fn max_item_size(mut self, size: Size<Pixels>) -> Self {
    //     self.max_item_size = size;
    //     self
    // }

    // pub fn focused_item_scale(mut self, scale: f32) -> Self {
    //     self.focused_item_scale = scale;
    //     self
    // }

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

    // fn calculate_item_position(
    //     &self,
    //     index: usize,
    //     gap: Pixels,
    //     num_visible: usize,
    // ) -> Point<Pixels> {
    //     let gallery_pos = index as i32 - (num_visible as i32 / 2);
    //     calculate_position(
    //         self.axis,
    //         self.content_center.unwrap_or_default(),
    //         self.item_size,
    //         self.gap.unwrap_or_default(),
    //         gallery_pos,
    //         // last_visible_element_ix - first_visible_element_ix,
    //     )
    // }
}

/// Frame state used by the [VirtualList].
pub struct GalleryLayoutFrameState {
    /// Visible items to be painted.
    items: SmallVec<[AnyElement; 32]>,
    content_size: Size<Pixels>,
    // item_size_with_gap: Pixels,
}

impl IntoElement for GalleryLayout {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl InteractiveElement for GalleryLayout {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.base.interactivity()
    }
}

impl Element for GalleryLayout {
    type RequestLayoutState = GalleryLayoutFrameState;
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
        // let longest_item_size = self.measure_item(None, window, cx);

        let layout_id = self.base.interactivity().request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| {
                window.with_text_style(style.text_style().cloned(), |window| {
                    window.request_layout(style, None, cx)
                })
            },
        );

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
            GalleryLayoutFrameState {
                items: SmallVec::new(),
                content_size,
                // item_size_with_gap,
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
        let rem_size = window.rem_size();
        let font_size = window.text_style().font_size.to_pixels(rem_size);
        let border_widths = style.border_widths.to_pixels(rem_size);
        let bounds_size = bounds.size;
        let paddings = style.padding.to_pixels(bounds_size.into(), rem_size);
        let axis = self.axis;
        let gap = style.gap.along(axis).to_pixels(font_size.into(), rem_size);
        self.gap = Some(gap);

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

        // Extract values needed in closure to avoid borrowing issues
        let items_count = self.items_count;
        let item_size = self.item_size;
        let render_items = &self.render_items;
        // let item_transitions = &self.item_transitions;

        self.base.interactivity().prepaint(
            global_id,
            inspector_id,
            bounds,
            layout.content_size,
            window,
            cx,
            |_style, _, hitbox, window, cx| {
                let num_visible = 3;
                let num_rendered = num_visible + 2;
                let num_before = num_rendered / 2;
                let num_after = num_rendered - num_before;

                let focused_ix = self.focused_ix;
                let start_ix = focused_ix.saturating_sub(num_before);
                let end_ix = cmp::min(focused_ix + num_after, items_count);
                let visible_range = start_ix..end_ix;

                // let visible_range = 0..cmp::min(3, items_count);
                // let num_visible = visible_range.len();
                let items = render_items(visible_range.clone(), window, cx);

                let content_mask = ContentMask { bounds };

                // println!("item_size: {:?}, gap: {:?}", item_size, gap);

                let center_pos = content_bounds.center();
                let center_item_pos =
                    center_pos - point(item_size.width / 2., item_size.height / 2.);
                self.content_center = Some(center_pos);
                let _ = window.with_content_mask(Some(content_mask), |window| {
                    for (mut item, ix) in items.into_iter().zip(visible_range.clone()) {
                        let interval = match axis {
                            Axis::Horizontal => item_size.width + gap,
                            Axis::Vertical => item_size.height + gap,
                        };
                        if let Some(transitions) = self.transitions.as_ref() {
                            let local_offset = *transitions.slide.evaluate(window, cx);
                            let relative_pos = ix as f32 - focused_ix as f32 + local_offset;

                            // position relative to focused item in pixels
                            let relative_pos_px = match axis {
                                Axis::Horizontal => point(interval * relative_pos, px(0.0)),
                                Axis::Vertical => point(px(0.0), interval * relative_pos),
                            };

                            let item_pos = center_item_pos + relative_pos_px;

                            // let fade_first_opacity = *transitions.fade_first.evaluate(window, cx);
                            // let fade_last_opacity = *transitions.fade_last.evaluate(window, cx);

                            let opacity =
                                ((num_rendered / 2) as f32 - relative_pos.abs()).clamp(0., 1.);

                            item = div()
                                .size_full()
                                .opacity(opacity)
                                .child(item)
                                .into_any_element();

                            item.layout_as_root(item_size.into(), window, cx);
                            item.prepaint_at(item_pos, window, cx);
                            layout.items.push(item);
                        }
                    }
                });

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

// pub fn calculate_position(
//     axis: Axis,
//     center: Point<Pixels>,
//     item_size: Size<Pixels>,
//     gap: Pixels,
//     gallery_pos: i32,
//     // num_visible: usize,
// ) -> Point<Pixels> {
//     // let base_position = item_size_with_gap * ix as f32;
//     // match axis {
//     //     Axis::Horizontal => point(base_position, px(0.0)),
//     //     Axis::Vertical => point(px(0.0), base_position),
//     // }

//     let center_item_pos = center - point(item_size.width / 2., item_size.height / 2.);

//     // let interval = match axis {
//     //     Axis::Horizontal => item_size.width + gap,
//     //     Axis::Vertical => item_size.height + gap,
//     // };

//     let interval = px(0.0);

//     match axis {
//         Axis::Horizontal => point(
//             center_item_pos.x + interval * gallery_pos as f32,
//             center_item_pos.y,
//         ),
//         Axis::Vertical => point(
//             center_item_pos.x,
//             center_item_pos.y + interval * gallery_pos as f32,
//         ),
//     }
// }
