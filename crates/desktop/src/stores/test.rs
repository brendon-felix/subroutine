use gpui::{IntoElement, Styled};
use gpui_component::button::Button;

pub fn test() -> impl IntoElement {
    Button::new("test").cursor_pointer()
}
