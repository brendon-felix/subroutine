pub mod actions;
pub mod handlers;
pub mod helpers;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Widget, WidgetRef},
};
use std::fmt;
use tui_textarea::{CursorMove, TextArea};

use actions::EditorPendingAction;

use crate::ui::editor::{
    actions::{EditorAction, EditorActions},
    helpers::{cursor_style, select_current_line, select_current_paragraph, select_current_word},
};

#[derive(Debug, Clone, Copy)]
pub enum TextObject {
    Char,
    WordInner,
    WordAround,
    Line,
    ParagraphInner,
    ParagraphAround,
    Selection,
    To(CursorMove),
}

#[derive(Debug, Clone, Copy)]
pub enum TextObjectModifier {
    Inner,
    Around,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualMode {
    Char,
    Line,
    // Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    Normal,
    Insert,
    Replace,
    Visual(VisualMode),
}

impl fmt::Display for EditorMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Self::Normal => write!(f, "NORMAL"),
            Self::Insert => write!(f, "INSERT"),
            Self::Replace => write!(f, "REPLACE"),
            Self::Visual(_) => write!(f, "VISUAL"),
        }
    }
}

pub struct EditorState {
    mode: EditorMode,
    pending_action: Option<EditorPendingAction>,
    yank_type: Option<TextObject>,
    is_active: bool,
    valid_state: Option<bool>,
}

// pub enum EditorStyle {
//     Normal,
//     Valid,
//     Invalid,
// }

pub struct Editor {
    // title: Option<String>,
    state: EditorState,
    textarea: TextArea<'static>,
    single_line: bool,
    validator: Option<Box<dyn Fn(&str) -> Option<bool>>>,
    style_overridden: bool,
    cursor_style_overridden: bool,
    border_style_overridden: bool,
}

impl Editor {
    pub fn new() -> Self {
        let state = EditorState {
            mode: EditorMode::Normal,
            pending_action: None,
            yank_type: None,
            is_active: true,
            valid_state: None,
        };
        let mut textarea = TextArea::default();
        textarea.set_selection_style(Style::default().add_modifier(Modifier::REVERSED));
        // textarea.set_cursor_line_style(Style::default().bg(Color::Rgb(50, 50, 50)));
        textarea.set_cursor_line_style(Style::default());
        let mut editor = Self {
            state,
            textarea,
            single_line: false,
            validator: None,
            style_overridden: false,
            cursor_style_overridden: false,
            border_style_overridden: false,
        };
        editor.update_style();
        editor
    }

    pub fn with_single_line(mut self) -> Self {
        self.single_line = true;
        self
    }

    pub fn with_validator<F>(mut self, validator: F) -> Self
    where
        F: 'static + Fn(&str) -> Option<bool>,
    {
        self.validator = Some(Box::new(validator));
        self
    }

    pub fn with_content(mut self, content: &str) -> Self {
        self.textarea.insert_str(content);
        self
    }

    pub fn with_placeholder(mut self, placeholder: &str) -> Self {
        self.textarea.set_placeholder_text(placeholder);
        self
    }

    pub fn with_block(mut self, block: Block<'static>) -> Self {
        self.textarea.set_block(block);
        self
    }

    pub fn with_overridden_style(mut self, style: Style) -> Self {
        self.textarea.set_style(style);
        self.style_overridden = true;
        self
    }

    pub fn with_overridden_cursor_style(mut self, style: Style) -> Self {
        self.textarea.set_cursor_style(style);
        self.cursor_style_overridden = true;
        self
    }

    pub fn with_overridden_border_style(mut self, style: Style) -> Self {
        if let Some(mut block) = self.textarea.block().cloned() {
            block = block.border_style(style);
            self.textarea.set_block(block);
            self.border_style_overridden = true;
        }
        self
    }

    pub fn set_content(&mut self, content: &str) {
        self.textarea.reset_with_content(content);
    }

    pub fn set_block(&mut self, block: Block<'static>) {
        self.textarea.set_block(block);
    }

    pub fn set_style(&mut self, style: Style) {
        self.textarea.set_style(style);
    }

    pub fn override_style(&mut self, style: Style) {
        self.textarea.set_style(style);
        self.style_overridden = true;
    }

    pub fn clear_style_override(&mut self) {
        self.style_overridden = false;
    }

    pub fn override_cursor_style(&mut self, style: Style) {
        self.textarea.set_cursor_style(style);
        self.cursor_style_overridden = true;
    }

    pub fn clear_cursor_style_override(&mut self) {
        self.cursor_style_overridden = false;
    }

    pub fn override_border_style(&mut self, style: Style) {
        if let Some(mut block) = self.textarea.block().cloned() {
            block = block.border_style(style);
            self.textarea.set_block(block);
            self.border_style_overridden = true;
        }
    }

