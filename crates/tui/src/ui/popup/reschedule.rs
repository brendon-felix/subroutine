use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Margin, Rect},
    widgets::{Block, BorderType, Borders, Clear},
};
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::Input;

use crate::{
    app::AppAction,
    taskparser::{TaskParser, TokenType},
    tasks::{TaskAction, TaskData},
    ui::{
        UIAction,
        editor::{
            Editor, EditorMode,
            actions::{EditorAction, EditorActions},
            handlers,
        },
        popup::Popup,
        utils,
    },
};

pub struct ReschedulePopup {
    editor: Editor,
    selected_tasks: Vec<TaskData>,
    block: Block<'static>,
    tx: UnboundedSender<AppAction>,
}

impl ReschedulePopup {
    pub fn new(selected_tasks: Vec<TaskData>, tx: UnboundedSender<AppAction>) -> Self {
        let mut editor = Editor::new().with_single_line();
        editor.set_mode(EditorMode::Insert);
        Self {
            editor,
            selected_tasks,
            block: Block::new()
                .title(" Reschedule Tasks ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
            tx,
        }
    }

    fn submit(&mut self) {
        let content = self.editor.get_content().trim().to_string();
        if !content.is_empty() {
            if let Some(target) = parse_input(&content).ok() {
                // let _ = self.tx.send(AppAction::TaskAction(TaskAction::Reschedule));
                if let Some(earliest_due) = self
                    .selected_tasks
                    .iter()
                    .filter_map(|task| task.due_date)
                    .filter(|dt| dt.timestamp() > 0)
                    .min()
                {
                    self.selected_tasks
                        .iter()
                        .cloned()
                        .for_each(|mut task_data| {
                            task_data.due_date = match &target {
                                RescheduleTarget::RelativeToDueDate(duration) => {
                                    Some(task_data.due_date.unwrap() + *duration)
                                }
                                RescheduleTarget::AbsoluteTime(dt) => {
                                    let utc = dt.with_timezone(&chrono::Utc);
                                    let duration = utc - earliest_due;
                                    Some(task_data.due_date.unwrap() + duration)
                                }
                            };
                            let _ = self
                                .tx
                                .send(AppAction::TaskAction(TaskAction::Edit, task_data));
                        });
                    let _ = self.tx.send(AppAction::UIAction(UIAction::ClosePopup));
                }
            }
        }
    }
}

impl Popup for ReschedulePopup {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                let _ = self.tx.send(AppAction::UIAction(UIAction::ClosePopup));
            }
            _ => {
                let input: Input = key_event.into();
                let mode = self.editor.get_mode();
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
                        EditorAction::Submit => self.submit(),
                        _ => self.editor.execute_action(action),
                    }
                }
            }
        }
    }

    fn handle_mouse_event(&mut self, _mouse_event: MouseEvent) {}

    fn allow_key_cmd(&self) -> bool {
        self.editor.get_mode() == EditorMode::Normal
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, _last_frame: Instant) {
        // let popup_area = utils::centered_area_with_offset(area, 3, 60, 4);
        let popup_area = utils::centered_area(area, 3, 60);
        f.render_widget(Clear, popup_area);

        let inner_area = self.block.inner(popup_area).inner(Margin::new(1, 0));
        f.render_widget(&self.block, popup_area);
        self.editor.update_style();
        f.render_widget(&self.editor, inner_area);
    }
}

#[derive(Debug, Clone)]
pub enum RescheduleTarget {
    /// Relative to the task's original due datetime (e.g., "5min", "2 hours")
    RelativeToDueDate(chrono::Duration),
    /// Absolute time from now (e.g., "now", "now + 5min")
    AbsoluteTime(chrono::DateTime<chrono::Local>),
}

fn parse_input(duration_str: &str) -> Result<RescheduleTarget, String> {
    // First, try the legacy relative/absolute duration parsing for backward compatibility
    if let Ok(target) = parse_legacy_duration(duration_str) {
        return Ok(target);
    }

    // If legacy parsing fails, try full datetime parsing using TaskParser
    parse_datetime_with_taskparser(duration_str)
}

fn parse_legacy_duration(duration_str: &str) -> Result<RescheduleTarget, String> {
    let duration_str = duration_str.trim().to_lowercase();
    let is_absolute = duration_str.starts_with("now");
    if duration_str == "now" {
        return Ok(RescheduleTarget::AbsoluteTime(chrono::Local::now()));
    }

    let duration_part = if is_absolute {
        let after_now = duration_str.strip_prefix("now").unwrap().trim();
        after_now.strip_prefix("+").unwrap_or(after_now).trim()
    } else {
        &duration_str
    };

    let mut num_end = 0;
    for (i, ch) in duration_part.chars().enumerate() {
        if ch.is_ascii_digit() {
            num_end = i + 1;
        } else {
            break;
        }
    }

    if num_end == 0 {
        return Err(format!(
            "Invalid legacy duration format. Expected number followed by unit (e.g., '5min', '2 hours')"
        ));
    }

    let num_str = &duration_part[..num_end];
    let unit_str = duration_part[num_end..].trim();

    let value = num_str
        .parse::<i64>()
        .map_err(|_| format!("Invalid duration value: {}", num_str))?;

    let duration = match unit_str {
        "min" | "minute" | "minutes" => chrono::Duration::minutes(value),
        "hr" | "hour" | "hours" | "h" => chrono::Duration::hours(value),
        "day" | "days" | "d" => chrono::Duration::days(value),
        _ => {
            return Err(format!(
                "Invalid duration unit: '{}'. Use 'min', 'minutes', 'hr', 'hours', 'day', or 'days'",
                unit_str
            ));
        }
    };

    if is_absolute {
        Ok(RescheduleTarget::AbsoluteTime(
            chrono::Local::now() + duration,
        ))
    } else {
        Ok(RescheduleTarget::RelativeToDueDate(duration))
    }
}

fn parse_datetime_with_taskparser(input: &str) -> Result<RescheduleTarget, String> {
    let parser = TaskParser::new();
    let tokens = parser.parse(input);

    for (token, _text) in tokens {
        match token {
            TokenType::DueDate(dt) => {
                return Ok(RescheduleTarget::AbsoluteTime(dt));
            }
            _ => continue,
        }
    }

    // If no date was parsed, return an error
    Err(format!(
        "Could not parse '{}' as a valid date/time or duration. \
        Supported formats include: \
        - Legacy: '5min', '2 hours', 'now', 'now + 30min' \
        - Absolute dates: 'tomorrow', 'dec 12', 'next friday', '11/23' \
        - Times: '4pm', 'tomorrow 5pm', 'dec 12 3:30pm' \
        - And many other natural language date formats",
        input
    ))
}
