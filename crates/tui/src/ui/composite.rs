use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};
// use tui_textarea::CursorMove;

use crate::ui::editor::{
    Editor, EditorMode,
    actions::{EditorAction, EditorActions, EditorPendingAction},
};

// use crate::ui::{AppWidget, WidgetStyle};

pub struct CompositeEditorState {
    // position: Position,
    // sub_positions: Vec<Position>,
    last_area: Rect,
    sub_areas: Vec<Rect>,
    splits_fn: Box<dyn Fn(Rect) -> Vec<Rect>>,
}

impl CompositeEditorState {
    pub fn new(num_editors: usize, splits_fn: Box<dyn Fn(Rect) -> Vec<Rect>>) -> Self {
        Self {
            // position: Position::default(),
            // sub_positions: vec![Position::default(); num_editors],
            last_area: Rect::default(),
            sub_areas: vec![Rect::default(); num_editors],
            splits_fn,
        }
    }

    // pub fn set_position(&mut self, position: Position) {
    //     self.position = position;
    // }

    // pub fn set_sub_positions(&mut self, positions: Vec<Position>) {
    //     self.sub_positions = positions;
    // }

    // pub fn get_sub_positions(&self) -> &Vec<Position> {
    //     &self.sub_positions
    // }

    pub fn set_last_area(&mut self, area: Rect) {
        self.last_area = area;
    }

    pub fn set_sub_areas(&mut self, areas: Vec<Rect>) {
        self.sub_areas = areas;
    }

    pub fn get_sub_areas(&self) -> &Vec<Rect> {
        &self.sub_areas
    }
}

pub struct CompositeEditor {
    pub editors: Vec<Editor>,
    active_index: Option<usize>,
    // constraints: Vec<Constraint>,
    // last_area_pos: Option<Position>,
}

impl CompositeEditor {
    pub fn new(editors: Vec<Editor>) -> Self {
        let active_index = if editors.is_empty() { None } else { Some(0) };
        let mut composite = Self {
            editors,
            active_index,
            // constraints: vec![Constraint::Fill(1); n_editors],
            // last_area_pos: None,
        };
        composite.set_active_editor(active_index);
        composite
    }

    // pub fn with_constraints(mut self, constraints: Vec<Constraint>) -> Self {
    //     self.constraints = constraints;
    //     self
    // }

    pub fn set_mode(&mut self, mode: EditorMode) {
        self.execute_action(EditorAction::SetMode(mode));
    }

    pub fn set_active_editor(&mut self, index: Option<usize>) {
        self.active_index = index;
        self.set_style_active();
    }

    pub fn set_active_editor_previous(&mut self) {
        if let Some(current_index) = self.active_index {
            if current_index > 0 {
                self.set_active_editor(Some(current_index - 1));
            }
        }
    }

    pub fn set_active_editor_next(&mut self) {
        if let Some(current_index) = self.active_index {
            if current_index + 1 < self.editors.len() {
                self.set_active_editor(Some(current_index + 1));
            }
        }
    }

    pub fn set_active_editor_first(&mut self) {
        if !self.editors.is_empty() {
            self.set_active_editor(Some(0));
        }
    }

    pub fn set_active_editor_last(&mut self) {
        if !self.editors.is_empty() {
            self.set_active_editor(Some(self.editors.len() - 1));
        }
    }

    pub fn clear_all(&mut self) {
        for editor in self.editors.iter_mut() {
            editor.set_content("");
        }
    }

    pub fn all_valid(&self) -> bool {
        self.editors.iter().all(|editor| editor.is_valid())
    }

    pub fn get_active_editor(&mut self) -> Option<(&mut Editor, usize)> {
        self.active_index
            .and_then(|index| self.editors.get_mut(index).map(|editor| (editor, index)))
    }

    pub fn get_active_content(&self) -> Option<&[String]> {
        self.active_index
            .and_then(|index| self.editors.get(index))
            .map(|editor| editor.get_lines())
    }

