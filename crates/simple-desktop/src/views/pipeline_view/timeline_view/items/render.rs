use gpui::{
    AnyElement, ClickEvent, Context, ElementId, Entity, FocusHandle, IntoElement, Pixels,
    SharedString, Window, div, prelude::FluentBuilder, px,
};
use gpui::{AppContext, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled};
use gpui_component::input::{Input, InputState, Position};
use gpui_component::{
    ActiveTheme, Icon, IconName, h_flex, label::Label, menu::ContextMenuExt, skeleton::Skeleton,
    v_flex,
};

use super::super::TimelineView;
use super::super::timeline::HOUR_DIVIDER_HEIGHT;
use super::{
    ATTACHED_ITEM_LEFT, ActiveDropState, ActiveResizeState, FALLBACK_ITEM_DURATION,
    ItemTimelineBounds, META_ROW_HEIGHT, RESCHEDULE_TRANSITION_DURATION, RESIZE_HANDLE_HEIGHT,
    ResizeDragData, ResizeEdge, ResizeGhost, SLOT_GAP, STICKY_TITLE_HEIGHT, STICKY_TITLE_PADDING,
    SlotLayout, attach_transition, render_item_preview,
};
use crate::components::Checkbox;
use crate::views::format_item_meta;
use crate::views::pipeline_view::{action_context_menu, event_context_menu};
use crate::{
    components::{DragData, Draggable},
    utils::ButtonColorizeExt,
};
use simple_core::AnyItem;
use uuid::Uuid;

impl TimelineView {
    /// Width of the item area for the current frame — fills all available space
    /// to the right of `ATTACHED_ITEM_LEFT` (minus `ITEMS_RIGHT_GAP`).
    pub(super) fn item_area_width(&self) -> Pixels {
        use super::ITEMS_RIGHT_GAP;
        if let Some(bounds) = self.bounds {
            let available = bounds.size.width - ATTACHED_ITEM_LEFT - ITEMS_RIGHT_GAP;
            if available > px(0.) {
                return available;
            }
        }
        // Bounds not yet captured (first frame) — return a placeholder.
        px(400.)
    }

    pub(crate) fn render_skeleton_items(&self) -> Vec<impl IntoElement> {
        use chrono::Timelike;
        let now = chrono::Local::now();
        let hour_start = now
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap();

        let h = self.duration_to_height(chrono::Duration::minutes(30));
        (0..2)
            .map(|i| {
                let time = hour_start + chrono::Duration::hours(2) * i;
                let y = self.time_to_offset(time);
                div()
                    .absolute()
                    .top(y)
                    .w_full()
                    .h(h)
                    .pl_16()
                    .child(Skeleton::new().w_64().h_full().rounded_lg().opacity(0.7))
            })
            .collect()
    }

