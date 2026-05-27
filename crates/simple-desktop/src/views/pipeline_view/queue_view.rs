use std::{collections::HashSet, rc::Rc, time::Duration};

use gpui::{
    App, AsyncApp, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Pixels, Render, Size, Styled, Window, div, prelude::FluentBuilder, px, size,
};
use gpui_component::{
    ActiveTheme, Colorize, VirtualListScrollHandle, checkbox::Checkbox, h_flex,
    menu::ContextMenuExt, scroll::ScrollableElement, v_virtual_list,
};
use simple_core::AnyItem;
use uuid::Uuid;

use crate::{
    components::{DragData, Draggable},
    stores::AppDatabaseStore,
    utils::{ButtonColorizeExt, ButtonColors},
};

const COMPLETE_CHECKBOX_DURATION: Duration = Duration::from_millis(250);

pub struct QueueView {
    focus_handle: FocusHandle,
    scroll_handle: VirtualListScrollHandle,
    items: Vec<AnyItem>,
    list_item_sizes: Rc<Vec<Size<Pixels>>>,
    loaded: bool,
    completing_items: HashSet<Uuid>,
}

impl QueueView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let scroll_handle = VirtualListScrollHandle::new();

        cx.bind_keys([]);

        Self {
            focus_handle,
            scroll_handle,
            items: vec![],
            list_item_sizes: Rc::new(vec![]),
            loaded: false,
            completing_items: HashSet::new(),
        }
    }

    pub fn refresh_items(&mut self, queue: Vec<AnyItem>, cx: &mut Context<Self>) {
        self.items = queue;
        self.items.sort_by_key(|i| i.time().map(|t| t.timestamp()));

        self.list_item_sizes = Rc::new(
            (0..self.items.len())
                .map(|_| size(px(0.), px(64.)))
                .collect(),
        );

        if !self.loaded {
            self.loaded = true;
        }

        cx.notify();
    }

    fn begin_complete_item(&mut self, action_id: Uuid, cx: &mut Context<Self>) {
        self.completing_items.insert(action_id);
        cx.notify();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            cx.background_executor()
                .timer(COMPLETE_CHECKBOX_DURATION)
                .await;
            let _ = this.update(cx, |view, cx| {
                view.completing_items.remove(&action_id);
                let store = AppDatabaseStore::global(cx);
                store.update(cx, |store, cx| {
                    store.complete_action(action_id, cx);
                });
            });
        })
        .detach();
    }
}

impl Focusable for QueueView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for QueueView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let completing_items = self.completing_items.clone();

        div()
            .id("queue-view-container")
            .size_full()
            .track_focus(&self.focus_handle)
            .child(
                v_virtual_list(
                    cx.entity(),
                    "queue-view-list",
                    self.list_item_sizes.clone(),
                    move |view, visible_range, _, cx| {
                        visible_range
                            .map(|i| {
                                let item = &view.items[i];
                                let item_id = item.id();
                                let any_item = item.clone();
                                let any_item_for_drag = item.clone();
                                let any_item_for_menu = item.clone();
                                let is_completing = completing_items.contains(&item_id);

                                let button_colors = match item {
                                    AnyItem::Action(_) => {
                                        ButtonColors::normal(cx.theme().button_primary, cx)
                                    }
                                    AnyItem::Event(_) => ButtonColors::normal(
                                        cx.theme()
                                            .button_primary
                                            .mix_oklab(cx.theme().foreground, 0.5),
                                        cx,
                                    ),
                                };

                                let is_action = matches!(item, AnyItem::Action(_));
                                let preview_title: gpui::SharedString = any_item.title().into();
                                let fg = cx.theme().muted_foreground;
                                let colors = button_colors;

                                let drag_data = DragData::new(any_item_for_drag.clone())
                                    .with_label(any_item_for_drag.title())
                                    .with_preview(move || {
                                        let mut preview = div()
                                            .w(px(64. * 4.))
                                            .h(px(52.))
                                            .rounded_lg()
                                            .bg(colors.bg)
                                            .p_2()
                                            .text_color(fg)
                                            .child(preview_title.clone());
                                        if let Some(border_color) = colors.border {
                                            preview = preview.border_1().border_color(border_color);
                                        }
                                        preview.into_any_element()
                                    });

                                div().size_full().px_2().py_1().child(
                                    Draggable::new(
                                        ("queue-view-draggable", item_id.as_u128() as u64),
                                        drag_data,
                                    )
                                    .size_full()
                                    .context_menu(
                                        move |menu, window, cx| match &any_item_for_menu {
                                            AnyItem::Action(a) => {
                                                super::action_context_menu(a.id)(menu, window, cx)
                                            }
                                            AnyItem::Event(e) => {
                                                super::event_context_menu(e.id)(menu, window, cx)
                                            }
                                        },
                                    )
                                    .child(
                                        div()
                                            .id(("queue-view-item", item_id.as_u128() as u64))
                                            .size_full()
                                            .button_colors(button_colors)
                                            .rounded_lg()
                                            .p_2()
                                            .child(
                                                h_flex()
                                                    .size_full()
                                                    .gap_2()
                                                    .when(is_action, |this| {
                                                        this.child(
                                                            Checkbox::new((
                                                                "queue-complete",
                                                                item_id.as_u128() as u64,
                                                            ))
                                                            .checked(is_completing)
                                                            .occlude()
                                                            .on_click(cx.listener(
                                                                move |this, _, _window, cx| {
                                                                    this.begin_complete_item(
                                                                        item_id, cx,
                                                                    );
                                                                },
                                                            )),
                                                        )
                                                    })
                                                    .child(any_item.title().to_string()),
                                            ),
                                    ),
                                )
                            })
                            .collect()
                    },
                )
                .min_w(px(64. * 4. - 4.))
                .my_1()
                .track_scroll(&self.scroll_handle),
            )
    }
}