    pub fn get_mode(&self) -> Option<EditorMode> {
        self.active_index
            .and_then(|index| self.editors.get(index))
            .map(|editor| editor.get_mode())
    }

    // pub fn create_chunks(&self, area: Rect) -> Vec<Rect> {
    //     if self.constraints.is_empty() {
    //         let constraints =
    //             vec![Constraint::Percentage(100 / self.editors.len() as u16,); self.editors.len()];
    //         Layout::vertical(constraints).split(area).to_vec()
    //     } else {
    //         Layout::vertical(self.constraints.clone())
    //             .split(area)
    //             .to_vec()
    //     }
    // }

    pub fn is_cursor_at_line_start(&mut self) -> bool {
        if let Some((editor, _)) = self.get_active_editor() {
            editor.is_cursor_at_line_start()
        } else {
            false
        }
    }

    pub fn is_cursor_at_line_end(&mut self) -> bool {
        if let Some((editor, _)) = self.get_active_editor() {
            editor.is_cursor_at_line_end()
        } else {
            false
        }
    }

    pub fn is_first_editor_active(&self) -> bool {
        self.active_index == Some(0)
    }

    pub fn is_last_editor_active(&self) -> bool {
        self.active_index == Some(self.editors.len().saturating_sub(1))
    }

    pub fn set_style_active(&mut self) {
        self.editors.iter_mut().enumerate().for_each(|(i, editor)| {
            if Some(i) == self.active_index {
                editor.set_active(true);
            } else {
                editor.set_active(false);
            }
        });
    }

    pub fn set_style_inactive(&mut self) {
        self.editors.iter_mut().for_each(|editor| {
            editor.set_active(false);
        });
    }

    // fn set_sub_positions(&mut self, positions: Vec<Position>) {
    //     self.editors
    //         .iter_mut()
    //         .zip(positions.into_iter())
    //         .for_each(|(editor, pos)| {
    //             // editor.set_last_area_pos(pos);
    //         });
    // }
}

impl EditorActions for CompositeEditor {
    fn execute_action(&mut self, action: EditorAction) {
        let num_editors = self.editors.len();
        if let Some((editor, idx)) = self.get_active_editor() {
            let desired_column = editor.get_desired_column();
            let mode = editor.get_mode();
            if mode == EditorMode::Normal || mode == EditorMode::Insert {
                let cursor_movement = match action {
                    // EditorAction::MoveCursor(CursorMove::Up)
                    //     if idx > 0 && editor.is_cursor_on_first_line() =>
                    // {
                    //     self.set_active_editor_previous();
                    //     Some(CursorMove::Bottom) // Move to bottom of editor above
                    // }
                    // EditorAction::MoveCursor(CursorMove::Down)
                    //     if idx + 1 < num_editors && editor.is_cursor_on_last_line() =>
                    // {
                    //     self.set_active_editor_next();
                    //     Some(CursorMove::Top) // Move to top of editor below
                    // }
                    // EditorAction::MoveCursor(CursorMove::Top) if idx > 0 => {
                    //     self.set_active_editor_first();
                    //     Some(CursorMove::Top) // Move to top of topmost editor
                    // }
                    // EditorAction::MoveCursor(CursorMove::Bottom) if idx + 1 < num_editors => {
                    //     self.set_active_editor_last();
                    //     Some(CursorMove::Bottom) // Move to bottom of bottommost editor
                    // }
                    // EditorAction::MultiAction(actions) => {
                    //     for act in actions {
                    //         self.execute_action(act);
                    //     }
                    //     None
                    // }
                    _ => {
                        editor.execute_action(action);
                        None
                    }
                };
                if let Some(movement) = cursor_movement {
                    if let Some((editor, _)) = self.get_active_editor() {
                        editor.set_desired_column(desired_column);
                        editor.execute_action(EditorAction::SetMode(mode));
                        editor.execute_action(EditorAction::MoveCursor(movement));
                    }
                }
            } else {
                editor.execute_action(action);
            }
        }
    }

