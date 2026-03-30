use std::time::Instant;

use chrono::{Local, TimeZone, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders},
};
use tachyonfx::{
    EffectManager, EffectTimer, Interpolation,
    fx::{self},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    app::AppAction,
    ui::{
        UIAction,
        composite::{CompositeEditor, CompositeEditorState},
        editor::{
            Editor, EditorMode,
            actions::{EditorAction, EditorActions},
            handlers,
        },
        normalmode::NormalModeAction,
        utils,
    },
};

// const SAMPLE_DESCRIPTION: &str = r#"This is a description.
// You can write multiple lines here,
// or edit the content as needed.

// This is another paragraph to demonstrate the editor functionality.
// Next we have a line that is really long ..."#;

fn validate_duedate(input: &str) -> Option<bool> {
    if input.trim().is_empty() {
        None
    } else {
        Some(utils::parse_datetime(input).is_ok())
    }
}

fn validate_priority(input: &str) -> Option<bool> {
    if input.trim().is_empty() {
        None
    } else {
        Some(parse_priority(input).is_some())
    }
}

fn splits_fn(area: Rect) -> Vec<Rect> {
    let primary_splits = Layout::new(
        Direction::Vertical,
        [
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
        ],
    )
    .split(area);
    let due_date_priority_split = Layout::new(
        Direction::Horizontal,
        [Constraint::Percentage(70), Constraint::Percentage(30)],
    )
    .split(primary_splits[1]);
    vec![
        primary_splits[0],
        due_date_priority_split[0],
        due_date_priority_split[1],
        primary_splits[2],
    ]
}

