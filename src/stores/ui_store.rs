use gpui::EventEmitter;

#[derive(Clone, Debug, PartialEq)]
pub enum ViewType {
    TaskList,
    Today,
    Upcoming,
}

#[derive(Clone, Debug)]
pub struct UiStateChanged;

// #[derive(Clone, Debug)]
// pub struct CommandPaletteToggled;

pub struct UiStateStore {
    pub current_view: ViewType,
}

impl UiStateStore {
    pub fn new() -> Self {
        Self {
            current_view: ViewType::TaskList,
        }
    }

    pub fn set_current_view(&mut self, view_type: ViewType) {
        self.current_view = view_type;
    }
}

impl EventEmitter<UiStateChanged> for UiStateStore {}
// impl EventEmitter<CommandPaletteToggled> for UiStateStore {}
