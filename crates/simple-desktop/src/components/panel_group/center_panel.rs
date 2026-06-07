use gpui::{
    AnyElement, App, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, px,
};
use gpui_component::v_flex;
use smallvec::SmallVec;

#[derive(Clone)]
pub struct CenterPanelState {
    // pub min_proportion: f32,
    pub min_width: Pixels,
}

impl Default for CenterPanelState {
    fn default() -> Self {
        Self {
            // min_proportion: 0.2,
            min_width: px(100.),
        }
    }
}
#[derive(IntoElement)]
pub struct CenterPanel {
    base: gpui::Stateful<gpui::Div>,
    // navigation_bar: Option<NavigationBar>,
    children: SmallVec<[AnyElement; 8]>,
}

impl CenterPanel {
    pub fn new() -> Self {
        Self {
            base: div().id("center-panel"),
            // navigation_bar: None,
            children: SmallVec::new(),
        }
    }

    // pub fn navigation_bar(mut self) -> Self {
    //     self.navigation_bar = Some(NavigationBar::new());
    //     self
    // }
}

impl Styled for CenterPanel {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for CenterPanel {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for CenterPanel {}

impl ParentElement for CenterPanel {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for CenterPanel {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.base.size_full().child(
            v_flex()
                .size_full()
                // .when_some(self.navigation_bar, |this, nav| this.child(nav))
                .children(self.children),
        )
    }
}
