use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Clear, Widget},
};
// use tokio::sync::mpsc::UnboundedSender;

// use crate::{app::AppAction, ui::UIAction};

pub fn centered_area(area: Rect, height: u16, width: u16) -> Rect {
    Layout::new(
        Direction::Horizontal,
        [
            Constraint::Fill(1),
            Constraint::Length(width),
            Constraint::Fill(1),
        ],
    )
    .split(
        Layout::new(
            Direction::Vertical,
            [
                Constraint::Fill(1),
                Constraint::Length(height),
                Constraint::Fill(1),
            ],
        )
        .split(area)[1],
    )[1]
}

pub fn paint_background(f: &mut Frame) {
    Clear.render(f.area(), f.buffer_mut());
    Block::default()
        .style(Style::default().bg(Color::Rgb(25, 25, 25)))
        .render(f.area(), f.buffer_mut());
}

// pub fn debug(msg: impl Into<String>, tx: &UnboundedSender<AppAction>) {
//     tx.send(AppAction::UIAction(UIAction::DebugMsg(msg.into(), 5000)))
//         .unwrap();
// }