    /// Get or create a title input for the given item, subscribing for change/enter/blur events.
    pub(super) fn get_or_create_title_input(
        &mut self,
        id: Uuid,
        title: String,
        is_action: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        use super::TitleEditState;
        use gpui_component::input::InputEvent;

        // Ensure edit state exists.
        self.title_edit_states
            .entry(id)
            .or_insert_with(|| TitleEditState::new(title.clone()));

        // Return existing input if already created.
        if let Some(existing) = self.title_inputs.get(&id) {
            return existing.clone();
        }

        // Create new InputState.
        let input = cx.new(|cx| InputState::new(window, cx).default_value(title));
        self.title_inputs.insert(id, input.clone());

        // Subscribe: Change → update parse state.
        self._title_subscriptions.push(cx.subscribe(
            &input,
            move |this, _, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    let value = this
                        .title_inputs
                        .get(&id)
                        .map(|e| e.read(cx).value().to_string())
                        .unwrap_or_default();
                    if is_action {
                        if let Some(state) = this.title_edit_states.get_mut(&id) {
                            state.update_action(value);
                        }
                    } else if let Some(state) = this.title_edit_states.get_mut(&id) {
                        state.update_event(value);
                    }
                }
            },
        ));

        // Subscribe: PressEnter → commit.
        self._title_subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    if is_action {
                        this.commit_action(id, window, cx);
                    } else {
                        this.commit_event(id, window, cx);
                    }
                }
            },
        ));

        // Subscribe: Blur → discard draft or revert existing item and exit editing mode.
        self._title_subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::Blur = event {
                    if this.draft_item_ids.contains(&id) {
                        // Discard the draft entirely — it was never saved.
                        this.items.retain(|i| i.item.id() != id);
                        this.draft_item_ids.remove(&id);
                        this.title_inputs.remove(&id);
                        this.title_edit_states.remove(&id);
                        this.editing_items.remove(&id);
                        cx.notify();
                    } else {
                        this.editing_items.remove(&id);
                        this.revert_item(id, window, cx);
                    }
                }
            },
        ));

        input
    }

    /// Render the hybrid title element: a plain label when not editing (double-click to start),
    /// and a styled input with visible border/background while editing.
    pub(super) fn render_title_input(
        &self,
        item_id: Uuid,
        title_input: Entity<InputState>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_editing = self.editing_items.contains(&item_id);

        let edit_state = self.title_edit_states.get(&item_id);
        let _parse_error = edit_state.map(|s| s.parse_error).unwrap_or(false);

        if !is_editing {
            // Label mode: looks like plain text, double-click enters edit mode.
            let title_text: SharedString = title_input.read(cx).value().to_string().into();
            let activate = cx.listener(move |this, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                if event.click_count() != 2 {
                    return;
                }
                this.editing_items.insert(item_id);
                if let Some(input) = this.title_inputs.get(&item_id) {
                    input.update(cx, |state, cx| {
                        state.set_cursor_position(Position::new(0, u32::MAX), window, cx);
                    });
                }
                cx.notify();
            });
            return div()
                .id(("title-label", item_id.as_u128() as u64))
                .w_full()
                .flex()
                .items_center()
                .px_1()
                .on_click(activate)
                .child(Label::new(title_text).text_sm())
                .into_any_element();
        }

        // Edit mode: input with background and border.
        div()
            .id(("title-input-wrap", item_id.as_u128() as u64))
            .w_full()
            .child(
                Input::new(&title_input)
                    .text_sm()
                    .w_full()
                    .py_0()
                    .px_1()
                    .appearance(false)
                    .bordered(false),
            )
            .into_any_element()
    }

    pub(super) fn render_attached_item(
        &mut self,
        item_id: Uuid,
        item_element_id: ElementId,
        item_colors: crate::utils::ButtonColors,
        _item_title: SharedString,
        item_any: AnyItem,
        title_input: Entity<InputState>,
        focus_handle: FocusHandle,
        is_completing: bool,
        layout: Option<SlotLayout>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        use gpui::KeyDownEvent;

        let id = item_element_id.clone();
        let colors = item_colors;
        let any_item = item_any;
        let preview_title: SharedString = any_item.title().into();

        let total_w = self.item_area_width();
        let half_gap = SLOT_GAP * 0.5;
        let scroll = self.scroll_offset();

        let target = if let Some(ref layout) = layout {
            ItemTimelineBounds {
                elapsed_secs: (layout.visual_start - self.start).num_seconds() as f64,
                duration_secs: (layout.visual_end - layout.visual_start).num_seconds() as f64,
                left_fraction: layout.column_index as f32 / layout.total_columns as f32,
                width_fraction: 1.0 / layout.total_columns as f32,
            }
        } else {
            let time = any_item.time_local()?;
            ItemTimelineBounds {
                elapsed_secs: (time - self.start).num_seconds() as f64,
                duration_secs: any_item
                    .duration()
                    .unwrap_or(FALLBACK_ITEM_DURATION)
                    .num_seconds() as f64,
                left_fraction: 0.0,
                width_fraction: 1.0,
            }
        };

        let bounds_t = attach_transition(
            ("item-bounds", any_item.truncated_id()),
            target,
            RESCHEDULE_TRANSITION_DURATION,
            window,
            cx,
        );
        // Skip animation while the user is actively dragging a resize handle so
        // the item tracks the cursor instantly rather than lagging behind.
        let is_being_resized = self
            .active_resize
            .as_ref()
            .map_or(false, |r| r.item_id == item_id);
        if is_being_resized {
            bounds_t.jump_to(target, cx);
        } else {
            let changed = bounds_t.update(cx, |val, _| *val = target);
            if changed {
                cx.notify();
            }
        }
        let anim = *bounds_t.evaluate(window, cx);

        let y = self.hour_height * (anim.elapsed_secs / 3600.0) as f32
            + HOUR_DIVIDER_HEIGHT / 2.0
            + half_gap
            + scroll;
        let h = self.hour_height * (anim.duration_secs / 3600.0) as f32 - SLOT_GAP;
        let item_left = ATTACHED_ITEM_LEFT + total_w * anim.left_fraction + half_gap;
        let item_w = total_w * anim.width_fraction - SLOT_GAP;

        let fg = cx.theme().muted_foreground;

        let too_short = false;
        let is_editing = self.editing_items.contains(&item_id);

        let meta_text = format_item_meta(&any_item);
        let show_meta = meta_text.is_some() && h >= STICKY_TITLE_HEIGHT + META_ROW_HEIGHT + px(6.);
        let title_block_h = if show_meta {
            STICKY_TITLE_HEIGHT + META_ROW_HEIGHT
        } else {
            STICKY_TITLE_HEIGHT
        };

        let title_y = (y + STICKY_TITLE_PADDING)
            .max(STICKY_TITLE_PADDING)
            .min(y + h - title_block_h - STICKY_TITLE_PADDING);
        let title_rel_y = title_y - y;

        let is_action = matches!(any_item, AnyItem::Action(_));
        let ring_color = cx.theme().ring;
        let inner = div()
            .id(id)
            .track_focus(&focus_handle.clone().tab_stop(true))
            .relative()
            .size_full()
            .rounded_lg()
            .button_colors(colors)
            .overflow_hidden()
            .when(focus_handle.is_focused(window), |el| {
                el.border_color(ring_color)
            })
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if event.is_held {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "enter" if !is_editing => {
                        cx.stop_propagation();
                        this.editing_items.insert(item_id);
                        if let Some(input) = this.title_inputs.get(&item_id) {
                            input.update(cx, |state, cx| {
                                state.set_cursor_position(Position::new(0, u32::MAX), window, cx);
                            });
                        }
                        cx.notify();
                    }
                    "space" if !is_editing && is_action => {
                        cx.stop_propagation();
                        this.begin_complete_item(item_id, window, cx);
                    }
                    "escape" if is_editing => {
                        cx.stop_propagation();
                        focus_handle.focus(window, cx);
                    }
                    "escape" => {
                        cx.stop_propagation();
                        this.focus_handle.focus(window, cx);
                    }
                    _ => {}
                }
            }))
            .child(
                v_flex()
                    .absolute()
                    .top(title_rel_y)
                    .left(px(0.))
                    .w_full()
                    .h(title_block_h)
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .w_full()
                            .h(STICKY_TITLE_HEIGHT)
                            .px_2()
                            .gap_2()
                            .when(!too_short && is_action, |this| {
                                let action_id = item_id;
                                this.child(
                                    Checkbox::new(("complete", item_id.as_u128() as u64))
                                        .checked(is_completing)
                                        .tab_stop(false)
                                        // .occlude()
                                        .cursor_default()
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.begin_complete_item(action_id, window, cx);
                                        })),
                                )
                            })
                            .when(!too_short && !is_action, |this| {
                                this.child(
                                    Icon::new(IconName::Calendar)
                                        .size_4()
                                        .flex_shrink_0()
                                        .text_color(cx.theme().muted_foreground),
                                )
                            })
                            .when(!too_short, |this| {
                                this.child(self.render_title_input(
                                    item_id,
                                    title_input.clone(),
                                    window,
                                    cx,
                                ))
                            }),
                    )
                    .when(show_meta, |this| {
                        this.child(h_flex().w_full().h(META_ROW_HEIGHT).px_3().when_some(
                            meta_text,
                            |this, meta| {
                                this.child(
                                    Label::new(meta)
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                )
                            },
                        ))
                    }),
            );

        let top_handle = div()
            .id(("resize-top", item_id.as_u64_pair().1))
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h(RESIZE_HANDLE_HEIGHT)
            .cursor_row_resize()
            .on_drag(
                ResizeDragData {
                    item_id,
                    edge: ResizeEdge::Top,
                },
                |_, _, _, cx| cx.new(|_| ResizeGhost),
            );

        let bottom_handle = div()
            .id(("resize-bottom", item_id.as_u64_pair().1))
            .absolute()
            .bottom_0()
            .left_0()
            .w_full()
            .h(RESIZE_HANDLE_HEIGHT)
            .cursor_row_resize()
            .on_drag(
                ResizeDragData {
                    item_id,
                    edge: ResizeEdge::Bottom,
                },
                |_, _, _, cx| cx.new(|_| ResizeGhost),
            );

        let positioned = div().absolute().top(y).h(h).left(item_left).w(item_w);

        Some(if is_editing {
            positioned
                .child(inner.w_full())
                .child(top_handle)
                .child(bottom_handle)
                .into_any_element()
        } else {
            let drag_data = DragData::new(any_item.clone())
                .with_label(any_item.title())
                .with_preview(move || {
                    render_item_preview(colors, preview_title.clone(), px(64. * 4.), h, fg)
                        .into_any_element()
                })
                .with_preview_size(gpui::size(px(64. * 4.), h));
            positioned
                .child(
                    Draggable::new((item_element_id.clone(), "draggable"), drag_data)
                        .h_full()
                        .w_full()
                        .child(inner)
                        .on_aux_click(|_, _, cx| cx.stop_propagation())
                        .context_menu(move |menu, window, cx| match &any_item {
                            AnyItem::Action(a) => action_context_menu(a.id)(menu, window, cx),
                            AnyItem::Event(e) => event_context_menu(e.id)(menu, window, cx),
                        }),
                )
                .child(top_handle)
                .child(bottom_handle)
                .into_any_element()
        })
    }

    pub(crate) fn render_attached_items(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        use super::TransitionState;
        use gpui::Entity;

        // Step 1: collect item data (immutable borrow of self.items).
        let item_data: Vec<(
            usize,
            Uuid,
            ElementId,
            crate::utils::ButtonColors,
            SharedString,
            AnyItem,
            TransitionState,
            FocusHandle,
        )> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                (
                    i,
                    item.item.id(),
                    item.element_id.clone(),
                    item.colors(cx),
                    SharedString::from(item.item.title().to_string()),
                    item.item.clone(),
                    item.transition_state,
                    item.focus_handle.clone(),
                )
            })
            .collect();

        // Step 2: compute slot-based layouts for all items at once.
        let any_items: Vec<AnyItem> = item_data
            .iter()
            .map(|(_, _, _, _, _, any, _, _)| any.clone())
            .collect();
        let slot_layouts = self.compute_slot_layouts(&any_items);

        // Step 3: pre-create title inputs (mutable borrow).
        let mut title_inputs: Vec<Option<Entity<InputState>>> = Vec::with_capacity(item_data.len());
        for (_, item_id, _, _, title, any_item, transition_state, _) in &item_data {
            if matches!(
                transition_state,
                TransitionState::Attached | TransitionState::Completing
            ) {
                let is_action = matches!(any_item, AnyItem::Action(_));
                let input = self.get_or_create_title_input(
                    *item_id,
                    title.to_string(),
                    is_action,
                    window,
                    cx,
                );
                title_inputs.push(Some(input));
            } else {
                title_inputs.push(None);
            }
        }

        // Step 4: render.
        item_data
            .into_iter()
            .zip(title_inputs)
            .zip(slot_layouts)
            .filter_map(
                |(
                    (
                        (
                            _i,
                            item_id,
                            element_id,
                            colors,
                            title,
                            any_item,
                            transition_state,
                            focus_handle,
                        ),
                        title_input,
                    ),
                    layout,
                )| {
                    let input = title_input?;
                    self.render_attached_item(
                        item_id,
                        element_id,
                        colors,
                        title,
                        any_item,
                        input,
                        focus_handle,
                        transition_state == TransitionState::Completing,
                        layout,
                        window,
                        cx,
                    )
                    .map(|e| e.into_any_element())
                },
            )
            .collect()
    }

    pub(crate) fn render_active_resize(
        &self,
        resize: &ActiveResizeState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let half_gap = SLOT_GAP * 0.5;
        let y = self.time_to_offset(resize.new_time) + half_gap;
        let h = self.duration_to_height(resize.new_end - resize.new_time) - SLOT_GAP;
        let color = cx.theme().drag_border;
        let label_time = resize.new_time.format("%-I:%M").to_string();
        let label_end = resize.new_end.format("%-I:%M").to_string();
        let label_text = match resize.edge {
            ResizeEdge::Top => label_time,
            ResizeEdge::Bottom => label_end,
        };
        div()
            .absolute()
            .top(y)
            .h(h)
            .left(ATTACHED_ITEM_LEFT + half_gap)
            .w(self.item_area_width() - SLOT_GAP)
            .rounded_lg()
            .border_1()
            .border_dashed()
            .border_color(color)
            .map(|this| match resize.edge {
                ResizeEdge::Top => this.flex().items_start().justify_start().pl_2().pt_1(),
                ResizeEdge::Bottom => this.flex().items_end().justify_start().pl_2().pb_1(),
            })
            .child(
                div()
                    .px(px(6.))
                    .bg(cx.theme().background.alpha(0.85))
                    .rounded_md()
                    .text_sm()
                    .text_color(color)
                    .child(label_text),
            )
    }

    pub(crate) fn render_active_drop(
        &self,
        drop_info: ActiveDropState,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let half_gap = SLOT_GAP * 0.5;
        let division = self.current_hour_division();
        let visual_end = match drop_info.drop_duration {
            Some(d) => {
                let actual_end = drop_info.drop_time + d;
                let ve_raw = division.ceil_division(actual_end);
                if ve_raw <= drop_info.slot_visual_start {
                    drop_info.slot_visual_end
                } else {
                    ve_raw
                }
            }
            None => drop_info.slot_visual_end,
        };
        let y = self.time_to_offset(drop_info.slot_visual_start) + half_gap;
        let h = self.duration_to_height(visual_end - drop_info.slot_visual_start) - SLOT_GAP;
        let area_w = self.item_area_width();
        let col_w = area_w * (1.0 / drop_info.total_columns as f32);
        let col_x = col_w * drop_info.column_index as f32;
        let outline_left = ATTACHED_ITEM_LEFT + col_x + half_gap;
        let color = cx.theme().drag_border;
        let label_text = drop_info.drop_time.format("%-I:%M").to_string();
        div()
            .absolute()
            .top(y)
            .h(h)
            .left(px(0.))
            .w_full()
            .child(
                div()
                    .absolute()
                    .top(px(0.))
                    .left(px(0.))
                    .w(outline_left)
                    .h_full()
                    .flex()
                    .items_start()
                    .justify_center()
                    .child(
                        div()
                            .px(px(7.))
                            .bg(cx.theme().background.alpha(0.8))
                            .rounded_lg()
                            .child(label_text)
                            .text_sm()
                            .text_color(color),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top(px(0.))
                    .left(outline_left)
                    .w(col_w - SLOT_GAP)
                    .h_full()
                    .rounded_lg()
                    .border_1()
                    .border_dashed()
                    .border_color(color),
            )
    }
}
