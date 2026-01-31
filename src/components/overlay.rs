use gpui::{App, Context, KeyBinding, MouseButton, Window, actions, prelude::*};
use gpui_component::{ActiveTheme, StyledExt, Theme, h_flex};

actions!(overlay, [CloseOverlay]);

/// Initialize overlay-related global bindings.
///
/// Binds `escape` to `CloseOverlay` in the `"Overlay"` key context.
/// Call this once at application startup (where other `init` functions are called).
pub fn init(cx: &mut App) {
    let context: Option<&str> = Some("Overlay");
    cx.bind_keys([KeyBinding::new("escape", CloseOverlay, context)]);
}

/// Reusable overlay chrome.
///
/// - Adds dimmed backdrop
/// - Positions absolutely and occludes underlying content
/// - Sets the key context to `"Overlay"` so shared overlay key bindings apply
/// - Dispatches `CloseOverlay` on backdrop click
///
/// The `inner` element should call `cx.stop_propagation()` on its mouse handlers
/// (or use `.on_any_mouse_down(|_,_,cx| cx.stop_propagation())`) so clicks inside
/// the dialog do not close the overlay.
///
/// Example usage from an overlay's `render`:
/// ```ignore
/// let theme = cx.theme();
/// let inner = /* build dialog card as element */;
/// crate::components::overlay::shell(theme, inner)
/// ```
pub fn shell<T: IntoElement>(theme: &Theme, inner: T) -> impl IntoElement {
    h_flex() // overlay background
        .bg(theme
            .background
            .blend(gpui::black().opacity(0.15))
            .opacity(0.4))
        .absolute()
        .inset_0()
        .size_full()
        .occlude()
        .key_context("Overlay")
        .on_mouse_down(MouseButton::Left, |_event, window, cx| {
            window.dispatch_action(Box::new(CloseOverlay), cx);
        })
        .justify_center()
        .items_start()
        .pt_20()
        .child(inner)
}
