use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Style, Stylize},
    text::Text,
    widgets::{Block, Clear},
};
use tachyonfx::{EffectManager, EffectTimer, Interpolation, fx};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::AppAction,
    term::TICK_PERIOD_MS,
    ui::{UIAction, popup::Popup},
};

pub struct DebugPopup {
    msg: Text<'static>,
    block: Block<'static>,
    ticks: u16,
    effects: EffectManager<()>,
    tx: UnboundedSender<AppAction>,
}

impl DebugPopup {
    pub fn new(msg: Text<'static>, ticks: u16, tx: UnboundedSender<AppAction>) -> Self {
        let msg_time = Duration::from_millis(ticks as u64 * TICK_PERIOD_MS);
        let mut effects: EffectManager<()> = EffectManager::default();
        let c = Color::Rgb(25, 25, 25);
        let delay = EffectTimer::from_ms((msg_time.as_millis() / 2) as u32, Interpolation::Linear);
        let timer = EffectTimer::from_ms((msg_time.as_millis() / 2) as u32, Interpolation::Linear);
        let fade = fx::delay(delay, fx::fade_to(c, c, timer));
        effects.add_effect(fade);
        Self {
            msg,
            block: Block::new().style(Style::default().bold().fg(Color::White).bg(Color::Red)),
            // .borders(Borders::ALL),
            // .border_style(Style::default().fg(Color::Red)),
            ticks,
            effects,
            tx,
        }
    }

    pub fn next_tick(&mut self) {
        self.ticks = self.ticks.saturating_sub(1);
    }

    pub fn is_expired(&self) -> bool {
        self.ticks == 0
    }
    // pub fn with_block(mut self, block: Block<'static>) -> Self {
    //     self.block = block;
    //     self
    // }
}

impl Popup for DebugPopup {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        // Handle key events for confirmation (e.g., Y/N)
        match key_event.code {
            KeyCode::Enter | KeyCode::Esc => {
                let _ = self.tx.send(AppAction::UIAction(UIAction::ClosePopup));
            }
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, _mouse_event: MouseEvent) {
        // Handle mouse events if necessary
    }

    fn allow_key_cmd(&self) -> bool {
        true
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        let popup_area = create_popup_area(area);
        f.render_widget(Clear, popup_area);

        // let inner_area = self.block.inner(popup_area);
        f.render_widget(&self.block, popup_area);

        f.render_widget(&self.msg, popup_area.inner(Margin::new(1, 0)));
        let elapsed = last_frame.elapsed();
        self.effects
            .process_effects(elapsed.into(), f.buffer_mut(), popup_area);
    }
}

fn create_popup_area(area: Rect) -> Rect {
    let popup_area = Layout::new(
        Direction::Vertical,
        [Constraint::Fill(1), Constraint::Length(2)],
    )
    .split(area)[1];
    Layout::new(
        Direction::Horizontal,
        [Constraint::Fill(1), Constraint::Max(80)],
    )
    .split(popup_area)[1]
}
