use std::time::Instant;

use chrono::{DateTime, Local, TimeZone, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, List, ListItem, ListState},
};
use tachyonfx::{EffectManager, EffectTimer, Interpolation, Motion, fx};
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::Input;

use crate::{
    app::AppAction,
    tasks::{TaskData, format_repeat_flag},
    ui::{
        UIAction,
        composite::{CompositeEditor, CompositeEditorState},
        editor::{
            Editor, EditorMode,
            actions::{EditorAction, EditorActions},
            handlers,
        },
        utils::{self, centered_area, paint_background},
    },
};

pub enum BatchModeAction {}

#[derive(PartialEq, Eq)]
enum ActivePane {
    Editor,
    TaskList,
}

pub struct BatchModeUI {
    list_state: ListState,
    editor: CompositeEditor,
    editor_state: CompositeEditorState,
    tasks: Vec<TaskData>,
    active_pane: ActivePane,
    next_task_time: DateTime<Utc>,
    editing_task_index: Option<usize>,
    last_list_area: Option<Rect>,
    effects: EffectManager<()>,
}

fn splits_fn(area: Rect) -> Vec<Rect> {
    let split = Layout::new(
        Direction::Horizontal,
        [Constraint::Fill(3), Constraint::Fill(1)],
    )
    .split(area);
    vec![split[0], split[1]]
}

fn validate_duedate(input: &str) -> Option<bool> {
    if input.trim().is_empty() {
        None
    } else {
        Some(utils::parse_datetime(input).is_ok())
    }
}

impl BatchModeUI {
    pub fn new() -> Self {
        let list_state = ListState::default();
        // let mut editor =
        //     CompositeEditor::new().with_block(Block::bordered().border_type(BorderType::Rounded));
        let mut editor = CompositeEditor::new(vec![
            Editor::new()
                .with_block(
                    Block::bordered()
                        .title("Title")
                        .border_type(BorderType::Rounded),
                )
                .with_single_line(),
            // .with_placeholder("Enter task title here..."),
            Editor::new()
                .with_block(
                    Block::bordered()
                        .title("Due")
                        .border_type(BorderType::Rounded),
                )
                .with_validator(validate_duedate)
                .with_overridden_cursor_style(Style::default())
                .with_single_line(),
            // .with_placeholder("Enter due date here..."),
        ]);
        editor.set_mode(EditorMode::Insert);
        let editor_state = CompositeEditorState::new(2, Box::new(splits_fn));
        let tasks = Vec::new();
        let active_pane = ActivePane::Editor;
        let now = Utc::now();
        editor.editors[1].set_content(&utils::format_datetime(now, false));
        let mut effects = EffectManager::default();
        let c = Color::Rgb(25, 25, 25);
        let timer = EffectTimer::from_ms(500, Interpolation::Linear);
        let fx = fx::sweep_in(Motion::UpToDown, 10, 0, c, timer);
        effects.add_effect(fx);
        Self {
            list_state,
            editor,
            editor_state,
            tasks,
            active_pane,
            next_task_time: now,
            editing_task_index: None,
            last_list_area: None,
            effects,
        }
    }

    pub fn execute_action(&mut self, action: BatchModeAction) {
        match action {
            // Handle Batch Mode actions here
        }
    }

