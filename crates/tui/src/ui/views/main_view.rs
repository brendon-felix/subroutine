use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, BorderType, Borders},
};
use simple_core::{Action, Event};
use std::time::Instant;
use tachyonfx::EffectManager;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::{AppAction, SyncStatus},
    ui::{
        utils,
        views::{AppView, pipeline_view::PipelineView, timeline_view::TimelineView},
    },
};

enum CurrentView {
    Pipeline,
    // Timeline,
    // Focus,
}

#[allow(unused)]
pub struct MainView {
    pipeline_view: PipelineView,
    // timeline_view: TimelineView,
    current_view: CurrentView,
    effects: EffectManager<()>,
    sync_status: SyncStatus,
    events: Vec<Event>,
}

#[allow(unused)]
impl MainView {
    pub fn new(tx: UnboundedSender<AppAction>) -> Self {
        let pipeline_view = PipelineView::new(tx.clone());
        // let timeline_view = TimelineView::new();
        let current_view = CurrentView::Pipeline;
        let effects: EffectManager<()> = EffectManager::default();
        let sync_status = SyncStatus::Idle;

        Self {
            pipeline_view,
            // timeline_view,
            current_view,
            effects,
            sync_status,
            events: Vec::new(),
        }
    }

    pub fn set_actions(&mut self, actions: Vec<Action>) {
        self.pipeline_view.set_actions(actions);
    }

    pub fn set_events(&mut self, events: Vec<Event>) {
        self.events = events.clone();
        self.pipeline_view.set_events(events);
    }

    pub fn set_sync_status(&mut self, status: SyncStatus) {
        self.sync_status = status.clone();
        self.pipeline_view.set_sync_status(status);
    }

    pub fn next_tick(&mut self) {}
}

impl AppView for MainView {
    fn handle_key_event(&mut self, key_event: KeyEvent, tx: &UnboundedSender<AppAction>) -> bool {
        match self.current_view {
            CurrentView::Pipeline if self.pipeline_view.handle_key_event(key_event, tx) => true,
            _ => false,
        }
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        let (sidebar_area, content_area) = create_areas(area);

        let sync_label = self.sync_status.to_string();
        let sidebar_block = Block::default()
            .title("Subroutine")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);
        let content_block = Block::default()
            .title(format!("Actions{}", sync_label))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);
        let content_inner = content_block.inner(content_area);
        f.render_widget(sidebar_block, sidebar_area);
        f.render_widget(content_block, content_area);

        match self.current_view {
            CurrentView::Pipeline => self.pipeline_view.draw(f, content_inner, last_frame),
        }
    }
}

fn create_areas(main_area: Rect) -> (Rect, Rect) {
    let chunks = Layout::new(
        Direction::Horizontal,
        [Constraint::Length(15), Constraint::Fill(1)],
    )
    .split(main_area);

    let left_chunks = Layout::new(Direction::Vertical, [Constraint::Fill(1)]).split(chunks[0]);
    (left_chunks[0], chunks[1])
}
