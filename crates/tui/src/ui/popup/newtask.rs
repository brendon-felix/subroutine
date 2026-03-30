use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::{
    Frame,
    layout::{Alignment, Margin, Rect},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;
// use tui_textarea::Input;

use crate::{
    app::AppAction,
    // taskparser::{TaskParser, TokenType, timeutils},
    // tasks::{TaskAction, TaskData},
    ui::{
        UIAction,
        // editor::{
        //     Editor, EditorMode,
        //     actions::{EditorAction, EditorActions},
        //     handlers,
        // },
        popup::Popup,
        utils,
        viewselector::View,
    },
};
use chrono::Local;
// use ticks::projects::ProjectID;

pub struct NewTaskPopup {
    // editor: Editor,
    view: View,
    block: Block<'static>,
    tx: UnboundedSender<AppAction>,
    // parser: TaskParser,
}

impl NewTaskPopup {
    pub fn new(view: View, tx: UnboundedSender<AppAction>) -> Self {
        // let mut editor = Editor::new().with_single_line();
        // editor.set_mode(EditorMode::Insert);
        Self {
            // editor,
            view,
            block: Block::new()
                .title(" New Task ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
            tx,
            // parser: TaskParser::new(),
        }
    }

    fn submit(&mut self) {
        // let content = self.editor.get_content().trim().to_string();
        // if !content.is_empty() {
        //     let data = parse(&content, self.view.clone());
        //     let _ = self
        //         .tx
        //         .send(AppAction::TaskAction(TaskAction::Create, data));
        //     let _ = self.tx.send(AppAction::UIAction(UIAction::ClosePopup));
        // }
    }
}

impl Popup for NewTaskPopup {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc => {
                let _ = self.tx.send(AppAction::UIAction(UIAction::ClosePopup));
            }
            _ => {
                // let input: Input = key_event.into();
                // let mode = self.editor.get_mode();
                // let action_opt = if let Some(pending_action) = self.editor.get_pending_action() {
                //     match handlers::handle_pending_action_input(input, pending_action) {
                //         Some(action) => Some(action),
                //         None => {
                //             self.editor.set_pending_action(None);
                //             None
                //         }
                //     }
                // } else {
                //     handlers::handle_input(input, mode, true)
                // };
                // if let Some(action) = action_opt {
                //     match action {
                //         EditorAction::Submit => self.submit(),
                //         _ => self.editor.execute_action(action),
                //     }
                // }
            }
        }
    }

    fn handle_mouse_event(&mut self, _mouse_event: MouseEvent) {}

    fn allow_key_cmd(&self) -> bool {
        // self.editor.get_mode() == EditorMode::Normal
        true
    }

    fn draw(&mut self, f: &mut Frame, area: Rect, _last_frame: Instant) {
        // let popup_area = utils::centered_area_with_offset(area, 3, 60, 4);
        let popup_area = utils::centered_area(area, 3, 60);
        f.render_widget(Clear, popup_area);

        let inner_area = self.block.inner(popup_area).inner(Margin::new(1, 0));
        f.render_widget(&self.block, popup_area);
        // self.editor.update_style();
        // f.render_widget(&self.editor, inner_area);

        // // Render with syntax highlighting
        // let content = self.editor.get_content();
        // let spans = self.parser.highlighted_spans(&content);

        // let line = Line::from(spans);
        // let paragraph = Paragraph::new(line);
        // f.render_widget(paragraph, inner_area);
    }
}

// fn parse(content: &str, view: View) -> TaskData {
//     let parser = TaskParser::new();
//     let tokens = parser.parse(content);

//     let mut data = TaskData::default();
//     let mut title_parts = Vec::new();
//     let mut user_specified_date = false;
//     let mut user_specified_project = false;
//     let mut repeat_flag_parsed: Option<crate::tasks::RepeatFlag> = None;

