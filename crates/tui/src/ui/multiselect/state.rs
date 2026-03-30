use ratatui::layout::Rect;

// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct MultiSelectListState {
    pub offset: usize,
    pub selected: Option<usize>,
    pub visual_start: Option<usize>,
    pub last_items_area: Option<Rect>,
}

impl MultiSelectListState {
    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    pub const fn with_selected(mut self, selected: Option<usize>) -> Self {
        self.selected = selected;
        self
    }

    /// Index of the first item to be displayed
    pub const fn offset(&self) -> usize {
        self.offset
    }

    pub const fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    /// Index of the selected item
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub const fn selected_mut(&mut self) -> &mut Option<usize> {
        &mut self.selected
    }

    pub fn start_visual_selection(&mut self) {
        if self.visual_start.is_none() {
            if let Some(idx) = self.selected {
                self.visual_start = Some(idx);
            }
        }
    }

    pub fn is_in_visual_mode(&self) -> bool {
        self.visual_start.is_some()
    }

    pub fn get_visual_start(&self) -> Option<usize> {
        self.visual_start
    }

    pub fn end_visual_selection(&mut self) {
        if self.visual_start.is_some() {
            self.visual_start = None;
        }
    }

    pub const fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }

    pub fn select_next(&mut self) {
        let next = self.selected.map_or(0, |i| i.saturating_add(1));
        self.select(Some(next));
    }

    pub fn select_previous(&mut self) {
        let previous = self.selected.map_or(usize::MAX, |i| i.saturating_sub(1));
        self.select(Some(previous));
    }

    pub fn select_first(&mut self) {
        self.select(Some(0));
    }

    pub fn select_last(&mut self) {
        self.select(Some(usize::MAX));
    }

    pub fn scroll_down_by(&mut self, amount: u16) {
        let selected = self.selected.unwrap_or_default();
        self.select(Some(selected.saturating_add(amount as usize)));
    }

    pub fn scroll_up_by(&mut self, amount: u16) {
        let selected = self.selected.unwrap_or_default();
        self.select(Some(selected.saturating_sub(amount as usize)));
    }
}
