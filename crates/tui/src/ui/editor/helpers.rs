use ratatui::style::{Color, Style, Stylize};
// use tui_textarea::{CursorMove, Input, Key, TextArea};

use crate::ui::editor::TextObjectModifier;

use super::EditorMode;

pub fn is_movement_key(input: &Input) -> bool {
    matches!(
        input,
        Input {
            key: Key::Char('h' | 'j' | 'k' | 'l' | 'w' | 'b' | 'e' | '0' | '$' | 'g' | '{' | '}'),
            ctrl: false,
            alt: false,
            shift: false,
        } | Input {
            key: Key::Char('G'),
            ctrl: false,
            alt: false,
            ..
        } | Input {
            key: Key::Up | Key::Down | Key::Left | Key::Right,
            ..
        }
    )
}

pub fn match_movement_key(input: &Input) -> Option<CursorMove> {
    match input {
        Input {
            key: Key::Char('h'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::Left),
        Input {
            key: Key::Left,
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::Back),
        Input {
            key: Key::Char('j'),
            ctrl: false,
            alt: false,
            shift: false,
        }
        | Input {
            key: Key::Down,
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::Down),
        Input {
            key: Key::Char('k'),
            ctrl: false,
            alt: false,
            shift: false,
        }
        | Input {
            key: Key::Up,
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::Up),
        Input {
            key: Key::Char('l'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::Right),
        Input {
            key: Key::Right,
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::Forward),
        Input {
            key: Key::Char('w'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::WordForward),
        Input {
            key: Key::Char('b'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::WordBack),
        Input {
            key: Key::Char('e'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::WordEnd),
        Input {
            key: Key::Char('0'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::Head),
        Input {
            key: Key::Char('$'),
            ctrl: false,
            alt: false,
            ..
        } => Some(CursorMove::End),
        Input {
            key: Key::Char('g'),
            ctrl: false,
            alt: false,
            shift: false,
        } => Some(CursorMove::Top),
        Input {
            key: Key::Char('G'),
            ctrl: false,
            alt: false,
            ..
        } => Some(CursorMove::Bottom),
        Input {
            key: Key::Char('{'),
            ctrl: false,
            alt: false,
            ..
        } => Some(CursorMove::ParagraphBack),
        Input {
            key: Key::Char('}'),
            ctrl: false,
            alt: false,
            ..
        } => Some(CursorMove::ParagraphForward),
        _ => None,
    }
}

pub fn cursor_style(mode: EditorMode, is_active: bool) -> Style {
    if !is_active {
        return Style::default();
    }

    let color = match mode {
        EditorMode::Normal => Color::Reset,
        EditorMode::Insert => Color::LightGreen,
        EditorMode::Replace => Color::LightCyan,
        EditorMode::Visual(_) => Color::LightBlue,
    };
    Style::default().fg(color).reversed()
}

// pub fn create_block<'a>(
//     title: Option<&'a str>,
//     is_active: bool,
//     is_valid: Option<bool>,
// ) -> Block<'a> {
//     let mut style = Style::default();
//     if let Some(valid) = is_valid {
//         if valid {
//             style = style.fg(Color::LightGreen);
//         } else {
//             style = style.fg(Color::LightRed);
//         }
//     }
//     if !is_active {
//         style = style.add_modifier(Modifier::DIM);
//     }

//     let mut border_style = style;
//     if !is_active {
//         border_style = border_style.add_modifier(Modifier::DIM);
//     }

//     let mut block = Block::default()
//         // .style(style)
//         .borders(Borders::ALL)
//         // .border_set(BorderType::Rounded.to_border_set())
//         .border_style(border_style);

//     if let Some(t) = title {
//         block = block.title(t);
//     }

//     block
// }

pub fn select_current_word(
    textarea: &mut TextArea,
    modifier: TextObjectModifier,
) -> (usize, usize) {
    let (current_row, current_col) = textarea.cursor();
    textarea.move_cursor(CursorMove::WordBack);
    textarea.start_selection();
    match modifier {
        TextObjectModifier::Inner => {
            textarea.move_cursor(CursorMove::WordEnd);
            textarea.move_cursor(CursorMove::Right);
        }
        TextObjectModifier::Around => {
            textarea.move_cursor(CursorMove::WordForward);
        }
    }
    (current_row, current_col)
}
pub fn select_current_line(textarea: &mut TextArea) -> (usize, usize) {
    let (current_row, current_col) = textarea.cursor();
    let total_lines = textarea.lines().len();

    if current_row + 1 == total_lines && total_lines > 1 {
        // Last line case: select from end of previous line to end of current line
        textarea.move_cursor(CursorMove::Up);
        textarea.move_cursor(CursorMove::End);
        textarea.start_selection();
        textarea.move_cursor(CursorMove::Down);
        textarea.move_cursor(CursorMove::End);
    } else {
        // Normal case: select entire line including newline
        textarea.move_cursor(CursorMove::Head);
        textarea.start_selection();
        let cursor = textarea.cursor();
        textarea.move_cursor(CursorMove::Down);
        if cursor == textarea.cursor() {
            textarea.move_cursor(CursorMove::End); // At the last line, move to end of the line instead
        }
    }
    (current_row, current_col)
}
pub fn select_current_paragraph(
    textarea: &mut TextArea,
    _modifier: TextObjectModifier,
) -> (usize, usize) {
    let (current_row, current_col) = textarea.cursor();
    (current_row, current_col)
}
