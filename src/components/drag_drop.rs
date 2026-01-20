//! Drag and drop components with draggable elements and drop zones.
//!
//! This module provides a comprehensive drag and drop system following GPUI's type-safe approach
//! while maintaining the essential styling and functionality features.

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::divider::Divider;
use gpui_component::{ActiveTheme, Theme};
use std::fmt::Debug;
use std::rc::Rc;

// use crate::theme::use_theme;

/// Core drag data structure that wraps the payload with preview and positioning information
pub struct DragData<T: Clone + Debug> {
    pub data: T,
    pub label: Option<SharedString>,
    pub preview_factory: Option<Rc<dyn Fn() -> AnyElement>>,
    pub position: Point<Pixels>,
}

impl<T: Clone + Debug> Clone for DragData<T> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            label: self.label.clone(),
            preview_factory: self.preview_factory.clone(),
            position: self.position.clone(),
        }
    }
}

impl<T: Clone + Debug> Debug for DragData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DragData")
            .field("data", &self.data)
            .field("label", &self.label)
            .field("preview_factory", &self.preview_factory.is_some())
            .field("position", &self.position)
            .finish()
    }
}

impl<T: Clone + Debug> DragData<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            label: None,
            preview_factory: None,
            position: Point::default(),
        }
    }

    pub fn with_label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_preview<F>(mut self, factory: F) -> Self
    where
        F: Fn() -> AnyElement + 'static,
    {
        self.preview_factory = Some(Rc::new(move || factory()));
        self
    }

    pub fn with_position(mut self, position: Point<Pixels>) -> Self {
        self.position = position;
        self
    }
}

impl<T: Clone + Debug + 'static> Render for DragData<T> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // let theme = use_theme();
        let theme = cx.theme();

        if let Some(factory) = &self.preview_factory {
            let preview = factory();
            return div()
                .absolute()
                .left(self.position.x)
                .top(self.position.y)
                .child(preview);
        }

        let size = gpui::size(px(250.0), px(80.0));

        div()
            .pl(self.position.x - size.width / 2.0)
            .pt(self.position.y - size.height / 2.0)
            .child(
                div()
                    .flex()
                    .justify_center()
                    .items_center()
                    .min_w(size.width)
                    .max_w(px(300.0))
                    .min_h(size.height)
                    .px(px(16.0))
                    .py(px(12.0))
                    .bg(theme.group_box.opacity(0.95))
                    .border_1()
                    .border_color(theme.border)
                    .text_color(theme.foreground)
                    .font_family(theme.font_family.clone())
                    .text_size(px(14.0))
                    .font_weight(FontWeight::MEDIUM)
                    .rounded_md()
                    .shadow(vec![BoxShadow {
                        color: hsla(0.0, 0.0, 0.0, 0.3),
                        offset: point(px(0.0), px(4.0)),
                        blur_radius: px(12.0),
                        spread_radius: px(0.0),
                    }])
                    .when_some(self.label.clone(), |this, label| this.child(label))
                    .when(self.label.is_none(), |this| this.child("Dragging...")),
            )
    }
}

/// Event data for drag move operations, providing position and bounds information
/// for calculating insertion points and visual feedback.
#[derive(Clone, Debug)]
pub struct DragMoveEvent<T> {
    pub data: T,
    pub position: Point<Pixels>,
    pub bounds: Bounds<Pixels>,
}

#[derive(Debug)]
pub struct DropIndicator {
    pub index: usize,
    pub position: DropPosition,
}

/// Position indicator for reorderable lists, showing where an item will be inserted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropPosition {
    Before,
    After,
}

/// Helper function to calculate drop position based on mouse position and element bounds.
/// Returns Before if the mouse is in the upper half, After if in the lower half.
pub fn calculate_drop_position(mouse_y: Pixels, bounds: &Bounds<Pixels>) -> DropPosition {
    let relative_y = mouse_y - bounds.origin.y;
    let midpoint = bounds.size.height / 2.0;

    if relative_y < midpoint {
        DropPosition::Before
    } else {
        DropPosition::After
    }
}

