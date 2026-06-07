use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

use gpui::{
    App, AsyncApp, Context, DragMoveEvent, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, Render, Size, Styled, Window, div, prelude::FluentBuilder,
    px, size,
};
use gpui_component::{
    ActiveTheme, Colorize, Icon, IconName, VirtualListScrollHandle, h_flex, label::Label,
    menu::ContextMenuExt, v_flex, v_virtual_list,
};
use simple_core::AnyItem;
use uuid::Uuid;

use crate::{
    components::{Checkbox, DragData, Draggable, DropZone},
    stores::AppDatabaseStore,
    utils::{ButtonColorizeExt, ButtonColors},
};

const COMPLETE_CHECKBOX_DURATION: Duration = Duration::from_millis(250);

pub struct FocusView {
    focus_handle: FocusHandle,
    scroll_handle: VirtualListScrollHandle,
    items: Vec<AnyItem>,
    item_focus_handles: HashMap<Uuid, FocusHandle>,
    list_item_sizes: Rc<Vec<Size<Pixels>>>,
    loaded: bool,
    completing_items: HashSet<Uuid>,
    drop_active: bool,
}

impl FocusView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let scroll_handle = VirtualListScrollHandle::new();

        cx.bind_keys([]);

        Self {
            focus_handle,
            scroll_handle,
            items: vec![],
            item_focus_handles: HashMap::new(),
            list_item_sizes: Rc::new(vec![]),
            loaded: false,
            completing_items: HashSet::new(),
            drop_active: false,
        }
    }

    pub fn refresh_items(&mut self, queue: Vec<AnyItem>, cx: &mut Context<Self>) {
        self.items = queue;
        self.items.sort_by_key(|i| i.time().map(|t| t.timestamp()));

        // Create focus handles for any new items; drop handles for removed items.
        let current_ids: HashSet<Uuid> = self.items.iter().map(|i| i.id()).collect();
        self.item_focus_handles
            .retain(|id, _| current_ids.contains(id));
        for item in &self.items {
            self.item_focus_handles
                .entry(item.id())
                .or_insert_with(|| cx.focus_handle());
        }

        self.list_item_sizes = Rc::new(
            (0..self.items.len())
                .map(|_| size(px(0.), px(64. * 4.)))
                .collect(),
        );

        if !self.loaded {
            self.loaded = true;
        }

        cx.notify();
    }

    fn drop_zone(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DropZone<DragData<AnyItem>> {
        let mouse_position = window.mouse_position();
        DropZone::new("focus-drop")
            .size_full()
            .active(self.drop_active)
            .rounded_none()
            .rounded_bl_2xl()
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DragData<AnyItem>>, _window, cx| {
                    let is_action = matches!(event.drag(cx).data, AnyItem::Action(_));
                    let is_over = event.bounds.contains(&mouse_position);
                    let active = is_over && is_action;
                    if active != this.drop_active {
                        this.drop_active = active;
                        cx.notify();
                    }
                },
            ))
            .on_drop(cx.listener(|this, data: &DragData<AnyItem>, _window, cx| {
                if this.drop_active {
                    if let AnyItem::Action(action) = &data.data {
                        let id = action.id;
                        let db_store = AppDatabaseStore::global(cx);
                        db_store.update(cx, |store, cx| store.auto_queue_action(id, cx));
                    }
                    this.drop_active = false;
                    cx.notify();
                }
            }))
    }

    fn begin_complete_item(
        &mut self,
        action_id: Uuid,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Focus the next item (or previous if last) before the item disappears.
        let next_focus = {
            let pos = self.items.iter().position(|i| i.id() == action_id);
            pos.and_then(|p| {
                let next_id = self
                    .items
                    .get(p + 1)
                    .or_else(|| if p > 0 { self.items.get(p - 1) } else { None })
                    .map(|i| i.id());
                next_id.and_then(|id| self.item_focus_handles.get(&id).cloned())
            })
        };
        if let Some(handle) = next_focus {
            handle.focus(window, cx);
        } else {
            self.focus_handle.focus(window, cx);
        }
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

impl Focusable for FocusView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FocusView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let completing_items = self.completing_items.clone();
        let ring_color = cx.theme().ring;

        self.drop_zone(window, cx)
            .track_focus(&self.focus_handle)
            .pt_8()
            .items_center()
            .justify_center()
            .child(
                v_virtual_list(
                    cx.entity(),
                    "focus-view-list",
                    self.list_item_sizes.clone(),
                    move |view, visible_range, window, cx| {
                        visible_range
                            .map(|i| {
                                let item = &view.items[i];
                                let item_id = item.id();
                                let any_item = item.clone();
                                let any_item_for_drag = item.clone();
                                let any_item_for_menu = item.clone();
                                let is_completing = completing_items.contains(&item_id);
                                let item_focus_handle = view
                                    .item_focus_handles
                                    .get(&item_id)
                                    .cloned()
                                    .unwrap_or_else(|| cx.focus_handle());
                                let is_focused = item_focus_handle.is_focused(window);

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
                                    AnyItem::Routine(_) => {
                                        ButtonColors::normal(cx.theme().button_primary, cx)
                                    }
                                };

                                let is_action = matches!(item, AnyItem::Action(_));
                                let preview_title: gpui::SharedString = any_item.title().into();
                                let fg = cx.theme().muted_foreground;
                                let colors = button_colors;
                                let meta_text = super::format_item_meta(&any_item);

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
                                    })
                                    .with_preview_size(gpui::size(px(64. * 4.), px(52.)));

                                div().size_full().px_2().py_1().child(
                                    Draggable::new(
                                        ("focus-view-draggable", item_id.as_u128() as u64),
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
                                            AnyItem::Routine(r) => {
                                                super::routine_context_menu(r.id)(menu, window, cx)
                                            }
                                        },
                                    )
                                    .child(
                                        div()
                                            .id(("focus-view-item", item_id.as_u128() as u64))
                                            .track_focus(&item_focus_handle.tab_stop(true))
                                            .size_full()
                                            .button_colors(button_colors)
                                            .rounded_lg()
                                            .p_2()
                                            .when(is_focused, |el| el.border_color(ring_color))
                                            .on_key_down(cx.listener(
                                                move |this, event: &KeyDownEvent, window, cx| {
                                                    if event.is_held {
                                                        return;
                                                    }
                                                    match event.keystroke.key.as_str() {
                                                        "space" | "enter"
                                                            if is_action && !is_completing =>
                                                        {
                                                            cx.stop_propagation();
                                                            this.begin_complete_item(
                                                                item_id, window, cx,
                                                            );
                                                        }
                                                        "up" | "k" => {
                                                            cx.stop_propagation();
                                                            let pos = this
                                                                .items
                                                                .iter()
                                                                .position(|i| i.id() == item_id);
                                                            if let Some(p) = pos {
                                                                if p > 0 {
                                                                    if let Some(h) =
                                                                        this.item_focus_handles.get(
                                                                            &this.items[p - 1].id(),
                                                                        )
                                                                    {
                                                                        h.focus(window, cx);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        "down" | "j" => {
                                                            cx.stop_propagation();
                                                            let pos = this
                                                                .items
                                                                .iter()
                                                                .position(|i| i.id() == item_id);
                                                            if let Some(p) = pos {
                                                                if let Some(next) =
                                                                    this.items.get(p + 1)
                                                                {
                                                                    if let Some(h) = this
                                                                        .item_focus_handles
                                                                        .get(&next.id())
                                                                    {
                                                                        h.focus(window, cx);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        "escape" => {
                                                            cx.stop_propagation();
                                                            this.focus_handle.focus(window, cx);
                                                        }
                                                        _ => {}
                                                    }
                                                },
                                            ))
                                            .child(
                                                v_flex()
                                                    .size_full()
                                                    .gap_0p5()
                                                    .justify_center()
                                                    .child(
                                                        h_flex()
                                                            .w_full()
                                                            .gap_2()
                                                            .when(is_action, |this| {
                                                                this.child(
                                                                Checkbox::new((
                                                                    "focus-complete",
                                                                    item_id.as_u128() as u64,
                                                                ))
                                                                .checked(is_completing)
                                                                .tab_stop(false)
                                                                .occlude()
                                                                .on_click(cx.listener(
                                                                    move |this, _, window, cx| {
                                                                        this.begin_complete_item(
                                                                            item_id, window, cx,
                                                                        );
                                                                    },
                                                                )),
                                                            )
                                                            })
                                                            .when(!is_action, |this| {
                                                                this.child(
                                                                    Icon::new(IconName::Calendar)
                                                                        .size_4()
                                                                        .flex_shrink_0()
                                                                        .text_color(
                                                                            cx.theme()
                                                                                .muted_foreground,
                                                                        ),
                                                                )
                                                            })
                                                            .child(
                                                                Label::new(
                                                                    any_item.title().to_string(),
                                                                )
                                                                .text_sm()
                                                                .truncate(),
                                                            ),
                                                    )
                                                    .when_some(meta_text, |this, meta| {
                                                        this.child(
                                                            Label::new(meta).text_xs().text_color(
                                                                cx.theme().muted_foreground,
                                                            ),
                                                        )
                                                    }),
                                            ),
                                    ),
                                )
                            })
                            .collect()
                    },
                )
                // .min_w(px(64. * 4. - 4.))
                .w_1_2()
                .my_1()
                .track_scroll(&self.scroll_handle),
            )
    }
}
