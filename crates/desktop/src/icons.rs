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
    Archive,
    Balloon,
    CalendarCheck,
    CalendarClock,
    CalendarPlus,
    Calendars,
    Check,
    CheckLine,
    CirclePlus,
    Clock,
    ClockPlus,
    Copy,
    FastForward,
    House,
    LayoutList,
    List,
    ListCheck,
    ListChecks,
    ListOrdered,
    ListPlus,
    ListRestart,
    ListStart,
    ListTodo,
    LogOut,
    MapPin,
    Minus,
    RefreshCcw,
    RefreshCw,
    Repeat,
    Repeat2,
    RotateCcw,
    Route,
    Rows3,
    Rows4,
    Save,
    ScrollText,
    SquareCheckBig,
    SquareMinus,
    StepForward,
    TicketCheck,
    Trash,
}

impl IconNamed for AppIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Archive => "icons/custom/archive.svg",
            Self::Balloon => "icons/custom/balloon.svg",
            Self::CalendarCheck => "icons/custom/calendar-check-2.svg",
            Self::CalendarClock => "icons/custom/calendar-clock.svg",
            Self::CalendarPlus => "icons/custom/calendar-plus.svg",
            Self::Calendars => "icons/custom/calendars.svg",
            Self::Check => "icons/custom/check.svg",
            Self::CheckLine => "icons/custom/check-line.svg",
            Self::CirclePlus => "icons/custom/circle-plus.svg",
            Self::Clock => "icons/custom/clock.svg",
            Self::ClockPlus => "icons/custom/clock-plus.svg",
            Self::Copy => "icons/custom/copy.svg",
            Self::FastForward => "icons/custom/fast-forward.svg",
            Self::House => "icons/custom/house.svg",
            Self::LayoutList => "icons/custom/layout-list.svg",
            Self::List => "icons/custom/list.svg",
            Self::ListCheck => "icons/custom/list-check.svg",
            Self::ListChecks => "icons/custom/list-checks.svg",
            Self::ListOrdered => "icons/custom/list-ordered.svg",
            Self::ListPlus => "icons/custom/list-plus.svg",
            Self::ListRestart => "icons/custom/list-restart.svg",
            Self::ListStart => "icons/custom/list-start.svg",
            Self::ListTodo => "icons/custom/list-todo.svg",
            Self::LogOut => "icons/custom/log-out.svg",
            Self::MapPin => "icons/custom/map-pin.svg",
            Self::Minus => "icons/custom/minus.svg",
            Self::RefreshCcw => "icons/custom/refresh-ccw.svg",
            Self::RefreshCw => "icons/custom/refresh-cw.svg",
            Self::Repeat => "icons/custom/repeat.svg",
            Self::Repeat2 => "icons/custom/repeat-2.svg",
            Self::RotateCcw => "icons/custom/rotate-ccw.svg",
            Self::Route => "icons/custom/route.svg",
            Self::Rows3 => "icons/custom/rows-3.svg",
            Self::Rows4 => "icons/custom/rows-4.svg",
            Self::Save => "icons/custom/save.svg",
            Self::ScrollText => "icons/custom/scroll-text.svg",
            Self::SquareCheckBig => "icons/custom/square-check-big.svg",
            Self::SquareMinus => "icons/custom/square-minus.svg",
            Self::StepForward => "icons/custom/step-forward.svg",
            Self::TicketCheck => "icons/custom/ticket-check.svg",
            Self::Trash => "icons/custom/trash.svg",
        }
        .into()
    }
}

impl RenderOnce for AppIcon {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        Icon::empty().path(self.path())
    }
}
