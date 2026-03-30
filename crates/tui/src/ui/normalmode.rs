use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};
use std::{sync::Arc, time::Instant};
use tachyonfx::EffectManager;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::AppAction,
    debug,
    ui::{
        UIAction,
        animate::start_sweep,
        // debug_msg,
        // popup::debug,
        // taskeditor::TaskEditor,
        // tasklist::TaskList,
        utils::{centered_area, paint_background},
        viewselector::{View, ViewSelector},
    },
};

#[derive(Debug, Clone)]
pub enum NormalModeAction {
    CreateNewTask,
    ExitTaskEditor,
    SwitchView(View),
    ClearSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ActivePane {
    ViewSelector,
    // TaskList,
    // TaskEditor,
}

pub struct NormalModeUI {
    view_selector: ViewSelector,
    // task_list: TaskList,
    // task_editor: TaskEditor,
    active_pane: ActivePane,
    effects: EffectManager<()>,
}
impl NormalModeUI {
    pub fn new(tx: UnboundedSender<AppAction>) -> Self {
        let mut view_selector = ViewSelector::new(tx.clone());
        // let mut task_list = TaskList::new(Arc::new(vec![]), tx.clone());
        // let mut task_editor = TaskEditor::new(tx.clone());
        view_selector.activate();
        // task_list.deactivate();
        // task_editor.deactivate();

        let mut effects: EffectManager<()> = EffectManager::default();
        start_sweep(500, &mut effects);

        Self {
            view_selector,
            // task_list,
            // task_editor,
            active_pane: ActivePane::ViewSelector,
            effects,
        }
    }

    pub fn execute_action(&mut self, action: NormalModeAction, _tx: &UnboundedSender<AppAction>) {
        match action {
            NormalModeAction::CreateNewTask => {
                // self.task_editor.clear_all_fields();
                // if let Some(view) = self.view_selector.get_current_view() {
                //     match view {
                //         View::Today => self.task_editor.set_due_date_content("Today"),
                //         View::Tomorrow => self.task_editor.set_due_date_content("Tomorrow"),
                //         View::Week => self.task_editor.set_due_date_content("Today"),
                //         // View::Inbox => self.task_editor.set_project_content(""),
                //         _ => {}
                //     }
                // }
                // self.active_pane = ActivePane::TaskEditor;
                // self.view_selector.deactivate();
                // self.task_list.deactivate();
                // self.task_editor.activate();
                // self.task_editor.set_mode(EditorMode::Insert);
            }
            NormalModeAction::ExitTaskEditor => {
                // if let Some(selected_task) = self.task_list.get_current_task() {
                //     self.task_editor.load_task(&selected_task);
                // } else {
                //     self.task_editor.clear_all_fields();
                // }
                // self.task_editor.clear_all_fields();
                // self.active_pane = ActivePane::TaskList;
                // self.task_editor.deactivate();
                self.view_selector.deactivate();
                // self.task_list.activate();
            }
            NormalModeAction::SwitchView(view) => {
                // self.task_list.clear_selection();
                // self.task_list.filter_by_view(&view);
                // self.task_list.start_animation();
            }
            NormalModeAction::ClearSelection => {
                // self.task_list.clear_selection();
            }
        }
    }

    fn apply_current_view_filter(&mut self) {
        if let Some(current_view) = self.view_selector.get_current_view() {
            // self.task_list.filter_by_view(current_view);
        }
    }

    pub fn get_current_view(&self) -> Option<View> {
        self.view_selector.get_current_view().cloned()
    }

