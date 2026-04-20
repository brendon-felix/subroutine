use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect, text::Text};

mod animate;
pub mod utils;
mod views;

pub use views::*;

use std::time::Instant;
use tokio::sync::mpsc::UnboundedSender;

use crate::app::AppAction;

#[derive(Debug, Clone)]
pub enum UIAction {
    Dialog(Text<'static>, Box<AppAction>),
    DebugMsg(String, u16),
}
