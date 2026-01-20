use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Window,
};
use gpui_component::{ActiveTheme, Icon, IconName, Selectable, h_flex};

use crate::components::pipeline::{PipelineItem, PipelineState};

/// Trait for providing data to a Gallery component
///
/// This simplified delegate trait works with the new AnimationManager-based gallery system.
/// Instead of rendering individual items, it provides data that the gallery uses to create
/// and animate its own visual elements.
#[allow(unused)]
pub trait PipelineDelegate: Sized + 'static {
    /// The type of item data this delegate provides
    // type Item: Selectable + IntoElement + Styled + StatefulInteractiveElement;

    type ItemId: Clone + PartialEq + 'static;

    fn render_item(
        &mut self,
        id: &Self::ItemId,
        window: &mut Window,
        cx: &mut Context<PipelineState<Self>>,
    ) -> Option<PipelineItem>;

    fn render_focused_item(
        &mut self,
        id: &Self::ItemId,
        window: &mut Window,
        cx: &mut Context<PipelineState<Self>>,
    ) -> Option<PipelineItem> {
        self.render_item(id, window, cx)
    }

    fn render_empty(
        &mut self,
        window: &mut Window,
        cx: &mut Context<PipelineState<Self>>,
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
        cx: &mut Context<PipelineState<Self>>,
    ) -> Option<AnyElement> {
        None
    }

    /// Return the total number of items in the gallery
    fn items_count(&self, cx: &App) -> usize;
}
