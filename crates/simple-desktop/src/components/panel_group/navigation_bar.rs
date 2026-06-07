use std::rc::Rc;

use gpui::{
    AnyElement, App, IntoElement, ParentElement, Pixels, RenderOnce, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    IconName,
    button::{Button, ButtonVariants},
    h_flex,
};
use smallvec::SmallVec;

#[derive(IntoElement)]
pub struct NavigationBar {
    base: gpui::Div,
    // style: StyleRefinement,
    left_panel_open: Option<bool>,
    right_panel_open: Option<bool>,
    on_toggle_left: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_toggle_right: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    /// Extra left padding to add when the left panel is closed, to avoid the
    /// toggle button being obscured by the macOS traffic light controls.
    /// Interpolated smoothly via the panel open/close transition.
    traffic_light_padding: Pixels,
    children: SmallVec<[AnyElement; 8]>,
}

#[allow(unused)]
impl NavigationBar {
    pub fn new() -> Self {
        Self {
            base: h_flex(),
            // style: StyleRefinement::default(),
            left_panel_open: None,
            right_panel_open: None,
            on_toggle_left: None,
            on_toggle_right: None,
            traffic_light_padding: px(0.),
            children: SmallVec::new(),
        }
    }

    /// Sets the animated extra left padding for the toggle button to clear the
    /// macOS traffic light controls when the left panel is closed.
    pub fn traffic_light_padding(mut self, padding: Pixels) -> Self {
        self.traffic_light_padding = padding;
        self
    }

    pub fn left_panel_open(mut self, open: bool) -> Self {
        self.left_panel_open = Some(open);
        self
    }

    pub fn right_panel_open(mut self, open: bool) -> Self {
        self.right_panel_open = Some(open);
        self
    }

    pub fn on_toggle_left(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_left = Some(Rc::new(f));
        self
    }

    pub fn on_toggle_right(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle_right = Some(Rc::new(f));
        self
    }
}

impl Styled for NavigationBar {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for NavigationBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for NavigationBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_toggle_left = self.on_toggle_left;
        let on_toggle_right = self.on_toggle_right;
        let traffic_light_padding = self.traffic_light_padding;

        self.base
            .items_center()
            // .justify_between()
            .when_some(self.left_panel_open, |this, open| {
                let on_toggle = on_toggle_left.clone();
                this.child(
                    div().pl(traffic_light_padding).child(
                        Button::new("left-panel-button")
                            .size_6()
                            .ghost()
                            .when_else(
                                open,
                                |btn| btn.icon(IconName::PanelLeftClose),
                                |btn| btn.icon(IconName::PanelLeftOpen),
                            )
                            .when_some(on_toggle, |btn, callback| {
                                btn.on_click(move |_, window, cx| callback(window, cx))
                            }),
                    ),
                )
            })
            .child(
                h_flex()
                    .h_full()
                    .items_center()
                    .flex_1()
                    .children(self.children),
            )
            .when_some(self.right_panel_open, |this, open| {
                let on_toggle = on_toggle_right.clone();
                this.child(
                    Button::new("right-panel-button")
                        .size_6()
                        .ghost()
                        .when_else(
                            open,
                            |btn| btn.icon(IconName::PanelRightClose),
                            |btn| btn.icon(IconName::PanelRightOpen),
                        )
                        .when_some(on_toggle, |btn, callback| {
                            btn.on_click(move |_, window, cx| callback(window, cx))
                        }),
                )
            })
    }
}
