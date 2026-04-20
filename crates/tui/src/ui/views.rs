use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};
use std::time::Instant;
use tachyonfx::EffectManager;
use tokio::sync::mpsc::UnboundedSender;

mod main_view;
mod pipeline_view;
mod root_view;
mod timeline_view;

pub use root_view::*;

use crate::{
    app::AppAction,
    ui::{UIAction, utils::paint_background},
};

#[allow(unused)]
pub trait AppView {
    fn execute_action(&mut self, action: UIAction, tx: &UnboundedSender<AppAction>) {}

    fn handle_key_event(&mut self, key_event: KeyEvent, tx: &UnboundedSender<AppAction>) -> bool {
        false
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {}

    fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant);

    fn render(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        paint_background(f);
        self.draw(f, area, last_frame);
    }
}

#[allow(unused)]
pub trait EffectsView: AppView {
    fn effects(&mut self) -> &mut EffectManager<()>;

    fn render(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        paint_background(f);
        self.draw(f, area, last_frame);
        self.effects()
            .process_effects(last_frame.elapsed().into(), f.buffer_mut(), area);
    }
}
