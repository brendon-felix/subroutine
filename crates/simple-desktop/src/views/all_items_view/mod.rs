use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window};
use gpui_component::{
    list::{List, ListState},
    v_flex,
};

use crate::stores::{AppDatabaseStore, DataChanged};

mod list;
use list::*;

pub struct AllItemsView {
    list_state: Entity<ListState<AllItemsList>>,
}

impl AllItemsView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = AllItemsList::new();
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));

        let db_store = AppDatabaseStore::global(cx);

        cx.subscribe(&db_store, |view, store, _: &DataChanged, cx| {
            view.list_state.update(cx, |list, cx| {
                let store = store.read(cx);
                let d = list.delegate_mut();
                d.actions = store.actions().into_iter().map(|a| (a, true)).collect();
                d.events = store.events().into_iter().map(|e| (e, true)).collect();
                d.routines = store.routines().into_iter().map(|r| (r, true)).collect();
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

impl Render for AllItemsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .absolute()
            .inset_0()
            .items_center()
            .child(List::new(&self.list_state))
    }
}
