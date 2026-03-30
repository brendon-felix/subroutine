mod focused;
pub mod state;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Block, StatefulWidget, Widget},
};

use state::FocusListState;

use crate::ui::focuslist::{
    focused::{FocusedItem, NextPrevItem},
    state::FocusListPosition,
};

const FOCUSED_WIDTH: u16 = 70;
const NEXT_PREV_WIDTH: u16 = 50;

#[derive(Clone)]
pub struct FocusListItem<'a> {
    content: Text<'a>,
    style: Style,
    border_color: Option<Color>,
}

impl<'a> FocusListItem<'a> {
    pub fn new<T>(content: T) -> Self
    where
        T: Into<Text<'a>>,
    {
        Self {
            content: content.into(),
            style: Style::default(),
            border_color: None,
        }
    }

    pub fn with_border_color(mut self, color: Color) -> Self {
        self.border_color = Some(color);
        self
    }
}

impl<'a, T> From<T> for FocusListItem<'a>
where
    T: Into<Text<'a>>,
{
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[derive(Default)]
pub struct FocusList<'a> {
    focused_index: Option<usize>,
    block: Option<Block<'a>>,
    items: Vec<FocusListItem<'a>>,
    style: Style,
}

impl<'a> FocusList<'a> {
    pub fn new<T>(items: T) -> Self
    where
        T: IntoIterator,
        T::Item: Into<FocusListItem<'a>>,
    {
        Self {
            items: items.into_iter().map(Into::into).collect(),
            ..Self::default()
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn with_block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn with_focused_index(mut self, index: usize) -> Self {
        self.focused_index = Some(index);
        self
    }

    pub fn set_items<T>(&mut self, items: T)
    where
        T: IntoIterator,
        T::Item: Into<FocusListItem<'a>>,
    {
        self.items = items.into_iter().map(Into::into).collect();
    }

    pub fn get_index(&self, position: FocusListPosition) -> Option<usize> {
        if let Some(current) = self.focused_index {
            let idx = match position {
                FocusListPosition::PrevPrev if current >= 2 => current.saturating_sub(2),
                FocusListPosition::Prev if current >= 1 => current.saturating_sub(1),
                FocusListPosition::Focused => current,
                FocusListPosition::Next if current + 1 < self.items.len() => current + 1,
                FocusListPosition::NextNext if current + 2 < self.items.len() => current + 2,
                _ => return None,
            };
            Some(idx)
        } else {
            None
        }
    }

    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    pub fn focus(&mut self, index: Option<usize>) {
        if let Some(i) = index
        // && !(self.focused_index() == Some(i))
        {
            if i >= self.items.len() {
                self.focused_index = Some(self.items.len().saturating_sub(1));
            } else {
                self.focused_index = Some(i);
            }
        } else {
            self.focused_index = None;
        }
    }

    pub fn focus_next(&mut self) -> bool {
        if let Some(current) = self.focused_index {
            let next = current.saturating_add(1);
            if next < self.items.len() {
                self.focused_index = Some(next);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn focus_previous(&mut self) -> bool {
        if let Some(current) = self.focused_index {
            if current > 0 {
                let previous = current.saturating_sub(1);
                self.focused_index = Some(previous);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn focus_first(&mut self) {
        self.focus(Some(0));
    }

    pub fn focus_last(&mut self) {
        self.focus(Some(self.len().saturating_sub(1)));
    }

    fn render_position(
        &self,
        position: FocusListPosition,
        new_area: Rect,
        buf: &mut Buffer,
        state: &mut FocusListState,
    ) {
        let area = if let Some(area) = state.get_area(position) {
            area
        } else {
            let area = constrained_centered_area(new_area, position);
            state.set_area(area, position);
            area
        };
        if let Some(idx) = self.get_index(position) {
            let item: FocusListItem<'_> = self.items[idx].clone();
            match position {
                FocusListPosition::Focused => FocusedItem::new(item).render(area, buf),
                FocusListPosition::Next | FocusListPosition::Prev => {
                    NextPrevItem::new(item).render(area, buf);
                }
                FocusListPosition::NextNext | FocusListPosition::PrevPrev
                    if !state.is_completed(position) =>
                {
                    NextPrevItem::new(item).render(area, buf);
                }
                _ => {}
            };
        }
    }
}

impl<'a> StatefulWidget for &FocusList<'a> {
    type State = FocusListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let (previous_area, focused_area, next_area) = split_area(area);
        self.render_position(FocusListPosition::PrevPrev, previous_area, buf, state);
        self.render_position(FocusListPosition::Prev, previous_area, buf, state);
        self.render_position(FocusListPosition::Focused, focused_area, buf, state);
        self.render_position(FocusListPosition::Next, next_area, buf, state);
        self.render_position(FocusListPosition::NextNext, next_area, buf, state);
    }
}

fn constrained_centered_area(area: Rect, position: FocusListPosition) -> Rect {
    let max = match position {
        FocusListPosition::Focused => FOCUSED_WIDTH,
        _ => NEXT_PREV_WIDTH,
    };
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![
            Constraint::Fill(1),
            Constraint::Max(max),
            Constraint::Fill(1),
        ])
        .split(area)[1]
}

fn split_area(area: Rect) -> (Rect, Rect, Rect) {
    let splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(7), // 1: previous item
            Constraint::Max(3),
            Constraint::Length(7), // 3: focused item
            Constraint::Max(3),
            Constraint::Length(7), // 5: next item
            Constraint::Fill(1),
        ])
        .split(area);
    (splits[1], splits[3], splits[5])
}
