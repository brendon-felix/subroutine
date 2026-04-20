use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Frame, layout::Rect};
use simple_core::{Action, Event};
use std::time::Instant;
use tachyonfx::EffectManager;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::{AppAction, SyncStatus},
    ui::{
        utils::centered_area,
        views::{AppView, main_view::MainView},
    },
};

#[allow(unused)]
pub struct RootView {
    main_view: MainView,
    effects: EffectManager<()>,
}

#[allow(unused)]
impl RootView {
    pub fn new(tx: UnboundedSender<AppAction>) -> Self {
        let main_view = MainView::new(tx.clone());
        let effects: EffectManager<()> = EffectManager::default();
        Self { main_view, effects }
    }

    pub fn next_tick(&mut self) {}

    pub fn set_actions(&mut self, actions: Vec<Action>) {
        self.main_view.set_actions(actions);
    }

    pub fn set_events(&mut self, events: Vec<Event>) {
        self.main_view.set_events(events);
    }

    pub fn set_sync_status(&mut self, status: SyncStatus) {
        self.main_view.set_sync_status(status);
    }
}

impl AppView for RootView {
    fn handle_key_event(&mut self, key_event: KeyEvent, tx: &UnboundedSender<AppAction>) -> bool {
        if self.main_view.handle_key_event(key_event, tx) {
            return true;
        }
        false
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        let root_area = centered_area(area, 38, 117);
        self.main_view.draw(f, root_area, last_frame);
    }
}