    pub fn allow_key_cmd(&self) -> bool {
        match self.active_pane {
            ActivePane::ViewSelector => true,
            // ActivePane::TaskList => true,
            // ActivePane::TaskEditor => !self.task_editor.is_in_insert_mode(),
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent, tx: &UnboundedSender<AppAction>) {
        match self.active_pane {
            ActivePane::ViewSelector => self.handle_key_event_view_selector(key_event, tx),
            // ActivePane::TaskList => self.handle_key_event_task_list(key_event, tx),
            // ActivePane::TaskEditor => self.handle_key_event_task_editor(key_event),
        }
    }

    pub fn handle_key_event_view_selector(
        &mut self,
        key_event: KeyEvent,
        tx: &UnboundedSender<AppAction>,
    ) {
        match key_event.code {
            // enter task list
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                // self.active_pane = ActivePane::TaskList;
                // self.view_selector.deactivate();
                // self.task_list.activate();
            }
            // create new task
            KeyCode::Char('n') => {
                let _ = tx.send(AppAction::UIAction(UIAction::NormalMode(
                    NormalModeAction::CreateNewTask,
                )));
            }
            // // focus mode
            // KeyCode::Char('f') if self.allow_key_cmd() => {
            //     if let Some(view) = self.view_selector.get_current_view() {
            //         let _ = tx.send(AppAction::UIAction(UIAction::EnterFocusMode(view.clone())));
            //     }
            // }
            // // batch mode
            // KeyCode::Char('b') if self.allow_key_cmd() => {
            //     if let Some(view) = self.view_selector.get_current_view() {
            //         // let _ = tx.send(AppAction::UIAction(UIAction::EnterBatchMode(view.clone())));
            //         let _ = tx.send(AppAction::Batch
            //     }
            // }
            _ => self.view_selector.handle_key_event(key_event),
        }
    }

    pub fn handle_key_event_task_list(
        &mut self,
        key_event: KeyEvent,
        tx: &UnboundedSender<AppAction>,
    ) {
        match key_event.code {
            // enter view selector
            KeyCode::Left | KeyCode::Char('h') => {
                self.active_pane = ActivePane::ViewSelector;
                // self.task_list.deactivate();
                self.view_selector.activate();
            }
            // edit selected task
            KeyCode::Enter => {}
            // create new task
            KeyCode::Char('n') => {
                let _ = tx.send(AppAction::UIAction(UIAction::NormalMode(
                    NormalModeAction::CreateNewTask,
                )));
            }
            // // enter focus mode
            // KeyCode::Char('f') if self.allow_key_cmd() => {
            //     if let Some(view) = self.view_selector.get_current_view() {
            //         let _ = tx.send(AppAction::UIAction(UIAction::EnterFocusMode(view.clone())));
            //     }
            // }
            // KeyCode::Char('b') if self.allow_key_cmd() => {
            //     if let Some(view) = self.view_selector.get_current_view() {
            //         let _ = tx.send(AppAction::UIAction(UIAction::EnterBatchMode(view.clone())));
            //     }
            // }
            // _ => self.task_list.handle_key_event(key_event),
            _ => {}
        }
    }

    pub fn handle_key_event_task_editor(&mut self, key_event: KeyEvent) {
        match key_event.code {
            // KeyCode::Esc => {
            //     if self.task_editor.is_in_insert_mode() {
            //         self.task_editor.handle_key_event(key_event);
            //     } else {
            //         if self.task_editor.unsaved_changes {
            //             if let Some(selected_task) = self.task_list.get_current_task() {
            //                 self.task_editor.load_task(&selected_task);
            //             } else {
            //                 self.task_editor.clear_all_fields();
            //             }
            //         }
            //         self.active_pane = ActivePane::TaskList;
            //         self.task_editor.deactivate();
            //         self.task_list.activate();
            //     }
            // }
            // _ => self.task_editor.handle_key_event(key_event),
            _ => {}
        }
    }

    pub fn handle_mouse_event(&mut self, _mouse_event: MouseEvent) {
        // Handle mouse events specific to Normal Mode here
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        paint_background(f);

        let main_area = centered_area(area, 38, 117);
        let (view_sel_area, content_area) = create_areas(main_area);

        self.view_selector.draw(f, view_sel_area, last_frame);

        // match self.active_pane {
        //     ActivePane::TaskEditor => self.task_editor.draw(f, content_area, last_frame),
        //     _ => self.task_list.draw(f, content_area, last_frame),
        // }

        let elapsed = last_frame.elapsed();
        self.effects
            .process_effects(elapsed.into(), f.buffer_mut(), main_area);
    }
}

fn create_areas(main_area: Rect) -> (Rect, Rect) {
    let chunks = Layout::new(
        Direction::Horizontal,
        [Constraint::Length(15), Constraint::Fill(1)],
    )
    .split(main_area);

    let left_chunks = Layout::new(
        Direction::Vertical,
        [Constraint::Length(17), Constraint::Fill(1)],
    )
    .split(chunks[0]);
    (left_chunks[0], chunks[1])
}
