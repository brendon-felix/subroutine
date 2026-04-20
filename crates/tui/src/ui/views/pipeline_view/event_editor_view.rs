use chrono::{Local, TimeZone};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};
use simple_core::Event;
use std::time::Instant;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::app::AppAction;
use crate::ui::AppView;

pub enum EditorOutcome {
    /// Stay in the editor.
    Stay,
    /// Return to the list.
    Back,
    /// Return to the list and delete the item with this id.
    Delete(Uuid),
}

pub struct EventEditorView {
    event: Event,
}

impl EventEditorView {
    pub fn new(event: Event) -> Self {
        Self { event }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditorOutcome {
        match key.code {
            KeyCode::Esc => EditorOutcome::Back,
            KeyCode::Char('d') => EditorOutcome::Delete(self.event.id),
            _ => EditorOutcome::Stay,
        }
    }
}

impl AppView for EventEditorView {
    fn handle_key_event(&mut self, key: KeyEvent, _tx: &UnboundedSender<AppAction>) -> bool {
        !matches!(self.handle_key(key), EditorOutcome::Stay)
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, _last_frame: Instant) {
        let event = &self.event;

        let block = Block::default()
            .title(format!(" {} ", event.title))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue))
            .padding(Padding::horizontal(1));

        let inner = block.inner(area);
        f.render_widget(block, area);

        if inner.height == 0 {
            return;
        }

        let mut rows: Vec<Line> = Vec::new();

        // ── When ──────────────────────────────────────────────────────────
        let local = Local.from_utc_datetime(&event.time.naive_utc());
        let when_val = local.format("%a %b %d %Y  %H:%M").to_string();
        rows.push(field_row("\u{f017}  When", &when_val));

        // ── Duration ──────────────────────────────────────────────────────
        let dur_val = match event.duration {
            Some(d) => {
                let m = d.num_minutes();
                if m >= 60 {
                    format!("{}h {}m", m / 60, m % 60)
                } else {
                    format!("{}m", m)
                }
            }
            None => "—".to_string(),
        };
        rows.push(field_row("\u{f254}  Duration", &dur_val));

        // ── Recurrence ────────────────────────────────────────────────────
        let rec_val = match event.recurrence {
            Some(d) => {
                let m = d.num_minutes();
                if m >= 1440 {
                    format!("every {} days", m / 1440)
                } else if m >= 60 {
                    format!("every {}h", m / 60)
                } else {
                    format!("every {}m", m)
                }
            }
            None => "—".to_string(),
        };
        rows.push(field_row("\u{f021}  Recurrence", &rec_val));

        // ── Ephemeral ─────────────────────────────────────────────────────
        rows.push(field_row(
            "\u{f05e}  Ephemeral",
            if event.ephemeral { "yes" } else { "no" },
        ));

        // ── Content ───────────────────────────────────────────────────────
        rows.push(Line::from(""));
        rows.push(Line::from(Span::styled(
            "Notes",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )));
        if let Some(content) = &event.content {
            for line in content.lines() {
                rows.push(Line::from(Span::raw(line.to_string())));
            }
        } else {
            rows.push(Line::from(Span::styled(
                "—",
                Style::default().fg(Color::DarkGray),
            )));
        }

        // ── Hint ──────────────────────────────────────────────────────────
        let hint = Line::from(Span::styled(
            " [esc] back  [d] delete",
            Style::default().fg(Color::DarkGray),
        ));

        // Layout: body rows fill available height, hint pinned to last row.
        let [body_area, hint_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);

        f.render_widget(Paragraph::new(rows), body_area);
        f.render_widget(Paragraph::new(hint), hint_area);
    }
}

fn field_row<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("{:<14}", label),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(value.to_string()),
    ])
}