    pub fn allow_key_cmd(&self) -> bool {
        match self.active_pane {
            ActivePane::Editor => {
                if let Some(mode) = self.editor.get_mode() {
                    mode == EditorMode::Normal
                } else {
                    false
                }
            }
            ActivePane::TaskList => true,
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent, tx: &UnboundedSender<AppAction>) {
        match key_event.code {
            KeyCode::Esc if self.allow_key_cmd() => {
                let _ = tx.send(AppAction::UIAction(UIAction::ExitToNormalMode));
                return;
            }
            KeyCode::Enter
                if key_event.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) =>
            {
                self.publish_all(tx);
                return;
            }
            _ => {}
        }
        match self.active_pane {
            ActivePane::Editor => match key_event.code {
                KeyCode::Up | KeyCode::Char('K')
                    if key_event.modifiers == KeyModifiers::SHIFT
                        && self.editor.is_last_editor_active() =>
                {
                    self.next_task_time = self.next_task_time + chrono::Duration::minutes(1);
                    self.editor.editors[1]
                        .set_content(&utils::format_datetime(self.next_task_time, false));
                }
                KeyCode::Down | KeyCode::Char('J')
                    if key_event.modifiers == KeyModifiers::SHIFT
                        && self.editor.is_last_editor_active() =>
                {
                    self.next_task_time = self.next_task_time - chrono::Duration::minutes(1);
                    self.editor.editors[1]
                        .set_content(&utils::format_datetime(self.next_task_time, false));
                }
                KeyCode::Up | KeyCode::Char('k') if self.editor.is_last_editor_active() => {
                    self.next_task_time = self.next_task_time + chrono::Duration::minutes(5);
                    self.editor.editors[1]
                        .set_content(&utils::format_datetime(self.next_task_time, false));
                }
                KeyCode::Down | KeyCode::Char('j') if self.editor.is_last_editor_active() => {
                    self.next_task_time = self.next_task_time - chrono::Duration::minutes(5);
                    self.editor.editors[1]
                        .set_content(&utils::format_datetime(self.next_task_time, false));
                }
                KeyCode::Enter if self.editor.is_last_editor_active() => {
                    self.submit();
                }
                KeyCode::Tab if self.editor.is_last_editor_active() => {
                    self.editor.set_pending_action(None);
                    self.editor.set_active_editor(None);
                    self.active_pane = ActivePane::TaskList;
                    if !self.tasks.is_empty() {
                        self.list_state.select(Some(0));
                    }
                }
                KeyCode::Tab => {
                    self.editor.set_active_editor_next();
                    self.editor.set_mode(EditorMode::Normal);
                }
                KeyCode::BackTab => {
                    self.editor.set_active_editor_previous();
                }
                _ => self.handle_key_event_editor(key_event),
            },
            ActivePane::TaskList => match key_event.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(selected) = self.list_state.selected() {
                        if selected > 0 {
                            self.list_state.select(Some(selected - 1));
                        }
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(selected) = self.list_state.selected() {
                        if selected < self.tasks.len() - 1 {
                            self.list_state.select(Some(selected + 1));
                        }
                    } else if !self.tasks.is_empty() {
                        self.list_state.select(Some(0));
                    }
                }
                KeyCode::Esc | KeyCode::BackTab => {
                    self.active_pane = ActivePane::Editor;
                    self.editor.set_active_editor(Some(1));
                }
                KeyCode::Delete | KeyCode::Char('D') => {
                    if let Some(selected) = self.list_state.selected() {
                        self.tasks.remove(selected);
                        if self.tasks.is_empty() {
                            self.list_state.select(None);
                            self.active_pane = ActivePane::Editor;
                            self.editor.set_active_editor(Some(0));
                        } else if selected >= self.tasks.len() {
                            self.list_state.select(Some(self.tasks.len() - 1));
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(selected) = self.list_state.selected() {
                        let task = &self.tasks[selected];
                        let title = task.title.as_deref().unwrap_or("");
                        self.editor.editors[0].set_content(title);
                        let due_date_str = if let Some(dt) = &task.due_date {
                            utils::format_datetime(*dt, false)
                        } else {
                            "".to_string()
                        };
                        self.editor.editors[1].set_content(&due_date_str);
                        self.editing_task_index = Some(selected);
                        self.active_pane = ActivePane::Editor;
                        self.editor.set_active_editor(Some(1));
                    }
                }
                _ => {}
            },
        }
    }

    fn handle_key_event_editor(&mut self, key_event: KeyEvent) {
        let input: Input = key_event.into();
        if let Some(mode) = self.editor.get_mode() {
            let action_opt = if let Some(pending_action) = self.editor.get_pending_action() {
                match handlers::handle_pending_action_input(input, pending_action) {
                    Some(action) => Some(action),
                    None => {
                        self.editor.set_pending_action(None);
                        None
                    }
                }
            } else {
                handlers::handle_input(input, mode, true)
            };
            if let Some(action) = action_opt {
                match action {
                    EditorAction::Submit if self.editor.is_last_editor_active() => self.submit(),
                    EditorAction::Submit => self.submit_field(),
                    _ => self.editor.execute_action(action),
                }
            }
        }
    }

    fn parse_inputs(&self) -> TaskData {
        let mut data = TaskData::default();

        let title = self.editor.editors[0].get_content().trim().to_string();
        if !title.is_empty() {
            data = data.title(title);
        }

        let due_date_input = self.editor.editors[1].get_content().trim().to_string();
        if !due_date_input.is_empty() {
            if let Ok(dt) = utils::parse_datetime(&due_date_input) {
                // Convert NaiveDateTime to DateTime<Utc> by treating it as local time first
                let dt_utc = Local.from_local_datetime(&dt).unwrap().with_timezone(&Utc);
                data = data.due_date(dt_utc);
            }
        }

        data
    }

    fn submit(&mut self) {
        // if self.editor.all_valid() {
        //     let task = self.parse_inputs();
        //     if task.title.is_some() {
        //         if let Some(dt) = &task.due_date {
        //             self.next_task_time = dt.clone() + chrono::Duration::minutes(5);
        //         }
        //         self.editor.editors[1]
        //             .set_content(&utils::format_datetime(self.next_task_time, false));
        //         self.tasks.push(task);
        //         self.tasks.sort_by_key(|t| {
        //             t.due_date
        //                 .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap())
        //         });
        //         self.editor.editors[0].set_content("");
        //         self.editor.set_active_editor_first();
        //     }
        // }

        if self.editor.all_valid() {
            let task = self.parse_inputs();
            if let Some(task_idx) = self.editing_task_index {
                if task.title.is_some() {
                    self.tasks[task_idx] = task;
                    self.tasks.sort_by_key(|t| {
                        t.due_date
                            .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap())
                    });
                    self.editing_task_index = None;
                    self.editor.editors[0].set_content("");
                    self.next_task_time = Utc::now();
                    self.editor.editors[1]
                        .set_content(&utils::format_datetime(self.next_task_time, false));
                    self.editor.set_active_editor_first();
                }
            } else {
                if task.title.is_some() {
                    if let Some(dt) = &task.due_date {
                        self.next_task_time = dt.clone() + chrono::Duration::minutes(5);
                    }
                    self.editor.editors[1]
                        .set_content(&utils::format_datetime(self.next_task_time, false));
                    self.tasks.push(task);
                    self.tasks.sort_by_key(|t| {
                        t.due_date
                            .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap())
                    });
                    self.editor.editors[0].set_content("");
                    self.editor.set_active_editor_first();
                }
            }
        }
    }

    fn submit_field(&mut self) {
        if !self.editor.is_last_editor_active() {
            self.editor.set_active_editor_next();
            self.editor.set_mode(EditorMode::Normal);
        }
    }

    fn publish_all(&mut self, tx: &UnboundedSender<AppAction>) {
        for task in &self.tasks {
            let data = task.clone();
            let action = AppAction::TaskAction(crate::tasks::TaskAction::Create, data);
            let _ = tx.send(action);
        }
        self.tasks.clear();
        self.list_state.select(None);
        self.active_pane = ActivePane::Editor;
        self.editor.set_active_editor(Some(0));
    }

    pub fn handle_mouse_event(&mut self, _mouse_event: MouseEvent) {
        // Handle mouse events specific to Focus Mode here
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        paint_background(f);
        let main_area = centered_area(area, 36, 80);
        let areas = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(main_area);
        let editor_area = areas[0];
        if let Some((editor, _)) = self.editor.get_active_editor() {
            editor.validate();
            editor.update_style();
        }

        f.render_stateful_widget(&mut self.editor, editor_area, &mut self.editor_state);

        let list_area = areas[2];
        self.last_list_area = Some(list_area);
        // let list_area = Layout::horizontal([
        //     Constraint::Fill(1),
        //     Constraint::Max(80),
        //     Constraint::Fill(1),
        // ])
        // .split(areas[2])[1];
        let items: Vec<ListItem> = self
            .tasks
            .iter()
            .map(|task| create_list_item(task))
            .collect();
        let mut list = List::new(items)
            .highlight_style(Style::default().bold().bg(Color::Rgb(30, 30, 30)))
            .block(Block::bordered().border_type(BorderType::Rounded));
        if self.active_pane != ActivePane::TaskList {
            list = list.dim();
        }
        f.render_stateful_widget(list, list_area, &mut self.list_state.clone());

        let now = chrono::Local::now();
        let items_right: Vec<ListItem> = self
            .tasks
            .iter()
            .map(|task| create_list_item_right(now, task))
            .collect();
        let mut list_right = List::new(items_right)
            .highlight_style(Style::default().bold().bg(Color::Rgb(30, 30, 30)))
            .block(Block::bordered().border_type(BorderType::Rounded));
        if self.active_pane != ActivePane::TaskList {
            list_right = list_right.dim();
        }
        f.render_stateful_widget(list_right, list_area, &mut self.list_state);
        let elapsed = last_frame.elapsed();
        self.effects
            .process_effects(elapsed.into(), f.buffer_mut(), main_area);
    }
}

