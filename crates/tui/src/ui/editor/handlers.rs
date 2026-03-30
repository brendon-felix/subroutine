// use tui_textarea::{CursorMove, Input, Key};

use crate::ui::editor::{
    actions::{EditorAction, EditorPendingAction},
    helpers::{is_movement_key, match_movement_key},
};

use super::{EditorMode, TextObject, TextObjectModifier, VisualMode};

pub fn handle_input(input: Input, mode: EditorMode, is_single_line: bool) -> Option<EditorAction> {
    match mode {
        EditorMode::Normal => handle_normal_mode_input(input, is_single_line),
        EditorMode::Insert => handle_insert_mode_input(input, is_single_line),
        EditorMode::Visual(vmode) => handle_visual_mode_input(input, vmode, is_single_line),
        EditorMode::Replace => handle_replace_mode_input(input, is_single_line),
    }
}

fn handle_normal_mode_input(input: Input, is_single_line: bool) -> Option<EditorAction> {
    match input {
        // Enter insert mode
        Input {
            key: Key::Char('i'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::SetMode(EditorMode::Insert)),
        Input {
            key: Key::Char('I'),
            ctrl: false,
            alt: false,
            ..
        } => Some(EditorAction::MultiAction(vec![
            EditorAction::MoveCursor(CursorMove::Head),
            EditorAction::SetMode(EditorMode::Insert),
        ])),
        Input {
            key: Key::Char('a'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::MultiAction(vec![
            EditorAction::MoveCursor(CursorMove::Right),
            EditorAction::SetMode(EditorMode::Insert),
        ])),
        Input {
            key: Key::Char('A'),
            ctrl: false,
            alt: false,
            ..
        } => Some(EditorAction::MultiAction(vec![
            EditorAction::MoveCursor(CursorMove::End),
            EditorAction::SetMode(EditorMode::Insert),
        ])),
        Input {
            key: Key::Char('o'),
            ctrl: false,
            alt: false,
            shift: false,
        } if is_single_line => Some(EditorAction::MultiAction(vec![
            EditorAction::MoveCursor(CursorMove::Down),
            EditorAction::MoveCursor(CursorMove::End),
            EditorAction::SetMode(EditorMode::Insert),
        ])),
        Input {
            key: Key::Char('o'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::MultiAction(vec![
            EditorAction::MoveCursor(CursorMove::End),
            EditorAction::Insert(TextObject::Line),
            EditorAction::SetMode(EditorMode::Insert),
        ])),
        Input {
            key: Key::Char('O'),
            ctrl: false,
            alt: false,
            ..
        } => Some(EditorAction::MultiAction(vec![
            EditorAction::MoveCursor(CursorMove::Up),
            EditorAction::MoveCursor(CursorMove::End),
            EditorAction::Insert(TextObject::Line),
            EditorAction::SetMode(EditorMode::Insert),
        ])),
        Input {
            key: Key::Enter,
            ctrl: false,
            alt: false,
            ..
        } => Some(EditorAction::MultiAction(vec![
            EditorAction::MoveCursor(CursorMove::Down),
            EditorAction::MoveCursor(CursorMove::Head),
        ])),
        Input {
            key: Key::Enter, ..
        } if is_single_line => Some(EditorAction::Submit),
        Input {
            key: Key::Enter,
            ctrl: true,
            ..
        } => Some(EditorAction::Submit),

        // Normal mode editing commands
        Input {
            key: Key::Char('x'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::Delete(TextObject::Char)),
        Input {
            key: Key::Char('d'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::Pending(EditorPendingAction::Delete(None))),
        Input {
            key: Key::Char('c'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::Pending(EditorPendingAction::Change(None))),
        Input {
            key: Key::Char('y'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::Pending(EditorPendingAction::Yank(None))),
        Input {
            key: Key::Char('p'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::Paste),
        Input {
            key: Key::Char('u'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::Undo),
        Input {
            key: Key::Char('r'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::Pending(EditorPendingAction::ReplaceChar)),
        Input {
            key: Key::Char('R'),
            ctrl: false,
            alt: false,
            ..
        } => Some(EditorAction::SetMode(EditorMode::Replace)),
        Input {
            key: Key::Char('r'),
            ctrl: true,
            alt: false,
            shift: false,
        } => Some(EditorAction::Redo),
        Input {
            key: Key::Char('s'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::Pending(EditorPendingAction::Select(None))),
        Input {
            key: Key::Char('v'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(EditorAction::Pending(EditorPendingAction::Select(None))),
        Input {
            key: Key::Char('V'),
            ctrl: false,
            alt: false,
            ..
        } => Some(EditorAction::SetMode(EditorMode::Visual(VisualMode::Line))),

        // Cursor movements
        Input { .. } if is_movement_key(&input) => {
            match_movement_key(&input).map(|mvmt| EditorAction::MoveCursor(mvmt))
        }
        _ => None,
    }
}

#[rustfmt::skip]
pub fn handle_pending_action_input(input: Input, pending: EditorPendingAction) -> Option<EditorAction> {
    match pending {
        EditorPendingAction::Delete(None) => match input {
            Input {
                key: Key::Char('i'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::Pending(EditorPendingAction::Delete(Some(
                TextObjectModifier::Inner,
            )))),
            Input {
                key: Key::Char('a'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::Pending(EditorPendingAction::Delete(Some(TextObjectModifier::Around)))),
            Input {
                key: Key::Char('d'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::Delete(TextObject::Line)),
            Input { .. } if is_movement_key(&input) => {
                match_movement_key(&input).map(|mvmt| EditorAction::Delete(TextObject::To(mvmt)))
            }
            _ => None,
        },
        EditorPendingAction::Delete(Some(modifier)) => match input {
            Input {
                key: Key::Char('w'),
                ctrl: false,
                alt: false,
                shift: false,
            } => match modifier {
                TextObjectModifier::Inner => Some(EditorAction::Delete(TextObject::WordInner)),
                TextObjectModifier::Around => Some(EditorAction::Delete(TextObject::WordAround)),
            },
            Input {
                key: Key::Char('p'),
                ctrl: false,
                alt: false,
                shift: false,
            } => match modifier {
                TextObjectModifier::Inner => Some(EditorAction::Delete(TextObject::ParagraphInner)),
                TextObjectModifier::Around => Some(EditorAction::Delete(TextObject::ParagraphAround)),
            },
            _ => None,
        },
        EditorPendingAction::Select(None) => match input {
            Input {
                key: Key::Char('i'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::Pending(EditorPendingAction::Select(Some(
                TextObjectModifier::Inner,
            )))),
            Input {
                key: Key::Char('a'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::Pending(EditorPendingAction::Select(Some(TextObjectModifier::Around)))),
            Input { .. } if is_movement_key(&input) => {
                match_movement_key(&input).map(|mvmt|
                    match mvmt {
                        CursorMove::Back | CursorMove::Left => EditorAction::MultiAction(vec![
                            EditorAction::MoveCursor(CursorMove::Right),
                            EditorAction::SetMode(EditorMode::Visual(VisualMode::Char)),
                            EditorAction::MoveCursor(CursorMove::Left),
                            EditorAction::MoveCursor(mvmt),
                        ]),
                        _ => EditorAction::MultiAction(vec![
                            EditorAction::SetMode(EditorMode::Visual(VisualMode::Char)),
                            EditorAction::MoveCursor(mvmt),
                        ])
                    }
                )
            }
            _ => None,
        },
        EditorPendingAction::Select(Some(modifier)) => match input {
            Input {
                key: Key::Char('w'),
                ctrl: false,
                alt: false,
                shift: false,
            } => match modifier {
                TextObjectModifier::Inner => Some(EditorAction::Select(TextObject::WordInner)),
                TextObjectModifier::Around => Some(EditorAction::Select(TextObject::WordAround)),
            },
            Input {
                key: Key::Char('p'),
                ctrl: false,
                alt: false,
                shift: false,
            } => match modifier {
                TextObjectModifier::Inner => Some(EditorAction::Select(TextObject::ParagraphInner)),
                TextObjectModifier::Around => Some(EditorAction::Select(TextObject::ParagraphAround)),
            },
            _ => None,
        },
        EditorPendingAction::Yank(None) => match input {
            Input {
                key: Key::Char('i'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::Pending(EditorPendingAction::Yank(Some(
                TextObjectModifier::Inner,
            )))),
            Input {
                key: Key::Char('a'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::Pending(EditorPendingAction::Yank(Some(TextObjectModifier::Around)))),
            Input {
                key: Key::Char('y'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::Yank(TextObject::Line)),
            Input { .. } if is_movement_key(&input) => {
                match_movement_key(&input).map(|mvmt| EditorAction::Yank(TextObject::To(mvmt)))
            }
            _ => None,
        },
        EditorPendingAction::Yank(Some(modifier)) => match input {
            Input {
                key: Key::Char('w'),
                ctrl: false,
                alt: false,
                shift: false,
            } => match modifier {
                TextObjectModifier::Inner => Some(EditorAction::Yank(TextObject::WordInner)),
                TextObjectModifier::Around => Some(EditorAction::Yank(TextObject::WordAround)),
            },
            Input {
                key: Key::Char('p'),
                ctrl: false,
                alt: false,
                shift: false,
            } => match modifier {
                TextObjectModifier::Inner => Some(EditorAction::Yank(TextObject::ParagraphInner)),
                TextObjectModifier::Around => Some(EditorAction::Yank(TextObject::ParagraphAround)),
            },
            _ => None,
        },
        EditorPendingAction::Change(None) => match input {
            Input {
                key: Key::Char('i'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::Pending(EditorPendingAction::Change(Some(
                TextObjectModifier::Inner,
            )))),
            Input {
                key: Key::Char('a'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::Pending(EditorPendingAction::Change(Some(TextObjectModifier::Around)))),
            Input {
                key: Key::Char('c'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::MultiAction(vec![
                EditorAction::Delete(TextObject::Line),
                EditorAction::SetMode(EditorMode::Insert),
            ])),
            Input { .. } if is_movement_key(&input) => {
                match_movement_key(&input).map(|mvmt| EditorAction::MultiAction(vec![
                    EditorAction::Delete(TextObject::To(mvmt)),
                    EditorAction::SetMode(EditorMode::Insert),
                ]))
            }
            _ => None,
        },
        EditorPendingAction::Change(Some(modifier)) => match input {
            Input {
                key: Key::Char('w'),
                ctrl: false,
                alt: false,
                shift: false,
            } => match modifier {
                TextObjectModifier::Inner => Some(EditorAction::MultiAction(vec![
                    EditorAction::Delete(TextObject::WordInner),
                    EditorAction::SetMode(EditorMode::Insert),
                ])),
                TextObjectModifier::Around => Some(EditorAction::MultiAction(vec![
                    EditorAction::Delete(TextObject::WordAround),
                    EditorAction::SetMode(EditorMode::Insert),
                ])),
            },
            Input {
                key: Key::Char('p'),
                ctrl: false,
                alt: false,
                shift: false,
            } => match modifier {
                TextObjectModifier::Inner => Some(EditorAction::MultiAction(vec![
                    EditorAction::Delete(TextObject::ParagraphInner),
                    EditorAction::SetMode(EditorMode::Insert),
                ])),
                TextObjectModifier::Around => Some(EditorAction::MultiAction(vec![
                    EditorAction::Delete(TextObject::ParagraphAround),
                    EditorAction::SetMode(EditorMode::Insert),
                ])),
            },
            _ => None,
        },
        EditorPendingAction::ReplaceChar => match input {
            Input {
                key: Key::Char(c),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::ReplaceChar(c)),
            _ => None,
        },
    }
}

pub fn handle_insert_mode_input(input: Input, is_single_line: bool) -> Option<EditorAction> {
    match input {
        Input { key: Key::Esc, .. }
        | Input {
            key: Key::Char('c'),
            ctrl: true,
            ..
        } => Some(EditorAction::MultiAction(vec![
            EditorAction::MoveCursor(CursorMove::Left),
            EditorAction::SetMode(EditorMode::Normal),
        ])),
        Input {
            key: Key::Enter, ..
        } if is_single_line => Some(EditorAction::Submit),
        Input {
            key: Key::Enter,
            ctrl: true,
            ..
        } => Some(EditorAction::Submit),
        input => Some(EditorAction::ApplyInput(input)),
    }
}

pub fn handle_visual_mode_input(
    input: Input,
    mode: VisualMode,
    _is_single_line: bool,
) -> Option<EditorAction> {
    match mode {
        VisualMode::Char => match input {
            Input { key: Key::Esc, .. } => Some(EditorAction::SetMode(EditorMode::Normal)),
            Input {
                key: Key::Char('d'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::MultiAction(vec![
                EditorAction::Delete(TextObject::Selection),
                EditorAction::SetMode(EditorMode::Normal),
            ])),
            Input {
                key: Key::Char('y'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::MultiAction(vec![
                EditorAction::Yank(TextObject::Selection),
                EditorAction::SetMode(EditorMode::Normal),
            ])),
            Input {
                key: Key::Char('c'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::MultiAction(vec![
                EditorAction::Delete(TextObject::Selection),
                EditorAction::SetMode(EditorMode::Insert),
            ])),
            Input { .. } if is_movement_key(&input) => {
                match_movement_key(&input).map(|mvmt| EditorAction::MoveCursor(mvmt))
            }
            _ => None,
        },
        VisualMode::Line => match input {
            Input { key: Key::Esc, .. } => Some(EditorAction::SetMode(EditorMode::Normal)),
            Input {
                key: Key::Char('d'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::MultiAction(vec![
                EditorAction::Delete(TextObject::Selection),
                EditorAction::SetMode(EditorMode::Normal),
            ])),
            Input {
                key: Key::Char('y'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::MultiAction(vec![
                EditorAction::Yank(TextObject::Selection),
                EditorAction::SetMode(EditorMode::Normal),
            ])),
            Input {
                key: Key::Char('c'),
                ctrl: false,
                alt: false,
                shift: false,
            } => Some(EditorAction::MultiAction(vec![
                EditorAction::Delete(TextObject::Selection),
                EditorAction::SetMode(EditorMode::Insert),
            ])),
            Input { .. } if is_movement_key(&input) => {
                match_movement_key(&input).map(|mvmt| EditorAction::MoveCursor(mvmt))
            }
            _ => None,
        },
    }
}

pub fn handle_replace_mode_input(input: Input, _is_single_line: bool) -> Option<EditorAction> {
    match input {
        Input { key: Key::Esc, .. }
        | Input {
            key: Key::Char('c'),
            ctrl: true,
            ..
        } => Some(EditorAction::MultiAction(vec![
            EditorAction::MoveCursor(CursorMove::Left),
            EditorAction::SetMode(EditorMode::Normal),
        ])),
        Input {
            key: Key::Char(c), ..
        } => Some(EditorAction::MultiAction(vec![
            EditorAction::ReplaceChar(c),
            EditorAction::MoveCursor(CursorMove::Forward),
        ])),
        _ => None,
    }
}
