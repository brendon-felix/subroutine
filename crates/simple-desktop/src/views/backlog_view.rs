use std::{collections::HashSet, rc::Rc, time::Duration};

use gpui::{
    AsyncApp, Context, DragMoveEvent, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    Render, SharedString, Size, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{ActiveTheme, VirtualListScrollHandle, h_flex, label::Label, v_virtual_list};
use gpui_squircle::{SquircleStyled, squircle};
use simple_core::{Action, AnyItem};
use uuid::Uuid;

use crate::{
    components::{Checkbox, DragData, Draggable, DropZone},
    stores::{AppDatabaseStore, DataChanged},
    utils::{ButtonColorizeExt, ButtonColors},
};

const ITEM_HEIGHT: Pixels = px(80.);
const COMPLETE_CHECKBOX_DURATION: Duration = Duration::from_millis(250);

fn render_backlog_item_preview(
    colors: ButtonColors,
    title: SharedString,
    item_w: Pixels,
    item_h: Pixels,
    muted_fg: Hsla,
) -> impl IntoElement {
    h_flex()
        .w(item_w)
        .h(item_h)
        .px_2()
        .gap_2()
        .py_0p5()
        .rounded_lg()
        .shadow_md()
        .border_1()
        .bg(colors.bg)
        .when_some(colors.border, |this, c| this.border_color(c))
        .child(Label::new(title).text_sm().text_color(muted_fg))
}

pub struct BacklogView {
    scroll_handle: VirtualListScrollHandle,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    backlog: Vec<Action>,
    completing_items: HashSet<Uuid>,
    drop_active: bool,
}

// impl Focusable for BacklogView {
//     fn focus_handle(&self, _cx: &App) -> FocusHandle {
//         self.focus_handle.clone()
//     }
// }

impl BacklogView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let scroll_handle = VirtualListScrollHandle::new();
        let item_sizes = Rc::new(vec![]);

        let db_store = AppDatabaseStore::global(cx);

        let backlog = db_store.read(cx).backlogged_actions();

        cx.subscribe(&db_store, |view, store, _: &DataChanged, cx| {
            view.backlog = store.read(cx).backlogged_actions();
            view.refresh_item_sizes(ITEM_HEIGHT);
            cx.notify();
        })
        .detach();

        Self {
            scroll_handle,
            item_sizes,
            backlog,
            completing_items: HashSet::new(),
            drop_active: false,
        }
    }

    fn refresh_item_sizes(&mut self, height: Pixels) {
        let num_items = self.backlog.len();
        let item_sizes = Rc::new(
            (0..num_items)
                .map(|_| Size::new(Pixels::default(), height))
                .collect(),
        );
        self.item_sizes = item_sizes;
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

    // fn scroll_offset(&self) -> Pixels {
    //     self.scroll_handle
    //         .offset()
    //         .along(Axis::Vertical)
    //         .min(px(0.))
    // }

    fn drop_zone(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DropZone<DragData<AnyItem>> {
        let mouse_position = window.mouse_position();
        DropZone::new("backlog-drop")
            .size_full()
            .active(self.drop_active)
            // .inactive_border(true)
            // .rounded_xl()
            .border_t_0()
            .rounded_none()
            .rounded_br_2xl()
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DragData<AnyItem>>, _window, cx| {
                    // let item_opt: Option<&Action> = event.dragged_item().downcast_ref();
                    let is_over = event.bounds.contains(&mouse_position);
                    // let is_action = event
                    //     .dragged_item()
                    //     .downcast_ref::<AnyItem>()
                    //     .is_some_and(|i| i.is_action());
                    // let active = is_over && item_opt.map(|i| matches!(i, Action)).unwrap_or(false);
                    let active = is_over;
                    if active != this.drop_active {
                        this.drop_active = active;
                        cx.notify();
                    }
                },
            ))
            .on_drop(cx.listener(|this, data: &DragData<AnyItem>, _window, cx| {
                if this.drop_active {
                    let id = data.data.id();
                    let db_store = AppDatabaseStore::global(cx);
                    db_store.update(cx, |store, cx| store.backlog_action(id, cx));
                    this.drop_active = false;
                    cx.notify();
                }
            }))
    }

    fn render_items(&self, cx: &Context<Self>) -> impl IntoElement {
        let muted_fg = cx.theme().muted_foreground;
        let completing_items = self.completing_items.clone();
        v_virtual_list(
            cx.entity(),
            "timeline",
            self.item_sizes.clone(),
            move |view, visible_range, _, cx| {
                visible_range
                    .filter_map(|i| {
                        view.backlog.get(i).cloned().map(|action| {
                            let action_id = action.id;
                            let item = AnyItem::Action(action);
                            let title: SharedString = item.title().into();
                            // let colors = ButtonColors::normal(cx.theme().button_primary, cx);
                            let colors = ButtonColors::outline(cx.theme().button_primary, cx);
                            // let colors = ButtonColors::solid(
                            //     cx.theme().button_primary,
                            //     cx.theme().background,
                            //     cx,
                            // );
                            let preview_title = title.clone();
                            let preview_colors = colors;
                            let is_completing = completing_items.contains(&action_id);
                            let drag_data = DragData::new(item)
                                .with_preview(move || {
                                    render_backlog_item_preview(
                                        preview_colors,
                                        preview_title.clone(),
                                        px(64. * 4.),
                                        ITEM_HEIGHT,
                                        muted_fg,
                                    )
                                    .into_any_element()
                                })
                                .with_preview_size(gpui::size(px(64. * 4.), ITEM_HEIGHT));
                            div().px_2().py_1().size_full().child(
                                squircle()
                                    .rounded(px(10.))
                                    // .border_1()
                                    // .border_color(cx.theme().primary)
                                    .button_colors(colors)
                                    .size_full()
                                    .child(
                                        Draggable::new(("backlog-draggable", i as u32), drag_data)
                                            .size_full()
                                            .child(
                                                h_flex()
                                                    .id(("backlog-item", i as u32))
                                                    .size_full()
                                                    .text_ellipsis()
                                                    .overflow_hidden()
                                                    .child(
                                                        h_flex()
                                                            .size_full()
                                                            .px_2()
                                                            .gap_2()
                                                            .child(
                                                                Checkbox::new((
                                                                    "backlog-complete",
                                                                    action_id.as_u128() as u64,
                                                                ))
                                                                .checked(is_completing)
                                                                .tab_stop(false)
                                                                // .occlude()
                                                                .cursor_default()
                                                                .on_click(cx.listener(
                                                                    move |this, _, _window, cx| {
                                                                        this.begin_complete_item(
                                                                            action_id, cx,
                                                                        );
                                                                    },
                                                                )),
                                                            )
                                                            .child(Label::new(title).text_sm()),
                                                    ),
                                            ),
                                    ),
                            )
                        })
                    })
                    .collect()
            },
        )
        .min_w(px(64. * 4. - 4.))
        .track_scroll(&self.scroll_handle)
        .my_1()
    }
}

impl Render for BacklogView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drop_zone(window, cx).child(self.render_items(cx))
    }
}
