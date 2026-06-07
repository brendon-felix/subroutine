use std::{rc::Rc, time::Duration};

use gpui::{
    Context, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels, Render, SharedString,
    Size, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable, VirtualListScrollHandle, button::Button, h_flex, label::Label,
    v_virtual_list,
};
use gpui_transitions::WindowUseTransition;
use simple_core::{AnyItem, Routine};
use uuid::Uuid;

use crate::{
    AppIcon,
    components::{DragData, Draggable},
    stores::{AppDatabaseStore, RoutineDataChanged},
    utils::{ButtonColorizeExt, ButtonColors},
};

const ITEM_HEIGHT: Pixels = px(80.);

fn render_routine_preview(
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
        .border_1()
        .bg(colors.bg)
        .when_some(colors.border, |this, c| this.border_color(c))
        .child(Label::new(title).text_sm().text_color(muted_fg))
}

pub struct RoutinesView {
    scroll_handle: VirtualListScrollHandle,
    item_sizes: Rc<Vec<Size<Pixels>>>,
    routines: Vec<Routine>,
    expanded_items: Vec<Uuid>,
}

impl RoutinesView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let scroll_handle = VirtualListScrollHandle::new();
        let item_sizes = Rc::new(vec![]);
        let db_store = AppDatabaseStore::global(cx);
        let routines = vec![];
        let expanded_items = vec![];
        cx.subscribe(&db_store, |view, store, _: &RoutineDataChanged, cx| {
            view.routines = store.read(cx).routines();
            view.refresh_item_sizes(ITEM_HEIGHT);
            cx.notify();
        })
        .detach();
        Self {
            scroll_handle,
            item_sizes,
            routines,
            expanded_items,
        }
    }

    fn set_item_size(&mut self, index: usize, height: Pixels, cx: &mut Context<Self>) {
        let mut sizes = self.item_sizes.to_vec();
        if index < sizes.len() {
            sizes[index].height = height;
        }
        self.item_sizes = Rc::new(sizes);
        cx.notify();
    }

    fn refresh_item_sizes(&mut self, height: Pixels) {
        let num_items = self.routines.len();
        let item_sizes = Rc::new(
            (0..num_items)
                .map(|_| Size::new(Pixels::default(), height))
                .collect(),
        );
        self.item_sizes = item_sizes;
    }

    fn toggle_expand(&mut self, routine_id: Uuid, cx: &mut Context<Self>) {
        if let Some(index) = self.expanded_items.iter().position(|id| *id == routine_id) {
            self.expanded_items.remove(index);
        } else {
            self.expanded_items.push(routine_id);
        }
        cx.notify();
    }

    fn render_items(&self, cx: &Context<Self>) -> impl IntoElement {
        let muted_fg = cx.theme().muted_foreground;
        v_virtual_list(
            cx.entity(),
            "timeline",
            self.item_sizes.clone(),
            move |view, visible_range, window, cx| {
                visible_range
                    .filter_map(|i| {
                        view.routines.get(i).cloned().map(|routine| {
                            let title: SharedString = routine.title().into();
                            let num_steps = routine.steps().len();
                            let id = routine.id;
                            let duration_opt = routine.duration();
                            let colors = ButtonColors::normal(cx.theme().foreground, cx);
                            let preview_title = title.clone();
                            let preview_colors = colors;
                            let height_transition = window.use_keyed_transition(("height-transition", id.as_u64_pair().1), cx, Duration::from_millis(150), |_, _,| ITEM_HEIGHT);
                            let height = *height_transition.evaluate(window, cx);
                            let drag_data = DragData::new(AnyItem::Routine(routine))
                                .with_label(title.clone())
                                .with_preview(move || {
                                    render_routine_preview(
                                        preview_colors,
                                        preview_title.clone(),
                                        px(64. * 4.),
                                        ITEM_HEIGHT,
                                        muted_fg,
                                    )
                                    .into_any_element()
                                })
                                .with_preview_size(gpui::size(px(64. * 4.), ITEM_HEIGHT));
                            div().size_full().px_2().py_1().child(
                                Draggable::new(("routine-draggable", i as u32), drag_data)
                                    .size_full()
                                    .child(
                                        h_flex()
                                            .id(("routine", i as u32))
                                            .size_full()
                                            .rounded_lg()
                                            .button_colors(colors)
                                            .text_ellipsis()
                                            .overflow_hidden()
                                            .on_click(cx.listener(move |view, event, window, cx| {
                                                view.toggle_expand(id, cx);
                                                height_transition.update(cx, |value, _| {
                                                    *value = ITEM_HEIGHT * 4;
                                                });
                                            }))
                                            .child(
                                                h_flex()
                                                    .size_full()
                                                    .px_2()
                                                    .gap_2()
                                                    .child(
                                                        Button::new(("routine-start", i as u32))
                                                            .small()
                                                            .icon(Icon::new(AppIcon::Play))
                                                            // .text_color(cx.theme().muted_foreground)
                                                            .block_mouse_except_scroll()
                                                            .label("start")
                                                            .on_click(cx.listener(
                                                                move |view, event, window, cx| {
                                                                    AppDatabaseStore::global(cx)
                                                                        .update(cx, |store, cx| {
                                                                            store.instantiate_routine(id, None, cx);
                                                                        });
                                                                },
                                                            )),
                                                    )
                                                    .child(Label::new(title))
                                                    .child(
                                                        Label::new(format!("{} steps", num_steps))
                                                            // .text_color(muted_fg),
                                                            .text_sm(),
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

impl Render for RoutinesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // v_flex().w_full().items_center().child("Routines")
        self.render_items(cx)
    }
}