#[derive(IntoElement)]
pub struct Draggable<T: Clone + Debug + 'static> {
    base: Stateful<Div>,
    drag_data: DragData<T>,
    cursor_style: CursorStyle,
    hover_bg: Option<Hsla>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl<T: Clone + Debug + 'static> Draggable<T> {
    pub fn new(id: impl Into<ElementId>, drag_data: DragData<T>) -> Self {
        Self {
            base: div().id(id.into()),
            drag_data,
            cursor_style: CursorStyle::PointingHand,
            hover_bg: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    pub fn cursor_style(mut self, cursor: CursorStyle) -> Self {
        self.cursor_style = cursor;
        self
    }

    pub fn hover_bg(mut self, color: Hsla) -> Self {
        self.hover_bg = Some(color);
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    pub fn children<I>(mut self, children: impl IntoIterator<Item = I>) -> Self
    where
        I: IntoElement,
    {
        for child in children {
            self.children.push(child.into_any_element());
        }
        self
    }
}

impl<T: Clone + Debug + 'static> Styled for Draggable<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: Clone + Debug + 'static> ParentElement for Draggable<T> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl<T: Clone + Debug + 'static> RenderOnce for Draggable<T> {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let drag_data = self.drag_data.clone();
        let user_style = self.style;

        self.base
            .cursor(self.cursor_style)
            .when_some(self.hover_bg, |this, bg| {
                this.hover(move |style| style.bg(bg))
            })
            .on_drag(drag_data, |data: &DragData<T>, position, _, cx| {
                cx.new(|_| data.clone().with_position(position))
            })
            .map(|this| {
                let mut div = this;
                div.style().refine(&user_style);
                div
            })
            .children(self.children)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropZoneStyle {
    Dashed,
    Solid,
    Filled,
}

#[derive(IntoElement)]
pub struct DropZone<T: Clone + Debug + 'static> {
    base: Stateful<Div>,
    drop_style: DropZoneStyle,
    active: bool,
    min_height: Option<Pixels>,
    children: Vec<AnyElement>,
    user_style: StyleRefinement,
    insertion_indicator: Option<DropIndicator>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Clone + Debug + 'static> DropZone<T> {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: div().id(id.into()),
            drop_style: DropZoneStyle::Dashed,
            active: false,
            min_height: None,
            children: Vec::new(),
            user_style: StyleRefinement::default(),
            insertion_indicator: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn drop_zone_style(mut self, style: DropZoneStyle) -> Self {
        self.drop_style = style;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn min_h(mut self, height: impl Into<Pixels>) -> Self {
        self.min_height = Some(height.into());
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    pub fn children<I>(mut self, children: impl IntoIterator<Item = I>) -> Self
    where
        I: IntoElement,
    {
        for child in children {
            self.children.push(child.into_any_element());
        }
        self
    }

    pub fn insertion_indicator(mut self, indicator: Option<DropIndicator>) -> Self {
        self.insertion_indicator = indicator;
        self
    }
}

impl<T: Clone + Debug + 'static> Styled for DropZone<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.user_style
    }
}

impl<T: Clone + Debug + 'static> InteractiveElement for DropZone<T> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl<T: Clone + Debug + 'static> StatefulInteractiveElement for DropZone<T> {}

impl<T: Clone + Debug + 'static> ParentElement for DropZone<T> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

fn render_insertion_indicator(theme: &Theme) -> AnyElement {
    div()
        .h(px(74.0))
        .w_full()
        .bg(theme.drop_target)
        .rounded_lg()
        .border_1()
        .border_color(theme.primary)
        // .rounded(px(1.0))
        // .my(px(2.0))
        .into_any_element()
}

impl<T: Clone + Debug + 'static> RenderOnce for DropZone<T> {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        // let theme = use_theme();
        let theme = cx.theme().clone();
        let user_style = self.user_style;
        let indicator = self.insertion_indicator;

        let (border_width, border_color, bg_color) = match (self.drop_style, self.active) {
            (DropZoneStyle::Dashed, false) => (px(2.0), theme.border, gpui::transparent_black()),
            (DropZoneStyle::Dashed, true) => (px(2.0), theme.primary, theme.primary.opacity(0.05)),
            (DropZoneStyle::Solid, false) => (px(2.0), theme.border, gpui::transparent_black()),
            (DropZoneStyle::Solid, true) => (px(2.0), theme.primary, theme.primary.opacity(0.1)),
            (DropZoneStyle::Filled, false) => (px(1.0), theme.border, theme.muted),
            (DropZoneStyle::Filled, true) => (px(2.0), theme.primary, theme.primary.opacity(0.15)),
        };

        let mut children_with_indicators = Vec::new();
        let children = self.children;
        let original_children_len = children.len();

        for (i, child) in children.into_iter().enumerate() {
            // Add insertion indicator before this item if needed
            if let Some(ref ind) = indicator {
                if ind.index == i && matches!(ind.position, DropPosition::Before) {
                    println!("DEBUG: DropZone - Adding indicator BEFORE item {}", i);
                    children_with_indicators.push(
                        // div()
                        //     // .h(px(2.0))
                        //     .h_3()
                        //     .w_full()
                        //     .bg(theme.primary)
                        //     .rounded(px(1.0))
                        //     .my(px(2.0))
                        //     .into_any_element(),
                        // Divider::horizontal_dashed().into_any_element(),
                        render_insertion_indicator(&theme),
                    );
                }
            }

            children_with_indicators.push(child);

            // Add insertion indicator after this item if needed
            if let Some(ref ind) = indicator {
                if ind.index == i && matches!(ind.position, DropPosition::After) {
                    println!("DEBUG: DropZone - Adding indicator AFTER item {}", i);
                    children_with_indicators.push(
                        // div()
                        //     .h(px(2.0))
                        //     .w_full()
                        //     .bg(theme.primary)
                        //     .rounded(px(1.0))
                        //     .my(px(2.0))
                        //     .into_any_element(),
                        // Divider::horizontal_dashed().color(theme).into_any_element(),
                        render_insertion_indicator(&theme),
                    );
                }
            }
        }

        // Add indicator at the end if dropping beyond all items
        if let Some(ref ind) = indicator {
            if ind.index >= original_children_len {
                println!(
                    "DEBUG: DropZone - Adding indicator AT END (index {} >= len {})",
                    ind.index, original_children_len
                );
                children_with_indicators.push(
                    // div()
                    //     .h(px(2.0))
                    //     .w_full()
                    //     .bg(theme.primary)
                    //     .rounded(px(1.0))
                    //     .my(px(2.0))
                    //     .into_any_element(),
                    // Divider::horizontal_dashed().into_any_element(),
                    render_insertion_indicator(&theme),
                );
            }
        }

        self.base
            .flex()
            .flex_col()
            .items_start()
            .justify_start()
            .gap(px(8.0))
            .w_full()
            .when_some(self.min_height, |this, h| this.min_h(h))
            .px(px(16.0))
            .py(px(16.0))
            .rounded(theme.radius_lg)
            .bg(bg_color)
            .border_color(border_color)
            .when(self.drop_style == DropZoneStyle::Dashed, |this| {
                this.border(border_width)
            })
            .when(self.drop_style != DropZoneStyle::Dashed, |this| {
                this.border(border_width)
            })
            .drag_over::<DragData<T>>(move |style, _, _, _| {
                style
                    .bg(theme.primary.opacity(0.1))
                    .border_color(theme.primary)
            })
            .map(|this| {
                let mut div = this;
                div.style().refine(&user_style);
                div
            })
            .children(children_with_indicators)
    }
}

