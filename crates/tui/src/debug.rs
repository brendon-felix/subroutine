use once_cell::sync::OnceCell;
use tokio::sync::mpsc::UnboundedSender;

use crate::app::AppAction;
use crate::ui::UIAction;

const DEFAULT_DEBUG_TICKS: u16 = 20;

/// Global sender for debug messages
static DEBUG_SENDER: OnceCell<UnboundedSender<AppAction>> = OnceCell::new();

/// Initialize the global debug sender
/// This should be called once during app startup
pub fn init_debug_sender(sender: UnboundedSender<AppAction>) {
    DEBUG_SENDER
        .set(sender)
        .expect("Debug sender already initialized");
}

fn debug_msg<S: AsRef<str>>(msg: S, n_ticks: u16) {
    if let Some(sender) = DEBUG_SENDER.get() {
        let _ = sender.send(AppAction::UIAction(UIAction::DebugMsg(
            msg.as_ref().to_string(),
            n_ticks,
        )));
    }
}

pub fn debug<S: AsRef<str>>(msg: S) {
    debug_msg(msg, DEFAULT_DEBUG_TICKS);
}

pub fn debug_f(args: std::fmt::Arguments, n_ticks: u16) {
    debug_msg(args.to_string(), n_ticks);
}

/// Macro for easy formatted debug messages
///
/// # Examples
/// ```
/// debug_fmt!("User clicked at position: {}, {}", x, y);
/// debug_fmt!("Error occurred: {}", error; 30); // With custom duration
/// ```
#[macro_export]
macro_rules! debug {
    ($fmt:expr, $($arg:expr),+; $ticks:expr) => {
        $crate::debug::debug_f(format_args!($fmt, $($arg)*), $ticks)
    };
    ($($arg:tt)*) => {
        $crate::debug::debug(format_args!($($arg)*).to_string())
    };
}
