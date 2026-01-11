use std::ops::Range;

use gpui::{
    Action, App, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyBinding, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement, Pixels,
    Render, RenderOnce, ScrollStrategy, Size, Styled, Window, actions, div, prelude::FluentBuilder,
    px,
};
use gpui_component::{scroll::Scrollbar, v_flex};
use serde::Deserialize;

use crate::components::custom_list::{
    ListDelegate, SelectedPosition,
    virtual_list::{VirtualListScrollHandle, v_virtual_list},
};

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = custom_list, no_json)]
struct Confirm {
    /// Is confirm with secondary.
    pub secondary: bool,
}

actions!(
    custom_list,
    [
        Cancel,
        SelectUp,
        SelectDown,
        SelectLeft,
        SelectRight,
        SelectFirst,
        SelectLast,
        EnterVisualMode,
    ]
);

pub fn init(cx: &mut App) {
    let context: Option<&str> = Some("CustomList");
    cx.bind_keys([
        KeyBinding::new("escape", Cancel, context),
        KeyBinding::new("enter", Confirm { secondary: false }, context),
        KeyBinding::new("secondary-enter", Confirm { secondary: true }, context),
        KeyBinding::new("up", SelectUp, context),
        KeyBinding::new("k", SelectUp, context),
        KeyBinding::new("down", SelectDown, context),
        KeyBinding::new("j", SelectDown, context),
        KeyBinding::new("left", SelectLeft, context),
        KeyBinding::new("h", SelectLeft, context),
        KeyBinding::new("right", SelectRight, context),
        KeyBinding::new("l", SelectRight, context),
        KeyBinding::new("home", SelectFirst, context),
        KeyBinding::new("g", SelectFirst, context),
        KeyBinding::new("end", SelectLast, context),
        KeyBinding::new("G", SelectLast, context),
        KeyBinding::new("v", EnterVisualMode, context),
        KeyBinding::new("V", EnterVisualMode, context),
    ]);
}

#[derive(Clone)]
pub enum ListEvent {
    Select(usize),
    Confirm(usize),
    Cancel,
}

struct ListOptions {
    // size: Size,
    item_size: Size<Pixels>,
    scrollbar_visible: bool,
    // search_placeholder: Option<SharedString>,
    // max_height: Option<Length>,
    // paddings: EdgesRefinement<DefiniteLength>,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            // size: Size::default(),
            item_size: Size::new(px(0.0), px(50.0)),
            scrollbar_visible: true,
            // max_height: None,
            // search_placeholder: None,
            // paddings: EdgesRefinement::default(),
        }
    }
}

enum ListMode {
    Normal,
    Visual(usize),
}

pub struct ListState<D: ListDelegate> {
    pub focus_handle: FocusHandle,
    options: ListOptions,
    delegate: D,
    mode: ListMode,
    scroll_handle: VirtualListScrollHandle,
    deferred_scroll_to_index: Option<(usize, ScrollStrategy)>,
    num_entries: usize,
    selected_index: Option<usize>,
    mouse_cursor_hidden: bool,
}

