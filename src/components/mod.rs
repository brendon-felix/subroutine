use gpui::App;

pub mod checkbox;
pub mod command_palette;
pub mod custom_list;
pub mod drag_drop;
// pub mod gallery;
pub mod resizable;

pub fn init(cx: &mut App) {
    command_palette::init(cx);
    // gallery::init(cx);
    custom_list::init(cx);
}
