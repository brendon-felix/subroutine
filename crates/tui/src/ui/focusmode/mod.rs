mod helpers;

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};
use std::{sync::Arc, time::Instant};
use tachyonfx::EffectManager;
use ticks::tasks::{Task, TaskID};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::AppAction,
    tasks::{TaskAction, TaskData},
    ui::{
        UIAction,
        animate::start_sweep,
        focuslist::{FocusList, FocusListItem, state::FocusListState},
        focusmode::helpers::{
            animate_completion, animate_scroll_down, animate_scroll_up, animate_shift_down,
            animate_shift_up, create_list_item, render_no_tasks,
        },
        utils::paint_background,
        viewselector::View,
    },
};

pub const N_TICKS: u64 = 3;

#[derive(Debug, Clone)]
pub enum FocusModeAction {
    AnimateScrollUp,
    AnimateScrollDown,
    AnimateShiftUp,
    AnimateShiftDown,
    // AnimateCompletion(bool), // is_next_available
    AnimateCompletion,
    RemoveFocusedItem,
}

pub struct FocusModeUI {
    // test_content: String,
    all_tasks: Arc<Vec<Arc<Task>>>,
    shown_tasks: Vec<TaskID>,
    current_view: Option<View>,
    list: FocusList<'static>,
    list_state: FocusListState,
    // prev_buf: Buffer,
    // focus_buf: Buffer,
    // next_buf: Buffer,
    effects: EffectManager<()>,
    // tx: UnboundedSender<AppAction>,
}

impl FocusModeUI {
    pub fn new() -> Self {
        let mut effects = EffectManager::default();
        start_sweep(500, &mut effects);
        Self {
            all_tasks: Arc::new(Vec::new()),
            shown_tasks: Vec::new(),
            current_view: None,
            list: FocusList::new(Vec::<FocusListItem>::new()),
            list_state: FocusListState::default(),
            effects,
            // tx,
        }
    }

    pub fn execute_action(&mut self, action: FocusModeAction) {
        match action {
            FocusModeAction::AnimateScrollUp => {
                animate_scroll_up(&mut self.list_state, &mut self.effects)
            }
            FocusModeAction::AnimateScrollDown => {
                animate_scroll_down(&mut self.list_state, &mut self.effects)
            }
            FocusModeAction::AnimateShiftUp => {
                animate_shift_up(&mut self.list_state, &mut self.effects)
            }
            FocusModeAction::AnimateShiftDown => {
                animate_shift_down(&mut self.list_state, &mut self.effects)
            }
            FocusModeAction::AnimateCompletion => {
                animate_completion(&mut self.list_state, &mut self.effects)
            }
            FocusModeAction::RemoveFocusedItem => self.remove_focused_item(),
        }
    }

    pub fn reset_areas(&mut self) {
        self.list_state.reset_areas();
    }

    pub fn set_all_tasks(&mut self, tasks: Arc<Vec<Arc<Task>>>) {
        self.all_tasks = tasks;
    }

    pub fn set_view(&mut self, view: View) {
        self.shown_tasks = view.get_filtered_task_ids(Utc::now(), self.all_tasks.as_ref());
        if self.shown_tasks.is_empty() {
            self.list.focus(None);
        } else {
            self.list.focus(Some(0));
        }
        self.current_view = Some(view);
    }

    pub fn get_view(&self) -> Option<&View> {
        self.current_view.as_ref()
    }

    pub fn update_tasks(&mut self, tasks: Arc<Vec<Arc<Task>>>) {
        self.set_all_tasks(tasks);
        // Apply the current view filter
        if let Some(current_view) = &self.current_view {
            self.shown_tasks =
                current_view.get_filtered_task_ids(Utc::now(), self.all_tasks.as_ref());
        } else {
            self.shown_tasks = self
                .all_tasks
                .iter()
                .map(|task| task.get_id())
                .cloned()
                .collect();
        }
        if self.shown_tasks.is_empty() {
            self.list.focus(None);
        } else if let Some(selected) = self.list.focused_index() {
            if selected >= self.shown_tasks.len() {
                self.list.focus(Some(self.shown_tasks.len() - 1));
            }
        } else {
            self.list.focus(Some(0));
        }
        // self.tasks_loaded = true;
    }

    pub fn get_current_task(&self) -> Option<Arc<Task>> {
        self.get_focused_task().map(|(_, task)| task)
    }

    fn get_focused_task(&self) -> Option<(usize, Arc<Task>)> {
        if let Some(idx) = self.list.focused_index() {
            if let Some(task_id) = self.shown_tasks.get(idx) {
                if let Some(task) = self.all_tasks.iter().find(|t| t.get_id() == task_id) {
                    return Some((idx, Arc::clone(&task)));
                }
            }
        }
        None
    }

    fn remove_focused_item(&mut self) {
        if let Some(idx) = self.list.focused_index() {
            self.shown_tasks.remove(idx);
            if self.shown_tasks.is_empty() {
                self.list.focus(None);
            } else if idx >= self.shown_tasks.len() {
                self.list.focus(Some(self.shown_tasks.len() - 1));
            }
        }
    }

    pub fn allow_key_cmd(&self) -> bool {
        true
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent, tx: &UnboundedSender<AppAction>) {
        match key_event.code {
            KeyCode::Char('j') | KeyCode::Down if self.list.focus_next() => {
                let _ = tx.send(AppAction::UIAction(UIAction::FocusMode(
                    FocusModeAction::AnimateScrollDown,
                )));
            }
            KeyCode::Char('k') | KeyCode::Up if self.list.focus_previous() => {
                let _ = tx.send(AppAction::UIAction(UIAction::FocusMode(
                    FocusModeAction::AnimateScrollUp,
                )));
            }
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('e') if self.list.len() > 0 => {
                if let Some((i, task)) = self.get_focused_task() {
                    let _ = tx.send(AppAction::UIAction(UIAction::FocusMode(
                        FocusModeAction::AnimateCompletion,
                    )));
                    let data = TaskData::from_task(&task);
                    let shift_action = if i + 1 < self.shown_tasks.len() {
                        FocusModeAction::AnimateShiftUp
                    } else {
                        FocusModeAction::AnimateShiftDown
                    };
                    let _ = tx.send(AppAction::AfterNTicks(
                        N_TICKS as u32 - 1,
                        Box::new(AppAction::MultiAction(vec![
                            AppAction::TaskAction(TaskAction::Complete, data),
                            AppAction::UIAction(UIAction::FocusMode(
                                FocusModeAction::RemoveFocusedItem,
                            )),
                            AppAction::UIAction(UIAction::FocusMode(shift_action)),
                        ])),
                    ));
                }
            }
            KeyCode::Esc => {
                let _ = tx.send(AppAction::UIAction(UIAction::ExitToNormalMode));
            }
            _ => {}
        }
    }

    pub fn handle_mouse_event(&mut self, _mouse_event: MouseEvent) {
        // Handle mouse events specific to Focus Mode here
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        paint_background(f);

        let items: Vec<FocusListItem> = self
            .shown_tasks
            .iter()
            .filter_map(|task_id| self.all_tasks.iter().find(|t| t.get_id() == task_id))
            .map(|task| create_list_item(task))
            .collect();
        self.list.set_items(items);

        if self.list.len() > 0 {
            self.list_state.update_animations();
            f.render_stateful_widget(&self.list, area, &mut self.list_state);
        } else {
            render_no_tasks(f, area);
        }

        let elapsed = last_frame.elapsed();
        self.effects
            .process_effects(elapsed.into(), f.buffer_mut(), area);
    }
}