/// Data structure for reorderable drag operations.
/// Contains both the original index and the data being dragged.
#[derive(Clone, Debug)]
pub struct ReorderData<T: Clone + Debug> {
    pub index: usize,
    pub data: T,
}

/// A reorderable list item that acts as both a drag source and drop target.
///
/// This component is designed for creating lists where items can be dragged
/// to reorder them. It provides insertion indicators and handles the complex
/// logic of reordering.
#[derive(IntoElement)]
pub struct ReorderableItem<T: Clone + Debug + 'static> {
    base: Stateful<Div>,
    index: usize,
    data: T,
    preview_label: Option<String>,
    show_drop_indicator: Option<DropPosition>,
    children: Vec<AnyElement>,
    style: StyleRefinement,
}

impl<T: Clone + Debug + 'static> ReorderableItem<T> {
    /// Create a new reorderable item with the specified index and data.
    pub fn new(id: impl Into<ElementId>, index: usize, data: T) -> Self {
        Self {
            base: div().id(id.into()),
            index,
            data,
            preview_label: None,
            show_drop_indicator: None,
            children: Vec::new(),
            style: StyleRefinement::default(),
        }
    }

    /// Set a custom label for the drag preview.
    pub fn preview_label(mut self, label: impl Into<String>) -> Self {
        self.preview_label = Some(label.into());
        self
    }

    /// Show a drop indicator at the specified position.
    pub fn drop_indicator(mut self, position: Option<DropPosition>) -> Self {
        self.show_drop_indicator = position;
        self
    }

    /// Add a child element to this reorderable item.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// Add multiple child elements to this reorderable item.
    pub fn children<I>(mut self, children: impl IntoIterator<Item = I>) -> Self
    where
        I: IntoElement,
    {
        for child in children {
            self.children.push(child.into_any_element());
        }
        self
    }
}

