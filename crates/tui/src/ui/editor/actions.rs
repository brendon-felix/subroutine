// use tui_textarea::{CursorMove, Input};

use super::{EditorMode, TextObject, TextObjectModifier};

#[derive(Debug, Clone, Copy)]
pub enum EditorPendingAction {
    Delete(Option<TextObjectModifier>),
    Change(Option<TextObjectModifier>),
    Select(Option<TextObjectModifier>),
    Yank(Option<TextObjectModifier>),
    ReplaceChar,
    // Command(Option<EditorCommand>),
}

#[derive(Debug, Clone)]
pub enum EditorAction {
    SetMode(EditorMode),
    MoveCursor(CursorMove),
    Insert(TextObject),
    ApplyInput(Input),
    Delete(TextObject),
    Select(TextObject),
    Yank(TextObject),
    ReplaceChar(char),
    // Copy,
    Paste,
    Undo,
    Redo,
    // Command(EditorCommand),
    Pending(EditorPendingAction),
    MultiAction(Vec<EditorAction>),
    // App
    Submit,
}

pub trait EditorActions {
    fn set_pending_action(&mut self, pending: Option<EditorPendingAction>);
    fn get_pending_action(&mut self) -> Option<EditorPendingAction>;
    fn execute_action(&mut self, action: EditorAction);
}
