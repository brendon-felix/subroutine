pub mod batch;
pub mod confirm;
pub mod debug;
pub mod newtask;
// pub mod reschedule;

use std::time::Instant;

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};

pub trait Popup {
    fn handle_key_event(&mut self, key_event: KeyEvent);
    fn handle_mouse_event(&mut self, mouse_event: MouseEvent);
    fn allow_key_cmd(&self) -> bool;
    fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant);
}
