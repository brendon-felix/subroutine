use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::AppAction,
    ui::{UIAction, popup::Popup, utils},
};

pub struct ConfirmationPopup {
    content: Paragraph<'static>,
    block: Block<'static>,
    pending_action: AppAction,
    tx: UnboundedSender<AppAction>,
}

impl ConfirmationPopup {
    pub fn new(
        content: Paragraph<'static>,
        pending_action: AppAction,
        tx: UnboundedSender<AppAction>,
    ) -> Self {
        Self {
            content,
            block: Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
            pending_action,
            tx,
        }
    }

    // pub fn with_block(mut self, block: Block<'static>) -> Self {
    //     self.block = block;
    //     self
    // }

    pub fn help_text(&self) -> Paragraph<'static> {
        let help = Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::LightGreen)),
            // Span::raw(" confirm • "),
            Span::raw(" confirm │ "),
            Span::styled("Esc", Style::default().fg(Color::LightRed)),
            Span::raw(" cancel"),
        ]);
        // let help = Line::from(vec![
        //     Span::styled("y/Enter", Style::default().fg(Color::LightGreen).bold()),
        //     Span::raw(" to confirm, "),
        //     Span::styled("n/Esc", Style::default().fg(Color::LightRed).bold()),
        //     Span::raw(" to cancel"),
        // ]);
        // let help_content = Text::from("Press Y to confirm, N or Esc to cancel.");
        Paragraph::new(help).centered()
    }
}

impl Popup for ConfirmationPopup {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        // Handle key events for confirmation (e.g., Y/N)
        match key_event.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                let _ = self.tx.send(self.pending_action.clone());
                let _ = self.tx.send(AppAction::UIAction(UIAction::ClosePopup));
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                let _ = self.tx.send(AppAction::UIAction(UIAction::ClosePopup));
            }
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, _mouse_event: MouseEvent) {
        // Handle mouse events if necessary
    }

    fn allow_key_cmd(&self) -> bool {
        false
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, _last_frame: Instant) {
        let popup_area = utils::centered_area(area, 9, 50);

        f.render_widget(Clear, popup_area);

        let inner_area = self.block.inner(popup_area);
        f.render_widget(&self.block, popup_area);

        let areas = Layout::new(
            Direction::Vertical,
            [
                Constraint::Fill(1),
                Constraint::Min(1),
                Constraint::Fill(1),
                Constraint::Length(1),
            ],
        )
        .split(inner_area);

        f.render_widget(&self.content, areas[1]);
        f.render_widget(self.help_text(), areas[3]);
    }
}
