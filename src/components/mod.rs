use gpui::App;

pub mod command_palette;
pub mod custom_list;

pub fn init(cx: &mut App) {
    command_palette::init(cx);
    custom_list::init(cx);
}