    pub fn clear_border_style_override(&mut self) {
        self.border_style_overridden = false;
    }

    pub fn set_cursor_style(&mut self, style: Style) {
        self.textarea.set_cursor_style(style);
    }

    pub fn set_cursor_line_style(&mut self, style: Style) {
        self.textarea.set_cursor_line_style(style);
    }

    pub fn set_desired_column(&mut self, col: usize) {
        self.textarea.set_desired_column(col);
    }

    pub fn set_cursor_pos(mut self, row: u16, col: u16) -> Self {
        self.textarea.move_cursor(CursorMove::Jump(row, col));
        self
    }

    pub fn set_active(&mut self, active: bool) {
        self.state.is_active = active;
    }

    pub fn update_style(&mut self) {
        if self.style_overridden {
            return;
        }
        if !self.cursor_style_overridden {
            self.set_cursor_style(cursor_style(self.get_mode(), self.state.is_active));
        }
        let style = if self.state.is_active {
            self.textarea.style().remove_modifier(Modifier::DIM)
        } else {
            self.textarea.style().add_modifier(Modifier::DIM)
        };
        self.textarea.set_style(style);

        if let Some(block) = self.textarea.block().cloned() {
            if !self.border_style_overridden {
                let mut style = if let Some(valid_state) = self.state.valid_state {
                    if valid_state {
                        // Style::default().fg(ratatui::style::Color::LightGreen)
                        Style::default()
                    } else {
                        Style::default().fg(ratatui::style::Color::LightRed)
                    }
                } else {
                    Style::default()
                };
                if !self.state.is_active {
                    style = style.add_modifier(Modifier::DIM);
                };
                let block = block.border_style(style);
                self.textarea.set_block(block);
            }
        }
    }

    pub fn insert_str(&mut self, s: String) {
        let _ = self.textarea.insert_str(s);
    }

    pub fn is_single_line(&self) -> bool {
        self.single_line
    }

    pub fn is_valid(&self) -> bool {
        self.state.valid_state.unwrap_or(true)
    }

