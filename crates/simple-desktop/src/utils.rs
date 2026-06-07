use std::ops::{Mul, RangeInclusive};

use gpui::{
    App, Div, Hsla, InteractiveElement, Stateful, StatefulInteractiveElement, Styled,
    prelude::FluentBuilder, transparent_white,
};
use gpui_component::{ActiveTheme, Colorize};

// /// format hour using simplified US format (e.g. 1pm, 8am, noon, midnight)
// pub fn format_hour(hour: usize) -> String {
//     match hour {
//         0 => "midnight".to_string(),
//         12 => "noon".to_string(),
//         _ if hour < 13 => format!("{}am", hour),
//         _ => format!("{}pm", hour - 12),
//     }
// }

// pub trait AppAnimation {
//     fn duration(&self) -> Duration;
// }

// pub fn fraction(f: f32) -> DefiniteLength {
//     DefiniteLength::Fraction(f)
// }

#[derive(Clone, Copy)]
pub struct ButtonColors {
    pub bg: Hsla,
    pub hover: Hsla,
    pub active: Hsla,
    pub border: Option<Hsla>,
}

#[allow(unused)]
impl ButtonColors {
    pub fn normal(base_color: Hsla, cx: &App) -> Self {
        Self {
            bg: base_color.mix_oklab(cx.theme().transparent, 0.2),
            hover: base_color.mix_oklab(cx.theme().transparent, 0.3),
            active: base_color.mix_oklab(cx.theme().transparent, 0.4),
            // border: None,
            border: Some(base_color.mix_oklab(transparent_white(), 0.4)),
        }
    }

    pub fn outline(base_color: Hsla, cx: &App) -> Self {
        Self {
            bg: cx.theme().transparent,
            hover: base_color.mix_oklab(cx.theme().transparent, 0.2),
            active: base_color.mix_oklab(cx.theme().transparent, 0.3),
            border: Some(base_color.mix_oklab(transparent_white(), 0.4)),
        }
    }
}

pub trait ButtonColorizeExt {
    fn button_colors(self, colors: ButtonColors) -> Self;
}

impl ButtonColorizeExt for Stateful<Div> {
    fn button_colors(self, colors: ButtonColors) -> Self {
        self.bg(colors.bg)
            .hover(|s| s.bg(colors.hover))
            .active(|s| s.bg(colors.active))
            .when_some(colors.border, |this, color| {
                this.border_1().border_color(color)
            })
    }
}

#[derive(Clone)]
pub struct ZoomState<T: Copy + Mul<f32, Output = T>> {
    pub base_value: T,
    pub zoom: f32,
    pub zoom_factor: f32,
    pub range: RangeInclusive<f32>,
}

#[allow(unused)]
impl<T: Copy + Mul<f32, Output = T>> ZoomState<T> {
    pub fn new(base_value: T) -> Self {
        let zoom = 1.0;
        let zoom_factor = 2.0;
        let range = 1.0 / 8.0..=8.0;
        Self {
            base_value,
            zoom,
            zoom_factor,
            range,
        }
    }

    /// Sets the initial zoom level, which is the multiplier applied to the base value to get the current zoomed value. The default initial zoom level is 1.0, which means no zoom.
    pub fn with_initial_zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }

    /// Sets the zoom factor, which determines how much the value changes when zooming in or out. Default is 2.0, meaning each zoom step doubles or halves the value.
    pub fn with_zoom_factor(mut self, zoom_factor: f32) -> Self {
        self.zoom_factor = zoom_factor;
        self
    }

    /// Sets the zoom range, which limits how much the value can be zoomed in or out.
    pub fn with_range(mut self, range: RangeInclusive<f32>) -> Self {
        self.range = range;
        self
    }

    /// Returns the current zoomed value, which is the base value multiplied by the current zoom level.
    pub fn current_value(&self) -> T {
        self.base_value * self.zoom
    }

    /// Zooms in by multiplying the current zoom level by the zoom factor, if it does not exceed the maximum zoom level defined in the range. Returns true if the zoom level was successfully updated, or false if it is already at or above the maximum zoom level.
    pub fn zoom_in(&mut self) -> bool {
        if self.zoom >= *self.range.end() {
            false
        } else {
            self.zoom *= self.zoom_factor;
            true
        }
    }

    /// Zooms out by dividing the current zoom level by the zoom factor, if it does not go below the minimum zoom level defined in the range. Returns true if the zoom level was successfully updated, or false if it is already at or below the minimum zoom level.
    pub fn zoom_out(&mut self) -> bool {
        if self.zoom <= *self.range.start() {
            false
        } else {
            self.zoom /= self.zoom_factor;
            true
        }
    }

    /// Sets the zoom level to a specific value, if it is within the zoom range. Returns true if the zoom level was successfully updated, or false if the specified zoom level is outside the range.
    pub fn zoom_to(&mut self, zoom: f32) -> bool {
        if zoom < *self.range.start() || zoom > *self.range.end() {
            false
        } else {
            self.zoom = zoom;
            true
        }
    }

    /// Zooms by a specific factor, which multiplies the current zoom level by the factor, if the resulting zoom level is within the zoom range. Returns true if the zoom level was successfully updated, or false if the resulting zoom level is outside the range.
    pub fn zoom_by(&mut self, factor: f32) -> bool {
        let new_zoom = self.zoom * factor;
        if new_zoom < *self.range.start() || new_zoom > *self.range.end() {
            false
        } else {
            self.zoom = new_zoom;
            true
        }
    }

    /// Resets the zoom level to the default value of 1.0.
    pub fn zoom_reset(&mut self) {
        self.zoom = 1.0;
    }

    /// Returns true if the current zoom level is less than the maximum zoom level defined in the range, indicating that it is possible to zoom in further.
    pub fn can_zoom_in(&self) -> bool {
        self.zoom < *self.range.end()
    }

    /// Returns true if the current zoom level is greater than the minimum zoom level defined in the range, indicating that it is possible to zoom out further.
    pub fn can_zoom_out(&self) -> bool {
        self.zoom > *self.range.start()
    }

    /// Returns true if the current zoom level is not equal to the default value of 1.0, indicating that the value is currently zoomed in or out.
    pub fn is_zoomed(&self) -> bool {
        self.zoom != 1.0
    }

    /// Returns true if the current zoom level is greater than the default value of 1.0, indicating that the value is currently zoomed in.
    pub fn is_zoomed_in(&self) -> bool {
        self.zoom > 1.0
    }

    /// Returns true if the current zoom level is less than the default value of 1.0, indicating that the value is currently zoomed out.
    pub fn is_zoomed_out(&self) -> bool {
        self.zoom < 1.0
    }
}

// /// Extension trait that adds `on_prepaint` to any `ParentElement`.
// pub trait ElementExt: ParentElement + Sized {
//     fn on_prepaint<F>(self, f: F) -> Self
//     where
//         F: FnOnce(Bounds<Pixels>, &mut Window, &mut App) + 'static,
//     {
//         self.child(
//             canvas(
//                 move |bounds, window, cx| f(bounds, window, cx),
//                 |_, _, _, _| {},
//             )
//             .absolute()
//             .size_full(),
//         )
//     }
// }

// impl<T: ParentElement> ElementExt for T {}
