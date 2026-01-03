use std::rc::Rc;

use gpui::{App, SharedString, Window};

#[derive(Clone)]
#[allow(unused)]
pub struct Command {
    pub id: SharedString,
    pub name: SharedString,
    pub description: Option<SharedString>,
    // pub icon: Option<IconSource>,
    // pub category: Option<SharedString>,
    pub shortcut: Option<SharedString>,
    pub on_select: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    // style: StyleRefinement,
    // disabled: bool,
    // selected: bool,
    search_text: String,
    // confirmed: bool,
}

impl Command {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        let id = id.into();
        let name = name.into();
        let search_text = name.to_string().to_lowercase();

        Self {
            id,
            name,
            description: None,
            // icon: None,
            // category: None,
            shortcut: None,
            on_select: None,
            // selected: false,
            search_text,
        }
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        let desc = description.into();
        self.search_text = format!("{} {}", self.name, desc).to_lowercase();
        self.description = Some(desc);
        self
    }

    // pub fn icon(mut self, icon: impl Into<IconSource>) -> Self {
    //     self.icon = Some(icon.into());
    //     self
    // }

    // pub fn category(mut self, category: impl Into<SharedString>) -> Self {
    //     self.category = Some(category.into());
    //     self
    // }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn on_select<F>(mut self, handler: F) -> Self
    where
        F: Fn(&mut Window, &mut App) + 'static,
    {
        self.on_select = Some(Rc::new(handler));
        self
    }

    pub fn matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }

        let query = query.to_lowercase();
        self.search_text.contains(&query)
    }

    pub fn match_score(&self, query: &str) -> i32 {
        if query.is_empty() {
            return 0;
        }

        let query = query.to_lowercase();
        let name_lower = self.name.to_string().to_lowercase();

        if name_lower == query {
            return 1000;
        }

        if name_lower.starts_with(&query) {
            return 500;
        }

        if name_lower.contains(&query) {
            return 100;
        }

        if self.search_text.contains(&query) {
            return 50;
        }

        0
    }
}
