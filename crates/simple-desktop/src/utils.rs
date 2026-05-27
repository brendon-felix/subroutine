use std::{
    ops::{Mul, RangeInclusive},
    time::Duration,
};

use gpui::{
    App, DefiniteLength, Div, Hsla, InteractiveElement, Stateful, StatefulInteractiveElement,
    Styled, prelude::FluentBuilder, transparent_white,
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

pub trait AppAnimation {
    fn duration(&self) -> Duration;
}

pub fn fraction(f: f32) -> DefiniteLength {
    DefiniteLength::Fraction(f)
}

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

    pub fn with_zoom_factor(mut self, zoom_factor: f32) -> Self {
        self.zoom_factor = zoom_factor;
        self
    }

    pub fn with_range(mut self, range: RangeInclusive<f32>) -> Self {
        self.range = range;
        self
    }

    pub fn current_value(&self) -> T {
        self.base_value * self.zoom
    }

    pub fn zoom_in(&mut self) -> bool {
        if self.zoom >= *self.range.end() {
            false
        } else {
            self.zoom *= self.zoom_factor;
            true
        }
    }

    pub fn zoom_out(&mut self) -> bool {
        if self.zoom <= *self.range.start() {
            false
        } else {
            self.zoom /= self.zoom_factor;
            true
        }
    }

    pub fn zoom_to(&mut self, zoom: f32) -> bool {
        if zoom < *self.range.start() || zoom > *self.range.end() {
            false
        } else {
            self.zoom = zoom;
            true
        }
    }

    pub fn zoom_by(&mut self, factor: f32) -> bool {
        let new_zoom = self.zoom * factor;
        if new_zoom < *self.range.start() || new_zoom > *self.range.end() {
            false
        } else {
            self.zoom = new_zoom;
            true
        }
    }

    pub fn zoom_reset(&mut self) {
        self.zoom = 1.0;
    }

    pub fn is_zoomed(&self) -> bool {
        self.zoom != 1.0
    }

    pub fn is_zoomed_in(&self) -> bool {
        self.zoom > 1.0
    }

    pub fn is_zoomed_out(&self) -> bool {
        self.zoom < 1.0
    }
}