impl<D> ListState<D>
where
    D: ListDelegate,
{
    pub fn new(delegate: D, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            options: ListOptions::default(),
            delegate,
            mode: ListMode::Normal,
            scroll_handle: VirtualListScrollHandle::new(),
            deferred_scroll_to_index: None,
            num_entries: 0,
            selected_index: None,
            mouse_cursor_hidden: false,
        }
    }

    pub fn delegate(&self) -> &D {
        &self.delegate
    }

    pub fn delegate_mut(&mut self) -> &mut D {
        &mut self.delegate
    }

    // /// Focus the list, if the list is searchable, focus the search input.
    // pub fn focus(&mut self, window: &mut Window, cx: &mut App) {
    //     self.focus_handle(cx).focus(window);
    // }

    // /// Return true if either the list or the search input is focused.
    // pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
    //     self.focus_handle.is_focused(window)
    // }

    fn _set_selected_index(
        &mut self,
        ix: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_selected_index(ix);
        self.scroll_to_selected_item(window, cx);
    }

    pub fn set_selected_index(
        &mut self,
        ix: Option<usize>,
        // _window: &mut Window,
        // _cx: &mut Context<Self>,
    ) {
        self.selected_index = ix;
        self.delegate.set_selected_index(ix);
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    // pub fn scroll_to_item(
    //     &mut self,
    //     ix: usize,
    //     strategy: ScrollStrategy,
    //     _: &mut Window,
    //     cx: &mut Context<Self>,
    // ) {
    //     if ix == 0 {
    //         let mut offset = self.scroll_handle.base_handle().offset();
    //         offset.y = px(0.);
    //         self.scroll_handle.base_handle().set_offset(offset);
    //         cx.notify();
    //         return;
    //     }
    //     self.deferred_scroll_to_index = Some((ix, strategy));
    //     cx.notify();
    // }

    // pub fn scroll_handle(&self) -> &VirtualListScrollHandle {
    //     &self.scroll_handle
    // }

    pub fn scroll_to_selected_item(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.selected_index {
            self.deferred_scroll_to_index = Some((ix, ScrollStrategy::Top));
            cx.notify();
        }
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // cx.propagate();
        match self.mode {
            ListMode::Visual(_) => {
                if let Some(ix) = self.selected_index {
                    self._set_selected_index(Some(ix), window, cx);
                }
                self.mode = ListMode::Normal;
            }
            ListMode::Normal => {
                self._set_selected_index(None, window, cx);
                self.delegate.cancel(window, cx);
                cx.emit(ListEvent::Cancel);
            }
        }
        cx.notify();
    }

    fn confirm(&mut self, confirm: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if self.num_entries == 0 {
            return;
        }

        if let Some(ix) = self.selected_index {
            self.delegate.set_selected_index(self.selected_index);
            self.delegate.confirm(confirm.secondary, window, cx);
            cx.emit(ListEvent::Confirm(ix));
            cx.notify();
        };
    }

    fn select_item(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_index = Some(ix);
        self.delegate.set_selected_index(Some(ix));
        self.mouse_cursor_hidden = true;
        self.scroll_to_selected_item(window, cx);
        cx.emit(ListEvent::Select(ix));
        cx.notify();
    }

    pub fn select_prev(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.num_entries > 0 {
            let ix = if let Some(selected_ix) = self.selected_index {
                selected_ix.saturating_sub(1)
            } else {
                self.num_entries.saturating_sub(1)
            };
            self.select_item(ix, window, cx);
        }
    }

    pub fn select_next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.num_entries > 0 {
            let ix = if let Some(selected_ix) = self.selected_index {
                self.num_entries.saturating_sub(1).min(selected_ix + 1)
            } else {
                0
            };
            self.select_item(ix, window, cx);
        }
    }

    pub fn select_first(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.num_entries > 0 {
            self.select_item(0, window, cx);
        }
    }

    pub fn select_last(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.num_entries > 0 {
            self.select_item(self.num_entries - 1, window, cx);
        }
    }

    pub fn enter_visual(&mut self) {
        // if let Some(ix) = self.selected_index {
        //     self.delegate.set_selected_index(Some(ix));
        //     cx.notify();
        // }
        self.mode = ListMode::Visual(self.selected_index.unwrap_or(0));
    }

    fn prepare_items_if_needed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.num_entries = self.delegate.items_count(cx)
    }

    fn render_list_item(
        &mut self,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + Styled {
        let selected: Option<SelectedPosition> = match self.mode {
            ListMode::Normal => self.selected_index.and_then(|selected_ix| {
                if selected_ix == ix {
                    Some(SelectedPosition::Single)
                } else {
                    None
                }
            }),
            ListMode::Visual(start_ix) => {
                // position is either Single, FirstRow, MiddleRow, LastRow
                if let Some(selected_ix) = self.selected_index {
                    let first_ix = start_ix.min(selected_ix);
                    let last_ix = start_ix.max(selected_ix);
                    match (first_ix.cmp(&ix), last_ix.cmp(&ix)) {
                        (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => {
                            Some(SelectedPosition::Single)
                        }
                        (std::cmp::Ordering::Equal, _) => Some(SelectedPosition::FirstRow),
                        (_, std::cmp::Ordering::Equal) => Some(SelectedPosition::LastRow),
                        (std::cmp::Ordering::Less, std::cmp::Ordering::Greater) => {
                            Some(SelectedPosition::MiddleRow)
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }
        };

        let ix = ix;
        div()
            .size_full()
            .children(self.delegate.render_item(ix, window, cx).map(|item| {
                item.size_full()
                    .flex()
                    .items_center()
                    .selected_position(selected)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, _window, _cx| {
                            this.selected_index = Some(ix);
                            this.mode = ListMode::Normal;
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseUpEvent, window, cx| {
                            if let Some(ix_before) = this.selected_index {
                                if ix != ix_before {
                                    return;
                                }
                            }
                            this.selected_index = Some(ix);
                            this.mode = ListMode::Normal;
                            this.confirm(
                                &Confirm {
                                    // secondary: event.modifiers().secondary(),
                                    secondary: false,
                                },
                                window,
                                cx,
                            );
                        }),
                    )
            }))
    }

    fn render_items(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scroll_handle = self.scroll_handle.clone();
        let scrollbar_visible = self.options.scrollbar_visible;

        v_flex()
            .h_full()
            .when(self.num_entries == 0, |this| {
                this.child(self.delegate.render_empty(window, cx))
            })
            .when(self.num_entries > 0, {
                |this| {
                    this.child(
                        v_virtual_list(
                            cx.entity(),
                            "virtual-list",
                            self.num_entries,
                            self.options.item_size,
                            move |list, visible_range: Range<usize>, window, cx| {
                                visible_range
                                    .map(|ix| {
                                        list.render_list_item(ix, window, cx)
                                            // .bg(gpui::rgb(rand::random::<u32>()))
                                            .into_any_element()
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        .track_scroll(&scroll_handle)
                        .into_any_element(),
                    )
                }
            })
            .when(scrollbar_visible && self.num_entries > 0, |this| {
                this.child(Scrollbar::vertical(&scroll_handle))
                // .paddings(EdgesRefinement {
                //     right: Some(px(14.0)),
                //     left: None,
                //     top: None,
                //     bottom: None,
                // })
            })
    }
}

impl<D> Focusable for ListState<D>
where
    D: ListDelegate,
{
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<D> EventEmitter<ListEvent> for ListState<D> where D: ListDelegate {}

impl<D> Render for ListState<D>
where
    D: ListDelegate,
{
    #[rustfmt::skip]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare_items_if_needed(window, cx);
        if let Some((ix, strategy)) = self.deferred_scroll_to_index.take() {
            if ix < self.num_entries {
                self.scroll_handle.scroll_to_item(ix, strategy);
            }
        }
        v_flex()
            .key_context("CustomList")
            .id("custom-list-state")
            .track_focus(&self.focus_handle)
            .size_full()
            .relative()
            .overflow_hidden()
            .on_action(cx.listener(|this, _: &Cancel, window, cx|      this.cancel(window, cx)))
            .on_action(cx.listener(|this, c: &Confirm, window, cx|     this.confirm(c, window, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, window, cx|  this.select_next(window, cx)))
            .on_action(cx.listener(|this, _: &SelectUp, window, cx|    this.select_prev(window, cx)))
            .on_action(cx.listener(|this, _: &SelectFirst, window, cx| this.select_first(window, cx)))
            .on_action(cx.listener(|this, _: &SelectLast, window, cx|  this.select_last(window, cx)))
            .on_action(cx.listener(|this, _: &EnterVisualMode, _window, _cx| this.enter_visual()))
            .child(self.render_items(window, cx))
    }
}

#[derive(IntoElement)]
pub struct List<D: ListDelegate + 'static> {
    state: Entity<ListState<D>>,
    // options: ListOptions,
}

impl<D> List<D>
where
    D: ListDelegate + 'static,
{
    /// Create a new List element with the given ListState entity.
    pub fn new(state: &Entity<ListState<D>>) -> Self {
        Self {
            state: state.clone(),
            // options: ListOptions::default(),
        }
    }
}

impl<D> RenderOnce for List<D>
where
    D: ListDelegate + 'static,
{
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // self.state.update(cx, |state, _| {
        //     state.options = self.options;
        // });

        self.state.clone()
    }
}
