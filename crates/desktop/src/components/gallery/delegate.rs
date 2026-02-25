use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Window,
};
use gpui_component::{h_flex, ActiveTheme, Icon, IconName, Selectable};

use crate::components::gallery::GalleryState;

/// Trait for providing data to a Gallery component
///
/// This simplified delegate trait works with the new AnimationManager-based gallery system.
/// Instead of rendering individual items, it provides data that the gallery uses to create
/// and animate its own visual elements.
#[allow(unused)]
pub trait GalleryDelegate: Sized + 'static {
    /// The type of item data this delegate provides
    type Item: Selectable + IntoElement + Styled + StatefulInteractiveElement;

    fn render_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<GalleryState<Self>>,
    ) -> Option<Self::Item>;

    fn render_focused_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<GalleryState<Self>>,
    ) -> Option<Self::Item> {
        self.render_item(ix, window, cx)
    }

    fn render_empty(
        &mut self,
        window: &mut Window,
        cx: &mut Context<GalleryState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .size_full()
            .justify_center()
            .text_color(cx.theme().muted_foreground.opacity(0.6))
            .child(Icon::new(IconName::Ellipsis).size_12())
            .into_any_element()
    }

    fn render_initial(
        &mut self,
        window: &mut Window,
        cx: &mut Context<GalleryState<Self>>,
    ) -> Option<AnyElement> {
        None
    }

    /// Confirm the current selection
    fn select_focused(
        &mut self,
        secondary: bool,
        window: &mut Window,
        cx: &mut Context<GalleryState<Self>>,
    ) {
    }

    /// Return the total number of items in the gallery
    fn items_count(&self, cx: &App) -> usize;
}
