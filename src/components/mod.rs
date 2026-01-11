use gpui::App;

pub mod checkbox;
pub mod command_palette;
pub mod custom_list;
// pub mod picker;
pub mod resizable;

pub fn init(cx: &mut App) {
    command_palette::init(cx);
    // picker::init(cx);
    custom_list::init(cx);
}