fn new_block(title: &'static str) -> Block<'static> {
    Block::new().borders(Borders::ALL).title(title)
}

pub struct TaskEditor {
    editor: CompositeEditor,
    editor_state: CompositeEditorState,
    pub unsaved_changes: bool,
    effects: EffectManager<()>,
    tx: UnboundedSender<AppAction>,
}

impl TaskEditor {
    pub fn new(tx: UnboundedSender<AppAction>) -> Self {
        let editors = vec![
            Editor::new()
                .with_single_line()
                .with_block(new_block("Title")),
            Editor::new()
                .with_single_line()
                .with_block(new_block("Due"))
                .with_validator(validate_duedate),
            Editor::new()
                .with_single_line()
                .with_block(new_block("Priority"))
                .with_validator(validate_priority),
            Editor::new().with_block(new_block("Description")),
        ];
        let editor_state = CompositeEditorState::new(editors.len(), Box::new(splits_fn));
        // let editor = CompositeEditor::new(editors).with_constraints(vec![
        //     Constraint::Length(3),
        //     Constraint::Length(3),
        //     Constraint::Length(3),
        //     Constraint::Min(3),
        // ]);
        let editor = CompositeEditor::new(editors);
        let effects: EffectManager<()> = EffectManager::default();
        // let c = Color::Rgb(25, 25, 25);
        // let timer = EffectTimer::from_ms(20000, Interpolation::ElasticOut);
        // let fx = fx::sweep_in(Motion::LeftToRight, 5, 0, c, timer);
        // effects.add_effect(fx);
        Self {
            editor,
            editor_state,
            unsaved_changes: false,
            effects,
            tx,
        }
    }

    // pub fn with_initial_mode(mut self, mode: EditorMode) -> Self {
    //     self.editor.set_mode(mode);
    //     self
    // }

    pub fn deactivate(&mut self) {
        self.editor.set_pending_action(None);
        self.editor.set_active_editor(None);
    }

    pub fn activate(&mut self) {
        self.editor.set_active_editor(Some(0));
    }

    // pub fn set_mode(&mut self, mode: EditorMode) {
    //     self.editor.set_mode(mode);
    // }

    pub fn set_title_content(&mut self, title: &str) {
        self.editor.editors[0].set_content(title);
    }

    pub fn set_due_date_content(&mut self, due_date: &str) {
        self.editor.editors[1].set_content(due_date);
    }

    pub fn set_priority_content(&mut self, due_date: &str) {
        self.editor.editors[2].set_content(due_date);
    }

    pub fn set_priority_color(&mut self, color: Color) {
        self.editor.editors[2].set_style(Style::default().fg(color));
        self.editor.editors[2].override_border_style(Style::default().fg(color));
    }

    pub fn set_description_content(&mut self, title: &str) {
        self.editor.editors[3].set_content(title);
    }

    pub fn clear_all_fields(&mut self) {
        self.editor.clear_all();
        // Reset priority color to default
        self.editor.editors[2].set_style(Style::default());
        self.editor.editors[2].override_border_style(Style::default());
        self.unsaved_changes = false;
    }

    pub fn load_task(&mut self, task: &Task) {
        self.set_title_content(&task.title);
        if task.due_date.timestamp() > 0 {
            let due_date_str = utils::format_datetime(task.due_date, task.is_all_day);
            self.set_due_date_content(&due_date_str);
        } else {
            self.set_due_date_content("");
        }
        match task.priority {
            TaskPriority::High => {
                self.set_priority_content("P1");
                self.set_priority_color(Color::LightRed);
            }
            TaskPriority::Medium => {
                self.set_priority_content("P2");
                self.set_priority_color(Color::LightYellow);
            }
            TaskPriority::Low => {
                self.set_priority_content("P3");
                self.set_priority_color(Color::LightBlue);
            }
            TaskPriority::None => {
                self.set_priority_content("");
                self.editor.editors[2].set_style(Style::default());
                self.editor.editors[2].override_border_style(Style::default());
            }
        }
        self.set_description_content(&task.content);
        self.editor_state
            .get_sub_areas()
            .iter()
            .map(|area| {
                let inner = Block::default().borders(Borders::ALL).inner(area.clone());
                // let bg = Color::Rgb(25, 25, 25);
                let timer = EffectTimer::from_ms(200, Interpolation::Linear);
                fx::coalesce(timer).with_area(inner)
                // fx::sweep_in(Motion::RightToLeft, 5, 0, bg, timer).with_area(inner)
            })
            .for_each(|fx| self.effects.add_effect(fx));
        self.unsaved_changes = false;
    }

    // pub fn has_unsaved_changes(&self) -> bool {
    //     self.unsaved_changes
    // }

    // pub fn discard_changes(&mut self) {
    //     self.unsaved_changes = false;
    // }

    fn parse_inputs(&self) -> TaskData {
        let title = self.editor.editors[0].get_content().trim().to_string();
        let due_date_input = self.editor.editors[1].get_content().trim().to_string();
        let priority_input = self.editor.editors[2].get_content().trim().to_string();
        let description = self.editor.editors[3].get_content().trim().to_string();

        let mut data = TaskData::default().title(title);

        if !due_date_input.is_empty() {
            if let Ok(dt) = utils::parse_datetime(&due_date_input) {
                // Convert NaiveDateTime to DateTime<Utc> by treating it as local time first
                let dt_utc = Local.from_local_datetime(&dt).unwrap().with_timezone(&Utc);
                data = data.due_date(dt_utc);
            }
        }

        if !priority_input.is_empty() {
            if let Some(priority) = parse_priority(&priority_input) {
                data = data.priority(priority);
            }
        }

        if !description.is_empty() {
            data = data.content(description);
        }

        data
    }

    pub fn is_in_insert_mode(&self) -> bool {
        if let Some(mode) = self.editor.get_mode() {
            match mode {
                EditorMode::Insert | EditorMode::Visual(_) => true,
                _ => false,
            }
        } else {
            false
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Enter
                if key_event.modifiers == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) =>
            {
                self.submit();
                return;
            }
            _ => {}
        }
        let input: Input = key_event.into();
        // if let Input {
        //     key: Key::Enter,
        //     ctrl: true,
        //     alt: false,
        //     shift: false,
        // } = input
        // {
        //     return;
        // }
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
                let is_single_line = self
                    .editor
                    .get_active_editor()
                    .map(|(e, _)| e.is_single_line())
                    .unwrap_or(false);

                // match input {
                //     Input {
                //         key: Key::Enter, ..
                //     } if self.is_in_insert_mode() && is_single_line => {
                //         self.editor.set_active_editor_next();
                //     }
                //     _ => handle_input(input, mode, is_single_line),
                // }
                handlers::handle_input(input, mode, is_single_line)
            };
            match action_opt {
                Some(action) => match action {
                    EditorAction::Submit if self.editor.is_last_editor_active() => self.submit(),
                    EditorAction::Submit => self.submit_field(),
                    EditorAction::ApplyInput(_) => {
                        self.editor.execute_action(action);
                    }
                    _ => self.editor.execute_action(action),
                },
                None => {}
            }
        }
    }

    // pub fn validate_active(&mut self) -> bool {
    //     if let Some((editor, _)) = self.editor.get_active_editor() {
    //         editor.validate()
    //     } else {
    //         true
    //     }
    // }

    pub fn submit(&mut self) {
        let data = self.parse_inputs();
        let action = AppAction::MultiAction(vec![
            AppAction::TaskAction(TaskAction::Create, data),
            AppAction::UIAction(UIAction::NormalMode(NormalModeAction::ExitTaskEditor)),
        ]);
        let _ = self.tx.send(action);
    }

    pub fn submit_field(&mut self) {
        // if !self.editor.is_last_editor_active() {
        //     self.editor.set_active_editor_next();
        //     if let Some(mode) = mode_before {
        //         self.editor.set_mode(mode);
        //     }
        // }

        if let Some((editor, _)) = self.editor.get_active_editor() {
            let mode_before = editor.get_mode();
            // editor.validate();
            editor.update_style();
            if !self.editor.is_last_editor_active() {
                self.editor.set_active_editor_next();
                self.editor.set_mode(mode_before);
            }
        }
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, last_frame: Instant) {
        if let Some((editor, _)) = self.editor.get_active_editor() {
            editor.validate();
            editor.update_style();
        }

        f.render_stateful_widget(&mut self.editor, area, &mut self.editor_state);

        let elapsed = last_frame.elapsed();
        self.effects
            .process_effects(elapsed.into(), f.buffer_mut(), area);
    }
}
