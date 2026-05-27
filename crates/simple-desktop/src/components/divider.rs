use gpui::{
    App, Axis, Div, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, canvas, div, point, prelude::FluentBuilder as _,
    px,
};
use gpui_component::{ActiveTheme, StyledExt};

/// The style of the divider line.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum DividerStyle {
    #[default]
    Solid,
    Dashed,
}

/// A divider that can be either vertical or horizontal.
#[derive(IntoElement)]
pub struct Divider {
    base: Div,
    stroke: Pixels,
    style: StyleRefinement,
    label: Option<SharedString>,
    axis: Axis,
    color: Option<Hsla>,
    line_style: DividerStyle,
}

impl Divider {
    // /// Creates a vertical divider.
    // pub fn vertical() -> Self {
    //     Self {
    //         base: div().h_full(),
    //         stroke: px(1.0),
    //         axis: Axis::Vertical,
    //         label: None,
    //         color: None,
    //         style: StyleRefinement::default(),
    //         line_style: DividerStyle::Solid,
    //     }
    // }

    /// Creates a horizontal divider.
    pub fn horizontal() -> Self {
        Self {
            base: div(),
            stroke: px(1.0),
            axis: Axis::Horizontal,
            label: None,
            color: None,
            style: StyleRefinement::default(),
            line_style: DividerStyle::Solid,
        }
    }

    // /// Creates a vertical dashed divider.
    // pub fn vertical_dashed() -> Self {
    //     Self::vertical().dashed()
    // }

    // /// Creates a horizontal dashed divider.
    // pub fn horizontal_dashed() -> Self {
    //     Self::horizontal().dashed()
    // }

    /// Sets the thickness of the divider line.
    pub fn stroke(mut self, stroke: impl Into<Pixels>) -> Self {
        self.stroke = stroke.into();
        self
    }

    // /// Sets the label for the divider.
    // pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    //     self.label = Some(label.into());
    //     self
    // }

    /// Sets the color for the divider line.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Sets the style of the divider to dashed.
    pub fn dashed(mut self) -> Self {
        self.line_style = DividerStyle::Dashed;
        self
    }

    fn render_base(axis: Axis, stroke: Pixels) -> Div {
        div().absolute().map(|this| match axis {
            Axis::Vertical => this.w(stroke).h_full(),
            Axis::Horizontal => this.h(stroke).w_full(),
        })
    }

    fn render_solid(axis: Axis, color: Hsla, stroke: Pixels) -> impl IntoElement {
        Self::render_base(axis, stroke).bg(color)
    }

    fn render_dashed(axis: Axis, color: Hsla, stroke: Pixels) -> impl IntoElement {
        Self::render_base(axis, stroke).child(
            canvas(
                move |_, _, _| {},
                move |bounds, _, window, _| {
                    let mut builder = PathBuilder::stroke(stroke).dash_array(&[px(4.), px(2.)]);
                    let (start, end) = match axis {
                        Axis::Horizontal => {
                            let x = bounds.origin.x;
                            let y = bounds.origin.y + px(0.5);
                            (point(x, y), point(x + bounds.size.width, y))
                        }
                        Axis::Vertical => {
                            let x = bounds.origin.x + px(0.5);
                            let y = bounds.origin.y;
                            (point(x, y), point(x, y + bounds.size.height))
                        }
                    };
                    builder.move_to(start);
                    builder.line_to(end);
                    if let Ok(line) = builder.build() {
                        window.paint_path(line, color);
                    }
                },
            )
            .size_full(),
        )
    }
}

impl Styled for Divider {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Divider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.color.unwrap_or(cx.theme().border);
        let axis = self.axis;
        let line_style = self.line_style;
        let stroke = self.stroke;

        self.base
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .refine_style(&self.style)
            .child(match line_style {
                DividerStyle::Solid => Self::render_solid(axis, color, stroke).into_any_element(),
                DividerStyle::Dashed => Self::render_dashed(axis, color, stroke).into_any_element(),
            })
            .when_some(self.label, |this, label| {
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .mx_auto()
                        .text_xs()
                        .bg(cx.theme().background)
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                )
            })
    }
}