    pub fn get_content(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub fn set_mode(&mut self, mode: EditorMode) {
        self.execute_action(EditorAction::SetMode(mode));
    }

    pub fn get_mode(&self) -> EditorMode {
        self.state.mode
    }

    pub fn get_cursor_pos(&self) -> (usize, usize) {
        self.textarea.cursor()
    }

    pub fn get_desired_column(&self) -> usize {
        self.textarea.get_desired_column()
    }

    pub fn get_lines(&self) -> &[String] {
        self.textarea.lines()
    }

    pub fn clone_content(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub fn get_validation_state(&self) -> Option<bool> {
        self.state.valid_state
    }

    pub fn is_cursor_at_line_end(&self) -> bool {
        let (row, col) = self.textarea.cursor();
        if let Some(line) = self.textarea.lines().get(row) {
            col >= line.len()
        } else {
            false
        }
    }

    pub fn is_cursor_at_line_start(&self) -> bool {
        let (_row, col) = self.textarea.cursor();
        col == 0
    }

    pub fn is_cursor_on_first_line(&self) -> bool {
        let (row, _col) = self.textarea.cursor();
        row == 0
    }

    pub fn is_cursor_on_last_line(&self) -> bool {
        let (row, _col) = self.textarea.cursor();
        row + 1 == self.textarea.lines().len()
    }

    pub fn validate(&mut self) {
        if let Some(validator) = &self.validator {
            self.state.valid_state = validator(&self.textarea.lines()[0]);
        }
    }
}

impl EditorActions for Editor {
    #[rustfmt::skip]
    fn execute_action(&mut self, action: EditorAction) {
        let mut pending = false;
        match action {
            EditorAction::SetMode(mode) => {
                match mode {
                    EditorMode::Normal => {
                        self.textarea.cancel_selection();
                        // self.validate();
                    }
                    EditorMode::Visual(vmode) => {
                        match vmode {
                            VisualMode::Char =>  self.textarea.start_selection(),
                            VisualMode::Line => self.textarea.start_line_selection(),
                        }
                    }
                    EditorMode::Insert | EditorMode::Replace => {}
                }
                self.state.mode = mode;
                // self.update_style();
            }
            EditorAction::MoveCursor(mvmt) => {
                match mvmt {
                    _ => self.textarea.move_cursor(mvmt),
                }
            }
            EditorAction::Insert(obj) => {
                match obj {
                    TextObject::Line => {
                        if self.single_line {
                            return;
                        }
                        let _ = self.textarea.insert_newline();
                    }
                    _ => {}
                }
            }
            EditorAction::ApplyInput(input) => {
                if self.single_line {
                    match input.key {
                        tui_textarea::Key::Enter => return,
                        _ => {}
                    }
                }
                let _ = self.textarea.input_without_shortcuts(input);
            }
            EditorAction::Delete(obj) => {
                self.state.yank_type = Some(obj);
                match obj {
                    TextObject::Char => {
                        self.textarea.start_selection();
                        self.textarea.move_cursor(CursorMove::Forward);
                        self.textarea.cut();
                    }
                    TextObject::WordInner => {
                        let _ = select_current_word(&mut self.textarea, TextObjectModifier::Inner);
                        self.textarea.cut();
                    }
                    TextObject::WordAround => {
                        let _ = select_current_word(&mut self.textarea, TextObjectModifier::Around);
                        self.textarea.cut();
                    }
                    TextObject::Line => {
                        let (current_row, current_col) = select_current_line(&mut self.textarea);
                        self.textarea.cut();
                        self.textarea.move_cursor(CursorMove::Jump(current_row as u16, current_col as u16));
                    }
                    TextObject::Selection => {
                        self.textarea.cut();
                    }
                    TextObject::To(mvmt) => {
                        self.textarea.start_selection();
                        self.textarea.move_cursor(mvmt);
                        self.textarea.cut();
                    }
                    _ => {}
                }
            },
            EditorAction::Select(obj) => {
                match obj {
                    TextObject::WordInner => {
                        let _ = select_current_word(&mut self.textarea, TextObjectModifier::Inner);
                    }
                    TextObject::WordAround => {
                        let _ = select_current_word(&mut self.textarea, TextObjectModifier::Around);
                    }
                    TextObject::ParagraphInner => {
                        let _ = select_current_paragraph(&mut self.textarea, TextObjectModifier::Inner);
                    }
                    TextObject::ParagraphAround => {
                        let _ = select_current_paragraph(&mut self.textarea, TextObjectModifier::Inner);
                    }
                    _ => {}
                }
                self.state.mode = EditorMode::Visual(VisualMode::Char);
                self.textarea
                    .set_cursor_style(cursor_style(self.state.mode, true));
            },
            EditorAction::Yank(obj) => {
                self.state.yank_type = Some(obj);
                match obj {
                    TextObject::Line => {
                        let (current_row, current_col) = select_current_line(&mut self.textarea);
                        self.textarea.copy();
                        self.textarea.move_cursor(CursorMove::Jump(current_row as u16, current_col as u16));
                    }
                    TextObject::Selection => {
                        if self.state.mode == EditorMode::Visual(VisualMode::Line) {
                            self.state.yank_type = Some(TextObject::Line);
                        }
                        self.textarea.copy();
                    }
                    _ => {}
                }
            }
            EditorAction::ReplaceChar(c) => {
                self.textarea.start_selection();
                self.textarea.move_cursor(CursorMove::Forward);
                self.textarea.set_yank_text(c);
                self.textarea.insert_char(c);
                self.textarea.move_cursor(CursorMove::Back);
            }
            // EditorAction::Copy => {
            //     self.textarea.copy();
            // }
            EditorAction::Paste => {
                match self.state.yank_type {
                    Some(TextObject::Line) => {
                        if self.single_line {
                            return;
                        }
                        let yanked = &self.textarea.yank_text();
                        self.textarea.set_yank_text(yanked.trim_end());
                        self.textarea.move_cursor(CursorMove::End);
                        let _ = self.textarea.insert_newline();
                        self.textarea.paste();
                        self.textarea.move_cursor(CursorMove::Head);
                    }
                    _ => {
                        self.textarea.move_cursor(CursorMove::Forward);
                        self.textarea.paste();
                    }
                }
            }
            EditorAction::Undo => {
                self.textarea.undo();
            }
            EditorAction::Redo => {
                self.textarea.redo();
            }
            // EditorAction::Command(cmd) => match cmd {
            //     EditorCommand::Submit => {}
            // },
            EditorAction::Pending(p) => {
                pending = true;
                self.state.pending_action = Some(p);
            }
            EditorAction::MultiAction(actions) => {
                for act in actions {
                    self.execute_action(act);
                }
            }
            EditorAction::Submit => {
                // handled externally
            }
        }

        if !pending {
            self.state.pending_action = None;
        }
    }
    fn set_pending_action(&mut self, pending: Option<EditorPendingAction>) {
        self.state.pending_action = pending;
    }
    fn get_pending_action(&mut self) -> Option<EditorPendingAction> {
        self.state.pending_action
    }
}

impl Widget for Editor {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.textarea.render(area, buf);
    }
}

impl Widget for &mut Editor {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.textarea.render(area, buf);
    }
}

impl WidgetRef for Editor {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        self.textarea.clone().render(area, buf);
    }
}
