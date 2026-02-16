use gpui::App;

pub mod checkbox;
pub mod command_palette;
pub mod custom_list;
pub mod drag_drop;
pub mod popover;
pub mod resizable;
pub mod task_creator;

pub fn init(cx: &mut App) {
    // picker::init(cx);
    command_palette::init(cx);
    // gallery::init(cx);
    custom_list::init(cx);
    popover::init(cx);
}
