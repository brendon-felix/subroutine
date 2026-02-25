use std::rc::Rc;

use gpui::{App, SharedString, StyleRefinement, Styled, Window};
use gpui_component::Icon;

#[derive(Clone)]
pub struct Command {
    pub label: SharedString,
    pub shortcut: Option<SharedString>,
    pub icon: Option<Icon>,
    pub search_terms: Vec<SharedString>,
    pub on_select: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    pub style: StyleRefinement,
}

impl Command {
    pub fn new(label: impl Into<SharedString>) -> Self {
        let label = label.into();

        Self {
            label,
            shortcut: None,
            icon: None,
            search_terms: vec![],
            on_select: None,
            style: StyleRefinement::default(),
        }
    }

    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn search_terms<I, S>(mut self, terms: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<SharedString>,
    {
        self.search_terms
            .extend(terms.into_iter().map(|s| s.into()));
        self
    }

    pub fn on_select<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl Styled for Command {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
