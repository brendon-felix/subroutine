use std::{sync::Arc, time::Instant};

use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::AppAction,
    // tasks::{is_due_this_week, is_due_today, is_due_tomorrow, is_in_inbox, is_overdue},
    ui::{UIAction, normalmode::NormalModeAction},
};
// use ticks::projects::ProjectID;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    Today,
    Tomorrow,
    Week,
    Inbox,
    // Project(ProjectID),
    All,
}

impl View {
    pub fn get_name(&self) -> String {
        let s = match self {
            View::Today => "Today",
            View::Tomorrow => "Tomorrow",
            View::Week => "This Week",
            View::Inbox => "Inbox",
            View::All => "All Tasks",
            // View::Project(project_id) => "Project", // Placeholder
        };
        s.into()
    }

    // pub fn contains_task(&self, now: DateTime<Utc>, task: &Task) -> bool {
    //     match self {
    //         View::Today => is_due_today(now, task) | is_overdue(now, task),
    //         View::Tomorrow => is_due_tomorrow(now, task),
    //         View::Week => is_due_this_week(now, task),
    //         View::Inbox => is_in_inbox(task),
    //         View::All => true,
    //         // View::Project(project_id) => {
    //         //     // Implement filtering logic for tasks in the specified project
    //         //     true
    //         //
    //     }
    // }

    // pub fn get_filtered_task_ids(
    //     &self,
    //     now: DateTime<Utc>,
    //     all_tasks: &Vec<Arc<Task>>,
    // ) -> Vec<TaskID> {
    //     // all_tasks
    //     //     .iter()
    //     //     .filter(|task| self.contains_task(now, task))
    //     //     .map(|task| task.get_id())
    //     //     .collect()
    //     all_tasks
    //         .iter()
    //         .filter(|task| self.contains_task(now, task))
    //         .map(|task| task.get_id())
    //         .cloned()
    //         .collect()
    // }
}

pub struct ViewSelector {
    pub views: Vec<View>,
    list_state: ListState,
    style: Style,
    current_block: Option<Block<'static>>,
    tx: UnboundedSender<AppAction>,
}

impl ViewSelector {
    pub fn new(tx: UnboundedSender<AppAction>) -> Self {
        let current_block = Some(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
        Self {
            views: vec![
                View::Today,
                View::Tomorrow,
                View::Week,
                View::Inbox,
                View::All,
            ],
            list_state: ListState::default().with_selected(Some(0)),
            style: Style::default(),
            current_block,
            tx,
        }
    }

    // pub fn add_view(&mut self, view: View) {
    //     self.views.push(view);
    // }

    pub fn activate(&mut self) {
        self.current_block = Some(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
        self.style = Style::default();
    }

    pub fn deactivate(&mut self) {
        self.current_block = Some(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .style(Style::default().add_modifier(Modifier::DIM)),
        );
        self.style = Style::default().add_modifier(Modifier::DIM);
    }

    pub fn get_current_view(&self) -> Option<&View> {
        self.list_state
            .selected()
            .and_then(|idx| self.views.get(idx))
    }

    // pub fn set_selection(&mut self, index: usize) {
    //     if index < self.views.len() {
    //         self.list_state.select(Some(index));
    //     }
    // }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        let idx = self.list_state.selected();
        match key_event.code {
            KeyCode::Char('j') | KeyCode::Down => self.list_state.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.list_state.select_previous(),
            KeyCode::Char('g') => self.list_state.select_first(),
            KeyCode::Char('G') => self.list_state.select_last(),
            _ => {}
        }
        if idx != self.list_state.selected() {
            if let Some(view) = self.get_current_view() {
                let _ = self.tx.send(AppAction::UIAction(UIAction::NormalMode(
                    NormalModeAction::SwitchView(view.clone()),
                )));
            }
        }
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, _last_frame: Instant) {
        let items: Vec<ListItem> = self
            .views
            .iter()
            .map(|view| create_list_item(view))
            .collect();
        let mut list = List::new(items)
            .style(self.style)
            .highlight_symbol(" ")
            .highlight_style(
                Style::new()
                    .bg(Color::Rgb(50, 50, 50))
                    .add_modifier(Modifier::BOLD),
            );
        if let Some(block) = self.current_block.clone() {
            list = list.block(block);
        }
        f.render_stateful_widget(list, area, &mut self.list_state);
    }
}

fn create_list_item(view: &View) -> ListItem<'static> {
    let line1 = Line::from("");
    let line2 = Line::from(view.get_name()).centered();
    let line3 = Line::from("");
    ListItem::new(vec![line1, line2, line3])
}