//     for (token, _text) in tokens {
//         match token {
//             TokenType::DueDate(dt) => {
//                 // Convert from Local to UTC
//                 data.due_date = Some(dt.with_timezone(&chrono::Utc));
//                 user_specified_date = true;
//             }
//             TokenType::Priority(priority) => {
//                 data.priority = Some(priority);
//             }
//             TokenType::ProjectName(project_name) => {
//                 // You may need to map project names to ProjectIDs
//                 // For now, we'll store it as content or handle it differently
//                 // This might require access to the project list
//                 // Leaving as content for now
//                 if data.content.is_none() {
//                     data.content = Some(format!("Project: {}", project_name));
//                 } else {
//                     data.content = Some(format!(
//                         "{}\nProject: {}",
//                         data.content.as_ref().unwrap(),
//                         project_name
//                     ));
//                 }
//                 user_specified_project = true;
//             }
//             TokenType::Repeat(repeat_flag) => {
//                 // Store repeat flag for potential date inference
//                 repeat_flag_parsed = Some(repeat_flag.clone());
//                 // Use the build method to create the repeat flag string
//                 data.repeat_flag = Some(repeat_flag.build());
//             }
//             TokenType::Title(word) => {
//                 title_parts.push(word);
//             }
//         }
//     }

//     // Apply date inference logic
//     let now = Local::now();

//     // Check if we have a repeat pattern and can infer a date from it
//     let inferred_repeat_date = if let Some(ref repeat_flag) = repeat_flag_parsed {
//         timeutils::initial_occurrence(repeat_flag)
//     } else {
//         None
//     };

//     if !user_specified_date {
//         // No explicit date was specified
//         if let Some(inferred_date) = inferred_repeat_date {
//             // Use the date inferred from repeat pattern
//             data.due_date = Some(inferred_date.with_timezone(&chrono::Utc));
//         } else {
//             // Apply view-based defaults
//             let default_date = match view {
//                 View::Today => {
//                     let today = now.date_naive();
//                     Some(
//                         today
//                             .and_hms_opt(0, 0, 0)
//                             .unwrap()
//                             .and_local_timezone(Local)
//                             .unwrap()
//                             .with_timezone(&chrono::Utc),
//                     )
//                 }
//                 View::Tomorrow => {
//                     let tomorrow = (now + chrono::Duration::days(1)).date_naive();
//                     Some(
//                         tomorrow
//                             .and_hms_opt(0, 0, 0)
//                             .unwrap()
//                             .and_local_timezone(Local)
//                             .unwrap()
//                             .with_timezone(&chrono::Utc),
//                     )
//                 }
//                 View::Week => {
//                     let today = now.date_naive();
//                     Some(
//                         today
//                             .and_hms_opt(0, 0, 0)
//                             .unwrap()
//                             .and_local_timezone(Local)
//                             .unwrap()
//                             .with_timezone(&chrono::Utc),
//                     )
//                 }
//                 View::Inbox | View::All => None,
//             };
//             data.due_date = default_date;
//         }
//     } else {
//         // User specified a date/time, but check if it's time-only with a repeat pattern
//         if let (Some(due_date), Some(inferred_date)) = (data.due_date, inferred_repeat_date) {
//             let due_date_local = due_date.with_timezone(&Local);

//             // If the parsed date is "today" and we have a repeat pattern,
//             // it's likely a time-only input (e.g., "every wed 3pm")
//             if due_date_local.date_naive() == now.date_naive() && repeat_flag_parsed.is_some() {
//                 let time = due_date_local.time();
//                 data.due_date = Some(
//                     inferred_date
//                         .date_naive()
//                         .and_time(time)
//                         .and_local_timezone(Local)
//                         .unwrap()
//                         .with_timezone(&chrono::Utc),
//                 );
//             } else {
//                 // Check if it's a time-only parse for view-based context
//                 if due_date_local.date_naive() == now.date_naive() {
//                     let time = due_date_local.time();
//                     let base_date = match view {
//                         View::Tomorrow => (now + chrono::Duration::days(1)).date_naive(),
//                         _ => now.date_naive(),
//                     };

//                     data.due_date = Some(
//                         base_date
//                             .and_time(time)
//                             .and_local_timezone(Local)
//                             .unwrap()
//                             .with_timezone(&chrono::Utc),
//                     );
//                 }
//             }
//         }
//     }

//     // Apply view-based project defaults if user didn't specify
//     if !user_specified_project {
//         match view {
//             View::Inbox => {
//                 data.project_id = Some(ProjectID("inbox".to_string()));
//             }
//             _ => {
//                 // For other views, don't set a default project
//                 // (will use inbox by default in create_task)
//             }
//         }
//     }

//     // Build the title from all title tokens
//     let title = if title_parts.is_empty() {
//         content.to_string()
//     } else {
//         title_parts.join(" ")
//     };

//     data.title = Some(title);

//     data
// }
