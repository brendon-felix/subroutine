use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window};
use gpui_component::{
    list::{List, ListState},
    v_flex,
};

use crate::stores::{AppDatabaseStore, DataChanged};

mod list;
use list::*;

pub struct SavedItemsView {
    list_state: Entity<ListState<SavedItemsList>>,
}

impl SavedItemsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = SavedItemsList::new();
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));

        let db_store = AppDatabaseStore::global(cx);

        cx.subscribe(&db_store, |view, store, _: &DataChanged, cx| {
            view.list_state.update(cx, |list, cx| {
                let store = store.read(cx);
                let d = list.delegate_mut();
                d.saved_actions = store
                    .actions()
                    .into_iter()
                    .filter(|a| a.template_id.is_some())
                    .collect();
                d.saved_events = store
                    .events()
                    .into_iter()
                    .filter(|e| e.template_id.is_some())
                    .collect();
                d.saved_routines = store.routines();
                d.filtered_ids = d
                    .saved_actions
                    .iter()
                    .map(|a| a.id)
                    .chain(d.saved_events.iter().map(|e| e.id))
                    .chain(d.saved_routines.iter().map(|r| r.id))
                    .collect();
                if d.loading {
                    d.loading = false;
                }
                cx.notify();
            });
        })
        .detach();

        Self { list_state }
    }
}

impl Render for SavedItemsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .absolute()
            .inset_0()
            .items_center()
            .child(List::new(&self.list_state))
    }
}
