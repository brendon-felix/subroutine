use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::AppAction,
    ui::{editor::EditorMode, popup::Popup, taskeditor::TaskEditor, utils::centered_area},
};

pub struct NewTaskPopup {
    editor: TaskEditor,
    is_active: bool,
    block: Block<'static>,
    tx: UnboundedSender<AppAction>,
}

impl NewTaskPopup {
    pub fn new(tx: UnboundedSender<AppAction>) -> Self {
        let mut editor = TaskEditor::new().with_initial_mode(EditorMode::Insert);
        editor.activate();
        Self {
            editor,
            is_active: true,
            block: Block::new().borders(Borders::ALL),
            tx,
        }
    }

    // pub fn with_block(mut self, block: Block<'static>) -> Self {
    //     self.block = block;
    //     self
    // }
}

impl Popup for NewTaskPopup {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => {
                if self.editor.is_in_insert_mode() {
                    self.editor.handle_key_event(key_event);
                } else {
                    let _ = self.tx.send(AppAction::ClosePopup);
                }
            }
            KeyCode::Esc => {
                if self.editor.is_in_insert_mode() {
                    self.editor.handle_key_event(key_event);
                } else if self.is_active {
                    self.editor.deactivate();
                    self.is_active = false;
                } else {
                    let _ = self.tx.send(AppAction::ClosePopup);
                }
            }
            KeyCode::Enter if !self.is_active => {
                self.editor.activate();
                self.is_active = true;
                // if let Some(new_task) = self.editor.build_task() {
                //     let _ = self.tx.send(AppAction::TaskAction(
                //         new_task.project_id,
                //         new_task.id,
                //         crate::app::TaskAction::Create(new_task),
                //     ));
                //     let _ = self.tx.send(AppAction::ClosePopup);
                // }

                // let _ = self.tx.send(AppAction::ClosePopup);
            }
            _ => self.editor.handle_key_event(key_event),
        }
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
        let _ = mouse_event;
    }

    fn allow_key_cmd(&self) -> bool {
        false
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        let popup_area = centered_area(area, 32, 100);

        f.render_widget(Clear, popup_area);

        let inner_area = self.block.inner(popup_area);
        f.render_widget(&self.block, popup_area);

        let areas = Layout::new(
            Direction::Vertical,
            [Constraint::Length(3), Constraint::Fill(1)],
        )
        .split(inner_area);

        let p = Paragraph::new("\nNew Task")
            .style(Style::default().fg(Color::Green))
            .centered();
        f.render_widget(p, areas[0]);

        // let content = Layout::new(
        //     Direction::Horizontal,
        //     [Constraint::Fill(3), Constraint::Fill(1)],
        // )
        // .split(areas[1]);

        self.editor.draw(f, areas[1], last_frame);

        // f.render_widget(&self.content, areas[0]);
        // f.render_widget(self.help_text(), areas[1]);
    }
}
