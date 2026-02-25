use gpui::{App, IntoElement, ParentElement, Styled, Window};
use gpui_component::{ActiveTheme, Icon, IconName, h_flex};

use crate::components::custom_list::ListItem;

/// A delegate trait that defines how a picker handles its items.
/// This allows for flexible item types, rendering, filtering, and selection behavior.
#[allow(unused)]
pub trait PickerDelegate: Sized + 'static {
    /// The type of items this picker manages
    type Item: Clone + 'static;

    /// Returns all available items
    fn items(&self) -> &[Self::Item];

    /// Returns the currently filtered items (subset of all items based on search query)
    fn filtered_items(&self) -> &[Self::Item];

    /// Updates the filtered items based on a search query
    fn update_filter(&mut self, query: &str);

    /// Returns the total count of filtered items
    fn items_count(&self) -> usize {
        self.filtered_items().len()
    }

    /// Renders a single item at the given index
    fn render_item(
        &self,
        ix: usize,
        item: &Self::Item,
        selected: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<ListItem>;

    /// Renders the empty state when no items match the search
    fn render_empty(&self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .size_full()
            .justify_center()
            .text_color(cx.theme().muted_foreground.opacity(0.6))
            .child(Icon::new(IconName::Search).size_12())
            .child("No items found")
    }

    /// Returns the placeholder text for the search input
    fn placeholder_text(&self) -> &str {
        "Search..."
    }

    /// Called when an item is confirmed (selected with Enter or double-click)
    fn confirm(&mut self, ix: usize, item: &Self::Item, window: &mut Window, cx: &mut App);

    /// Called when the picker is cancelled (Escape pressed)
    fn cancel(&mut self, window: &mut Window, cx: &mut App) {}

    /// Called when selection changes (up/down arrow navigation)
    fn select(&mut self, ix: Option<usize>, window: &mut Window, cx: &mut App) {}

    /// Returns whether an item matches a search query (for default filtering)
    fn item_matches(&self, item: &Self::Item, query: &str) -> bool;

    /// Returns a relevance score for an item given a query (higher is better)
    fn item_score(&self, item: &Self::Item, query: &str) -> i32 {
        // Default implementation: exact match > prefix match > contains match
        let item_text = self.item_text(item).to_lowercase();
        let query = query.to_lowercase();

        if item_text == query {
            1000
        } else if item_text.starts_with(&query) {
            500
        } else if item_text.contains(&query) {
            100
        } else {
            0
        }
    }

    /// Returns the searchable text for an item (used by default filtering)
    fn item_text(&self, item: &Self::Item) -> String;
}
