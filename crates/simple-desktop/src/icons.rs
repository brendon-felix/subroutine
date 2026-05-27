use gpui::{App, IntoElement, RenderOnce, SharedString, Window};
use gpui_component::{Icon, IconNamed};

/// The name of a custom icon in the asset bundle.
/// These map to SVG files inside `assets/icons/custom/`.
///
/// The standard gpui-component icons (arrows, chevrons, etc.) are handled
/// directly by gpui-component and do not need to be listed here.
#[derive(Clone, Debug, IntoElement)]
#[allow(dead_code)]
pub enum AppIcon {
    CalendarClock,
    CalendarPlus,
    ListPlus,
    ListIndentIncrease,
    Check,
    Plus,
    Minus,
    Trash,
    Close,
}

impl IconNamed for AppIcon {
    fn path(self) -> SharedString {
        match self {
            Self::CalendarClock => "icons/custom/calendar-clock.svg",
            Self::Check => "icons/custom/check.svg",
            Self::CalendarPlus => "icons/custom/calendar-plus.svg",
            Self::ListPlus => "icons/custom/list-plus.svg",
            Self::ListIndentIncrease => "icons/custom/list-indent-increase.svg",
            Self::Plus => "icons/custom/plus.svg",
            Self::Minus => "icons/custom/minus.svg",
            Self::Trash => "icons/custom/trash.svg",
            Self::Close => "icons/custom/close.svg",
        }
        .into()
    }
}

impl RenderOnce for AppIcon {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        Icon::empty().path(self.path())
    }
}