    fn set_pending_action(&mut self, pending: Option<EditorPendingAction>) {
        if let Some(active_index) = self.active_index {
            if let Some(editor) = self.editors.get_mut(active_index) {
                editor.set_pending_action(pending);
            }
        }
    }

    fn get_pending_action(&mut self) -> Option<EditorPendingAction> {
        if let Some(active_index) = self.active_index {
            if let Some(editor) = self.editors.get_mut(active_index) {
                return editor.get_pending_action();
            }
        }
        None
    }
}

// impl AppWidget for CompositeEditor {
//     fn set_widget_style(&mut self, style: WidgetStyle) {
//         match style {
//             WidgetStyle::Active => {
//                 if let Some(active_index) = self.active_index {
//                     self.editors.iter_mut().enumerate().for_each(|(i, editor)| {
//                         if i == active_index {
//                             editor.set_widget_style(WidgetStyle::Active);
//                         } else {
//                             editor.set_widget_style(WidgetStyle::Inactive);
//                         }
//                     });
//                 }
//             }
//             WidgetStyle::Inactive => {
//                 self.editors.iter_mut().for_each(|editor| {
//                     editor.set_widget_style(WidgetStyle::Inactive);
//                 });
//             }
//         }
//     }

//     fn on_click(&mut self, pos: Position) {
//         if let Some(area_pos) = self.last_area_pos {
//             // let chunks = self.create_chunks(area);
//             // for (i, chunk) in chunks.iter().enumerate() {
//             //     if chunk.contains(pos) {
//             //         self.set_active_editor(Some(i));
//             //         if let Some(editor) = self.get_active_editor() {
//             //             editor.set_last_area_pos(chunk.as_position());
//             //             editor.on_click(pos);
//             //         }
//             //         break;
//             //     }
//             // }
//         }
//     }

//     fn set_last_area_pos(&mut self, area_pos: Position) {
//         self.last_area_pos = Some(area_pos);
//         // let chunks = self.create_chunks(area);
//         // for chunk in chunks.iter() {
//         //     if let Some(editor) = self.get_active_editor() {
//         //         editor.set_last_area(chunk.clone());
//         //     }
//         // }
//     }
// }

// impl Widget for CompositeEditor {
//     fn render(self, area: Rect, buf: &mut Buffer) {
//         let chunks = self.create_chunks(area);
//         for (i, editor) in self.editors.into_iter().enumerate() {
//             editor.render(chunks[i], buf);
//         }
//     }
// }

// impl WidgetRef for CompositeEditor {
//     fn render_ref(&self, area: Rect, buf: &mut Buffer) {
//         let chunks = self.create_chunks(area);
//         for (i, editor) in self.editors.iter().enumerate() {
//             editor.render_ref(chunks[i], buf);
//         }
//     }
// }

// impl StatefulWidget for CompositeEditor {
//     type State = CompositeEditorState;

//     fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
//         let chunks = self.create_chunks(area);
//         state.set_sub_areas(chunks.clone());
//         for (i, editor) in self.editors.into_iter().enumerate() {
//             editor.render(chunks[i], buf);
//         }
//     }
// }

// impl StatefulWidgetRef for CompositeEditor {
//     type State = CompositeEditorState;

//     fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
//         let chunks = self.create_chunks(area);
//         // state.set_sub_positions(chunks.iter().map(|chunk| chunk.as_position()).collect());
//         state.set_sub_areas(chunks.clone());
//         for (i, editor) in self.editors.iter().enumerate() {
//             editor.render(chunks[i], buf);
//         }
//     }
// }

impl StatefulWidget for &mut CompositeEditor {
    type State = CompositeEditorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chunks = (state.splits_fn)(area);
        // state.set_sub_positions(chunks.iter().map(|chunk| chunk.as_position()).collect());
        state.set_sub_areas(chunks.clone());
        for (i, editor) in self.editors.iter_mut().enumerate() {
            editor.update_style();
            editor.render(chunks[i], buf);
        }
    }
}