impl<T: Clone + Debug + 'static> Styled for ReorderableItem<T> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<T: Clone + Debug + 'static> InteractiveElement for ReorderableItem<T> {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl<T: Clone + Debug + 'static> StatefulInteractiveElement for ReorderableItem<T> {}

impl<T: Clone + Debug + 'static> ParentElement for ReorderableItem<T> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl<T: Clone + Debug + 'static> RenderOnce for ReorderableItem<T> {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let data = self.data.clone();
        let index = self.index;
        let preview_label = self
            .preview_label
            .unwrap_or_else(|| format!("Item {}", index));
        let user_style = self.style;
        let primary_color = theme.primary;

        div()
            .flex()
            .flex_col()
            .when_some(
                self.show_drop_indicator,
                move |this, position| match position {
                    DropPosition::Before => this.border_t_2().border_color(primary_color),
                    DropPosition::After => this.border_b_2().border_color(primary_color),
                },
            )
            .child(
                self.base
                    .cursor(CursorStyle::PointingHand)
                    .on_drag(
                        DragData::new(ReorderData { index, data })
                            .with_label(SharedString::from(preview_label)),
                        move |drag_data: &DragData<ReorderData<T>>, position, _, cx| {
                            cx.new(|_| drag_data.clone().with_position(position))
                        },
                    )
                    .drag_over::<DragData<ReorderData<T>>>(move |style, _, _, _| {
                        style.bg(primary_color.opacity(0.05))
                    })
                    .map(|this| {
                        let mut div = this;
                        div.style().refine(&user_style);
                        div
                    })
                    .children(self.children),
            )
    }
}

/// A complete reorderable list component that manages drag and drop reordering.
pub struct ReorderableList<T: Clone + Debug> {
    items: Vec<T>,
    drop_target: Option<usize>,
    drag_position: Option<DropPosition>,
}

impl<T: Clone + Debug> ReorderableList<T> {
    /// Create a new reorderable list with the specified items.
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            drop_target: None,
            drag_position: None,
        }
    }

    /// Update the drop target based on drag move events.
    pub fn update_drop_target(&mut self, target_index: usize, position: DropPosition) {
        self.drop_target = Some(target_index);
        self.drag_position = Some(position);
    }

    /// Clear the drop target (typically when drag ends or leaves).
    pub fn clear_drop_target(&mut self) {
        self.drop_target = None;
        self.drag_position = None;
    }

    /// Perform a reorder operation, moving an item from one index to another.
    /// Returns true if the operation was successful.
    pub fn reorder(&mut self, from_index: usize, to_index: usize) -> bool {
        if from_index >= self.items.len() || to_index > self.items.len() || from_index == to_index {
            return false;
        }

        let item = self.items.remove(from_index);
        let insert_at = if to_index > from_index {
            to_index - 1
        } else {
            to_index
        };

        self.items.insert(insert_at, item);
        self.clear_drop_target();
        true
    }

    /// Get the current items in the list.
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// Get the current drop target information.
    pub fn drop_target(&self) -> Option<(usize, DropPosition)> {
        self.drop_target.zip(self.drag_position)
    }
}
