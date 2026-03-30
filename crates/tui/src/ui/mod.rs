use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::Rect,
    text::Text,
    widgets::{Paragraph, Wrap},
};

mod animate;
// mod batchmode;
// mod composite;
// mod multiselect;
mod normalmode;
pub mod popup;
// mod tasklist;
mod utils;
mod viewselector;

// use focusmode::FocusModeUI;
use normalmode::NormalModeUI;
use std::{sync::Arc, time::Instant};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::AppAction,
    ui::{
        // batchmode::BatchModeUI,
        // focusmode::FocusModeAction,
        normalmode::NormalModeAction,
        popup::{
            Popup, batch::BatchCreatePopup, confirm::ConfirmationPopup, debug::DebugPopup,
            newtask::NewTaskPopup,
        },
        viewselector::View,
    },
};

#[derive(Debug, Clone)]
pub enum UIAction {
    // EnterFocusMode(View),
    // EnterBatchMode(View),
    ExitToNormalMode,
    NormalMode(NormalModeAction),
    // FocusMode(FocusModeAction),
    Confirm(Text<'static>, Box<AppAction>),
    NewTask,
    BatchCreateTask,
    // RescheduleTask(String),
    ClosePopup,
    DebugMsg(String, u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AppUIMode {
    Normal,
    // Focus,
    // Batch,
}

pub struct AppUI {
    mode: AppUIMode,
    normal_ui: NormalModeUI,
    // focus_ui: FocusModeUI,
    // batch_ui: BatchModeUI,
    popup: Option<Box<dyn Popup>>,
    debug_popup: Option<DebugPopup>,
    tx: UnboundedSender<AppAction>,
}

impl AppUI {
    pub fn new(tx: UnboundedSender<AppAction>) -> Self {
        Self {
            mode: AppUIMode::Normal,
            normal_ui: NormalModeUI::new(tx.clone()),
            // focus_ui: FocusModeUI::new(),
            // batch_ui: BatchModeUI::new(),
            popup: None,
            debug_popup: None,
            tx,
        }
    }

    pub fn execute_action(&mut self, action: UIAction, tx: &UnboundedSender<AppAction>) {
        match action {
            // UIAction::EnterFocusMode(view) => {
            //     self.mode = AppUIMode::Focus;
            //     // self.focus_ui.set_view(view);
            // }
            UIAction::ExitToNormalMode => {
                self.mode = AppUIMode::Normal;
            }
            // UIAction::EnterBatchMode(view) => {
            //     self.mode = AppUIMode::Batch;
            // }
            UIAction::NormalMode(normal_action) => {
                self.normal_ui.execute_action(normal_action, tx);
            }
            // UIAction::FocusMode(focus_action) => {
            //     self.focus_ui.execute_action(focus_action);
            // }
            UIAction::Confirm(msg, pending_action) => {
                self.confirm(msg, *pending_action);
            }
            UIAction::NewTask => {
                self.new_task();
            }
            UIAction::BatchCreateTask => {
                self.batch_create_task();
            }
            // UIAction::RescheduleTask(duration_str) => {
            //     self.reschedule_task(duration_str);
            // }
            UIAction::ClosePopup => {
                self.close_popup();
            }
            UIAction::DebugMsg(msg, n_ticks) => {
                self.debug(msg, n_ticks);
            }
        }
    }

    pub fn next_tick(&mut self) {
        // self.focus_ui.next_tick();
        // self.normal_ui.next_tick();
        if let Some(debug_popup) = &mut self.debug_popup {
            debug_popup.next_tick();
            if debug_popup.is_expired() {
                self.debug_popup = None;
            }
        }
    }

    pub fn reset_areas(&mut self) {
        // self.focus_ui.reset_areas();
        // self.normal_ui.reset_areas();
    }

    pub fn debug(&mut self, msg: String, n_ticks: u16) {
        self.debug_popup = Some(DebugPopup::new(
            Text::from(msg),
            n_ticks, // 2 seconds
            self.tx.clone(),
        ));
    }

    pub fn confirm(&mut self, msg: Text<'static>, pending_action: AppAction) {
        let popup = ConfirmationPopup::new(
            Paragraph::new(msg).centered().wrap(Wrap::default()),
            pending_action,
            self.tx.clone(),
        );
        self.popup = Some(Box::new(popup));
    }

    // pub fn reschedule_popup(&mut self) {
    //     let selected_tasks: Vec<TaskData> = match self.mode {
    //         AppUIMode::Normal => self
    //             .normal_ui
    //             .get_selected_tasks()
    //             .iter()
    //             .map(|task| TaskData::from_task(task))
    //             .collect(),
    //         AppUIMode::Focus => {
    //             if let Some(current_task) = self.focus_ui.get_current_task() {
    //                 vec![TaskData::from_task(&current_task)]
    //             } else {
    //                 vec![]
    //             }
    //         }
    //     };
    //     let popup = ReschedulePopup::new(selected_tasks, self.tx.clone());
    //     self.popup = Some(Box::new(popup));
    // }

    // pub fn reschedule_task(&mut self, duration_str: String) {
    //     // Get selected tasks from current mode
    //     let selected_tasks = match self.mode {
    //         AppUIMode::Normal => self.normal_ui.get_selected_tasks(),
    //         AppUIMode::Focus => {
    //             // Focus mode doesn't support multi-select, just get current task
    //             if let Some(current_task) = self.focus_ui.get_current_task() {
    //                 vec![current_task]
    //             } else {
    //                 vec![]
    //             }
    //         }
    //     };

    //     if !selected_tasks.is_empty() {
    //         // Create task data for all selected tasks with the same duration string
    //         let task_data_list: Vec<crate::tasks::TaskData> = selected_tasks
    //             .into_iter()
    //             .map(|task| {
    //                 let mut task_data = crate::tasks::TaskData::from_task(&task);
    //                 task_data.reschedule_duration = Some(duration_str.clone());
    //                 task_data
    //             })
    //             .collect();

    //         // Use the new RescheduleMultipleTasks action to handle relative timing
    //         let _ = self
    //             .tx
    //             .send(AppAction::RescheduleMultipleTasks(task_data_list));
    //     }
    // }

    pub fn new_task(&mut self) {
        let view = match self.mode {
            AppUIMode::Normal => self.normal_ui.get_current_view(),
            // AppUIMode::Focus => self.focus_ui.get_view().cloned(),
            // AppUIMode::Batch => return,
        };
        let view = view.unwrap_or(View::Today);
        let popup = NewTaskPopup::new(view, self.tx.clone());
        self.popup = Some(Box::new(popup));
    }

    pub fn batch_create_task(&mut self) {
        let view = match self.mode {
            AppUIMode::Normal => self.normal_ui.get_current_view(),
            // AppUIMode::Focus => self.focus_ui.get_view().cloned(),
            // AppUIMode::Batch => return,
        };
        let view = view.unwrap_or(View::Today);
        let popup = BatchCreatePopup::new(view, self.tx.clone());
        self.popup = Some(Box::new(popup));
    }

    pub fn close_popup(&mut self) {
        if self.debug_popup.is_some() {
            self.debug_popup = None;
            return;
        }
        if self.popup.is_some() {
            self.popup = None;
        }
    }

    // pub fn apply_view_filter(&mut self, view: &crate::ui::views::View) {
    //     match self.mode {
    //         AppUIMode::Focus => self.focus_ui.filter_by_view(view),
    //         AppUIMode::Normal => {
    //             // The normal mode will handle view filtering internally
    //             // when the view list selection changes
    //         }
    //     }
    // }

    pub fn allow_key_cmd(&self) -> bool {
        match &self.popup {
            Some(p) => p.allow_key_cmd(),
            None => match self.mode {
                AppUIMode::Normal => self.normal_ui.allow_key_cmd(),
                // AppUIMode::Focus => self.focus_ui.allow_key_cmd(),
                // AppUIMode::Batch => self.batch_ui.allow_key_cmd(),
            },
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent, tx: &UnboundedSender<AppAction>) {
        // 'q' and 'ctrl+c' are handled by app.rs
        match key_event.code {
            KeyCode::Char('n') if self.allow_key_cmd() => {
                let _ = tx.send(AppAction::UIAction(UIAction::NewTask));
            }
            KeyCode::Char('b') if self.allow_key_cmd() => {
                let _ = tx.send(AppAction::UIAction(UIAction::BatchCreateTask));
            }
            KeyCode::Char('r') if self.allow_key_cmd() => {
                // self.reschedule_popup();
            }
            _ => {
                // if let Some(debug_popup) = &mut self.debug_popup {
                //     debug_popup.handle_key_event(key_event);
                //     return;
                // }
                if let Some(popup) = &mut self.popup {
                    popup.handle_key_event(key_event);
                    return;
                }
                match self.mode {
                    AppUIMode::Normal => self.normal_ui.handle_key_event(key_event, tx),
                    // AppUIMode::Focus => self.focus_ui.handle_key_event(key_event, tx),
                    // AppUIMode::Batch => self.batch_ui.handle_key_event(key_event, tx),
                }
            }
        }
    }

    pub fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
        if let Some(debug_popup) = &mut self.debug_popup {
            debug_popup.handle_mouse_event(mouse_event);
            return;
        }
        if let Some(popup) = &mut self.popup {
            popup.handle_mouse_event(mouse_event);
            return;
        }
        match self.mode {
            AppUIMode::Normal => self.normal_ui.handle_mouse_event(mouse_event),
            // AppUIMode::Focus => self.focus_ui.handle_mouse_event(mouse_event),
            // AppUIMode::Batch => self.batch_ui.handle_mouse_event(mouse_event),
        }
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        match self.mode {
            AppUIMode::Normal => self.normal_ui.draw(f, area, last_frame),
            // AppUIMode::Focus => self.focus_ui.draw(f, area, last_frame),
            // AppUIMode::Batch => self.batch_ui.draw(f, area, last_frame),
        }
        if let Some(popup) = &mut self.popup {
            popup.draw(f, area, last_frame);
        }
        if let Some(debug_popup) = &mut self.debug_popup {
            debug_popup.draw(f, area, last_frame);
        }
    }
}

// pub fn debug_msg(msg: &str, n_ticks: u16, tx: &UnboundedSender<AppAction>) {
//     let _ = tx.send(AppAction::UIAction(UIAction::DebugMsg(
//         msg.to_string(),
//         n_ticks,
//     )));
// }
