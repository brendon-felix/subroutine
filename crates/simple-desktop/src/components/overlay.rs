use gpui::{App, KeyBinding, MouseButton, Pixels, actions, div, prelude::*};
use gpui_component::{ActiveTheme, v_flex};

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
pub fn overlay<T: IntoElement>(inner: T, top: Pixels, cx: &mut App) -> impl IntoElement {
    let bg_color = cx
        .theme()
        .background
        .blend(gpui::black().opacity(0.15))
        .opacity(0.4);

    v_flex()
        // .bg(bg_color)
        .absolute()
        .inset_0()
        .size_full()
        .occlude()
        .key_context("Overlay")
        .on_mouse_down(MouseButton::Left, |_event, window, cx| {
            window.dispatch_action(Box::new(CloseOverlay), cx);
        })
        .items_center()
        .opacity(0.8)
        .child(div().w_full().h(top))
        .child(inner)
}