fn create_list_item(task: &TaskData) -> ListItem<'static> {
    let title = task.title.as_deref().unwrap_or("");
    // let line1 = Line::from("");
    let mut line2_text = format!(" {}", title);

    if let Some(repeat_str) = format_repeat_flag(&task.repeat_flag) {
        line2_text.push_str(&format!(" {}", repeat_str));
    }

    let line2 = Line::from(line2_text);
    let line3 = Line::from("");
    ListItem::from(vec![line2, line3])
}

fn create_list_item_right(now: DateTime<Local>, task: &TaskData) -> ListItem<'static> {
    let is_all_day =
        task.due_date.unwrap().time() == chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap();
    let line2 = if task.due_date.unwrap().timestamp() > 0 {
        let datetime_str = utils::format_datetime(task.due_date.unwrap(), is_all_day);
        let mut spans = vec![Span::from(datetime_str)];

        if let Some(repeat_str) = format_repeat_flag(&task.repeat_flag) {
            spans.push(Span::from(" • "));
            spans.push(Span::from(repeat_str));
        }
        spans.push(Span::from(" "));

        let now = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap()
            .with_timezone(&chrono::Utc);

        let mut line = Line::from(spans);
        if task.due_date.unwrap().timestamp() > 0 && task.due_date.unwrap() < now {
            line = line.style(Style::default().fg(Color::Red).dim());
        } else {
            line = line.style(Style::default().dim());
        }
        line.right_aligned()
    } else {
        Line::from(" ").right_aligned()
    };
    let line3 = Line::from("").right_aligned();
    ListItem::from(vec![line2, line3])
}
