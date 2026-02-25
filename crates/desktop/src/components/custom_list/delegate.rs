use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, Window};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex};

use crate::components::custom_list::{ListItem, ListState};

/// A delegate for the List.
#[allow(unused)]
pub trait ListDelegate: Sized + 'static {
    fn render_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem>;

    fn render_empty(
        &mut self,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .size_full()
            .justify_center()
            .text_color(cx.theme().muted_foreground.opacity(0.6))
            .child(Icon::new(IconName::EllipsisVertical).size_12())
            .into_any_element()
    }

    fn render_initial(
        &mut self,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<AnyElement> {
        None
    }

    fn set_selected_index(
        &mut self,
        ix: Option<usize>,
        // window: &mut Window,
        // cx: &mut Context<ListState<Self>>,
    );

    fn confirm(&mut self, secondary: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {}

    fn items_count(&self, cx: &App) -> usize;
}
