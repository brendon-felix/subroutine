use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, InteractiveElement, Interactivity, IntoElement,
    ParentElement, RenderOnce, Stateful, StatefulInteractiveElement, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder,
};
use gpui_component::{ActiveTheme, Disableable, Selectable, StyledExt, h_flex};
use smallvec::SmallVec;

pub enum SelectedPosition {
    Single,
    FirstRow,
    MiddleRow,
    LastRow,
    // FirstCol,
    // MiddleCol,
    // LastCol,
}

#[derive(IntoElement)]
#[allow(dead_code)]
pub struct ListItem {
    base: Stateful<Div>,
    style: StyleRefinement,
    disabled: bool,
    selected: Option<SelectedPosition>,
    secondary_selected: bool,
    confirmed: bool,
    // check_icon: Option<Icon>,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    // on_mouse_enter: Option<Box<dyn Fn(&MouseMoveEvent, &mut Window, &mut App) + 'static>>,
    // suffix: Option<Box<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
    children: SmallVec<[AnyElement; 2]>,
}

#[allow(dead_code)]
impl ListItem {
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id: ElementId = id.into();
        Self {
            base: div().id(id),
            style: StyleRefinement::default(),
            disabled: false,
            selected: None,
            secondary_selected: false,
            confirmed: false,
            on_click: None,
            // on_mouse_enter: None,
            // check_icon: None,
            // suffix: None,
            children: SmallVec::new(),
        }
    }

    pub fn selected_position(mut self, position: Option<SelectedPosition>) -> Self {
        self.selected = position;
        self
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

impl Disableable for ListItem {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Selectable for ListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = match selected {
            true => Some(SelectedPosition::Single),
            false => None,
        };
        self
    }

    fn is_selected(&self) -> bool {
        self.selected.is_some()
    }

    fn secondary_selected(mut self, selected: bool) -> Self {
        self.secondary_selected = selected;
        self
    }
}

impl Styled for ListItem {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl InteractiveElement for ListItem {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for ListItem {}

impl ParentElement for ListItem {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ListItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_active = self.confirmed || self.selected.is_some();
        let corner_radii = self.style.corner_radii.clone();
        let mut selected_style = StyleRefinement::default();
        selected_style.corner_radii = corner_radii;
        let theme = cx.theme();
        self.base
            .cursor_pointer()
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
            .child(
                h_flex()
                    // .bg(gpui::rgb(0x0000FF))
                    .w_full()
                    .children(self.children),
            )
            .when(!is_active, |this| {
                this.hover(|this| this.bg(theme.list_hover))
            })
            .map(|this| {
                if self.selected.is_some() || self.secondary_selected {
                    let bg = if self.selected.is_some() {
                        theme.list_active
                    } else {
                        theme.accent
                    };

                    this.bg(bg).child(
                        div()
                            .absolute()
                            .inset_0()
                            // .focused_border(cx)
                            .border_color(theme.list_active_border)
                            .refine_style(&selected_style)
                            .map(|this| match self.selected.unwrap() {
                                SelectedPosition::Single => this.border_1(),
                                SelectedPosition::FirstRow => {
                                    this.border_t_1().border_x_1().rounded_b_none()
                                }
                                SelectedPosition::MiddleRow => this.border_x_1().rounded_none(),
                                SelectedPosition::LastRow => {
                                    this.border_b_1().border_x_1().rounded_t_none()
                                } // SelectedPosition::FirstCol => {
                                  //     this.border_l_1().border_y_1().rounded_r_none()
                                  // }
                                  // SelectedPosition::MiddleCol => this.border_y_1().rounded_none(),
                                  // SelectedPosition::LastCol => {
                                  //     this.border_r_1().border_y_1().rounded_l_none()
                                  // }
                            }),
                    )
                } else {
                    this
                }
            })
    }
}
