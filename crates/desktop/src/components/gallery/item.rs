use gpui::{
    AnyElement, App, Div, ElementId, InteractiveElement, Interactivity, IntoElement, ParentElement,
    RenderOnce, Stateful, StatefulInteractiveElement, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder,
};
use gpui_component::{ActiveTheme, Selectable, StyledExt};
use smallvec::SmallVec;

#[derive(IntoElement)]
#[allow(dead_code)]
pub struct GalleryItem {
    base: Stateful<Div>,
    style: StyleRefinement,
    // secondary_selected: bool,
    focused: bool,
    // on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    // on_mouse_enter: Option<Box<dyn Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static>>,
    // suffix: Option<Box<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
    children: SmallVec<[AnyElement; 2]>,
}

#[allow(dead_code)]
impl GalleryItem {
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id: ElementId = id.into();
        Self {
            base: div().id(id),
            style: StyleRefinement::default(),
            focused: false,
            // on_click: None,
            // on_mouse_enter: None,
            // suffix: None,
            children: SmallVec::new(),
        }
    }

    // pub fn on_click(
    //     mut self,
    //     handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    // ) -> Self {
    //     self.on_click = Some(Box::new(handler));
    //     self
    // }

    // pub fn on_mouse_enter(
    //     mut self,
    //     handler: impl Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static,
    // ) -> Self {
    //     self.on_mouse_enter = Some(Box::new(handler));
    //     self
    // }
}

impl Selectable for GalleryItem {
    fn selected(mut self, selected: bool) -> Self {
        self.focused = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.focused
    }
}

impl Styled for GalleryItem {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl InteractiveElement for GalleryItem {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for GalleryItem {}

impl ParentElement for GalleryItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for GalleryItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let corner_radii = self.style.corner_radii.clone();
        let mut selected_style = StyleRefinement::default();
        selected_style.corner_radii = corner_radii;
        let theme = cx.theme();
        self.base
            .overflow_hidden()
            .text_ellipsis()
            .relative()
            .gap_x_1()
            // .py_1()
            .px_3()
            .text_base()
            .text_color(theme.foreground)
            .relative()
            .items_center()
            .justify_between()
            .refine_style(&self.style)
            .child(div().size_full().children(self.children))
            .when(!self.focused, |this| {
                this.hover(|this| this.bg(theme.list_hover))
            })
            .map(|this| {
                if self.focused {
                    this.bg(theme.list_active).child(
                        div()
                            .absolute()
                            .inset_0()
                            .border_1()
                            .border_color(theme.list_active_border)
                            .refine_style(&selected_style),
                    )
                } else {
                    this.bg(theme.list).child(
                        div()
                            .absolute()
                            .inset_0()
                            .border_1()
                            .border_color(theme.border),
                    )
                }
            })
    }
}
