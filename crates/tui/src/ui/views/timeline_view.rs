use std::time::Instant;

use chrono::{DateTime, Local, Timelike};
use crossterm::event::{KeyCode, KeyEvent};
use futures::stream::select;
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, StatefulWidget},
};
use tokio::sync::mpsc::UnboundedSender;
// use tui_scrollview::{ScrollView, ScrollViewState};
use tui_widget_list::{ListBuilder, ListState, ListView};

use crate::{
    app::AppAction,
    ui::{AppView, UIAction, utils},
};

pub struct TimelineState {
    // scroll_state: ScrollViewState,
    now: DateTime<Local>,
    list_state: ListState,
}

impl TimelineState {
    pub fn new() -> Self {
        let now = Local::now();
        let list_state = ListState::default();
        Self {
            // scroll_state: ScrollViewState::new(),
            now,
            list_state,
        }
    }
}

pub struct Timeline;

impl StatefulWidget for Timeline {
    type State = TimelineState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let selected_index = state.list_state.selected.unwrap_or_default();

        let list_length = selected_index + 12;

        let builder = ListBuilder::new(|context| {
            // time label is the bottom of the hour for the given index offset from the bottom of the current hour
            let datetime = state
                .now
                .with_minute(0)
                .unwrap()
                .with_second(0)
                .unwrap()
                .with_nanosecond(0)
                .unwrap()
                + chrono::Duration::hours(context.index as i64);

            // Format the datetime as "12am", "1pm", etc.
            // let datetime_label = datetime.format("%I%P").to_string();
            let datetime_label = match datetime.time().hour12() {
                (false, 12) => datetime.format("12am %b %d").to_string(), // Show date for 12am
                (false, h) => format!("{}am", h),
                (true, h) => format!("{}pm", h),
            };
            let item = if context.is_selected {
                let block = Block::new()
                    .borders(Borders::ALL)
                    .border_type(ratatui::widgets::BorderType::Rounded);
                Paragraph::new(datetime_label).block(block)
            } else {
                let block = Block::new()
                    .borders(Borders::LEFT)
                    .style(Style::default().fg(Color::DarkGray));
                Paragraph::new(format!("\n{}", datetime_label)).block(block)
            };

            // // Apply conditional styling for selected items
            // if context.is_selected {
            //     item = item.style(Style::default());
            // }

            let height = if context.is_selected { 11 } else { 3 };

            // Return (Widget, Height)
            (item, height)
        });

        let list = ListView::new(builder, list_length)
            .infinite_scrolling(false)
            .scroll_padding(2);
        list.render(area, buf, &mut state.list_state);
    }
}

pub struct TimelineView {
    timeline_state: TimelineState,
}

impl TimelineView {
    pub fn new() -> Self {
        Self {
            timeline_state: TimelineState::new(),
        }
    }
}

impl AppView for TimelineView {
    fn handle_key_event(&mut self, key_event: KeyEvent, tx: &UnboundedSender<AppAction>) -> bool {
        match key_event.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.timeline_state.list_state.previous();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.timeline_state.list_state.next();
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.timeline_state.list_state.select(Some(0));
            }
            // KeyCode::End | KeyCode::Char('G') => {
            //     self.timeline_state.list_state.select();
            // }
            // KeyCode::PageUp => {
            //     self.timeline_state.scroll_page_up();
            // }
            // KeyCode::PageDown => {
            //     self.timeline_state.scroll_page_down();
            // }
            _ => return false,
        }
        true
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, _last_frame: Instant) {
        Timeline.render(area, f.buffer_mut(), &mut self.timeline_state);
    }
}
