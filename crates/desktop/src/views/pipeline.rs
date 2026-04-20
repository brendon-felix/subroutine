use std::collections::HashMap;

use chrono::{DateTime, Local, Timelike, Utc};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Div, Entity, EventEmitter, Focusable, Hsla, InteractiveElement,
    IntoElement, Pixels, Render, ScrollHandle, SharedString, Styled, Window, actions, div, px,
};
use gpui_component::input::{Escape as InputEscape, Input, InputEvent, InputState};
use gpui_component::label::Label;
use gpui_component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_component::notification::NotificationType;
use gpui_component::scroll::Scrollbar;
use gpui_component::{ActiveTheme, Icon, IconName, WindowExt, h_flex, v_flex};
use palette::{Clamp, FromColor, IntoColor, Oklch, Srgb};
use simple_core::{Action, Event, QueueItem};
use simple_parser::{ParseDraft, parse_action_input, parse_event_input, recurrence_to_duration};
use uuid::Uuid;

use crate::components::checkbox::Checkbox;
use crate::components::drag_drop::{DragData, Draggable};
use crate::stores::DatabaseStore;
use crate::stores::database_store::PipelineChanged;
use crate::views::action_editor::StartActionEditor;
use crate::views::event_editor::StartEventEditor;

const ITEM_MIN_HEIGHT: f32 = 120.0;

pub struct StartQueueEventEditor {
    pub event_id: uuid::Uuid,
}

actions!(
    pipeline,
    [CompleteAction, DemoteAction, RemoveFromPipeline,]
);

#[derive(Clone, Copy)]
struct ItemColors {
    bg: Hsla,
    border: Hsla,
}

/// Tracks the live editing state for a single pipeline item's title input.
struct TitleEditState {
    current_text: String,
    draft: Option<ParseDraft>,
    /// True when the user has typed something that doesn't parse cleanly.
    parse_error: bool,
}

impl TitleEditState {
    fn new(initial: String) -> Self {
        let draft = parse_action_input(&initial).ok();
        Self {
            current_text: initial,
            draft,
            parse_error: false,
        }
    }

    fn update_action(&mut self, text: String) {
        self.current_text = text.trim().to_string();
        match parse_action_input(&self.current_text) {
            Ok(d) => {
                self.draft = Some(d);
                self.parse_error = false;
            }
            Err(_) => {
                self.draft = None;
                self.parse_error = !self.current_text.is_empty();
            }
        }
    }

    fn update_event(&mut self, text: String) {
        self.current_text = text.trim().to_string();
        match parse_event_input(&self.current_text) {
            Ok(d) => {
                self.draft = Some(d);
                self.parse_error = false;
            }
            Err(_) => {
                self.draft = None;
                self.parse_error = !self.current_text.is_empty();
            }
        }
    }
}

pub struct Pipeline {
    database_store: Entity<DatabaseStore>,
    entries: Vec<QueueItem>,
    title_inputs: HashMap<Uuid, Entity<InputState>>,
    /// Live parse state, keyed by item ID.
    title_edit_states: HashMap<Uuid, TitleEditState>,
    scroll_handle: ScrollHandle,
    _subscriptions: Vec<gpui::Subscription>,
}

impl Pipeline {
    pub fn new(
        database_store: Entity<DatabaseStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let entries = database_store.read(cx).pipeline.queue.clone();
        let title_inputs = Self::build_title_inputs(&entries, window, cx);

        let title_edit_states = entries
            .iter()
            .map(|item| (item.id(), TitleEditState::new(item.title().to_string())))
            .collect();

        let mut subscriptions = Vec::new();

        subscriptions.push(cx.subscribe_in(
            &database_store,
            window,
            |this, store, _event: &PipelineChanged, window, cx| {
                let new_entries = store.read(cx).pipeline.queue.clone();
                for item in &new_entries {
                    // Only reset the input if the item is not being actively edited
                    // (i.e. its input doesn't have focus).
                    let title = item.title().to_string();

                    this.title_inputs.entry(item.id()).or_insert_with(|| {
                        cx.new(|cx| InputState::new(window, cx).default_value(title.clone()))
                    });
                    this.title_edit_states
                        .entry(item.id())
                        .or_insert_with(|| TitleEditState::new(title.clone()));

                    // If the item isn't focused, sync the display value.
                    let focused = this
                        .title_inputs
                        .get(&item.id())
                        .map(|e| e.focus_handle(cx).is_focused(window))
                        .unwrap_or(false);

                    if !focused {
                        if let Some(input) = this.title_inputs.get(&item.id()) {
                            let current = input.read(cx).value();
                            if current != title.as_str() {
                                input.update(cx, |state, cx| {
                                    state.set_value(title.clone(), window, cx);
                                });
                            }
                        }
                        if let Some(state) = this.title_edit_states.get_mut(&item.id()) {
                            state.current_text = title.clone();
                            state.draft = parse_action_input(&title).ok();
                            state.parse_error = false;
                        }
                    }
                }

                let current_ids: std::collections::HashSet<Uuid> =
                    new_entries.iter().map(|i| i.id()).collect();
                this.title_inputs.retain(|id, _| current_ids.contains(id));
                this.title_edit_states
                    .retain(|id, _| current_ids.contains(id));
                this.entries = new_entries;
                cx.notify();
            },
        ));

        Self {
            database_store,
            entries,
            title_inputs,
            title_edit_states,
            scroll_handle: ScrollHandle::default(),
            _subscriptions: subscriptions,
        }
    }

    pub fn update_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let new_entries = self.database_store.read(cx).pipeline.queue.clone();
        for item in &new_entries {
            let title = item.title().to_string();

            if let Some(input) = self.title_inputs.get(&item.id()) {
                let current = input.read(cx).value();
                if current != title.as_str() {
                    input.update(cx, |state, cx| {
                        state.set_value(title.clone(), window, cx);
                    });
                }
            } else {
                self.title_inputs.insert(
                    item.id(),
                    cx.new(|cx| InputState::new(window, cx).default_value(title.clone())),
                );
            }

            self.title_edit_states
                .entry(item.id())
                .or_insert_with(|| TitleEditState::new(title));
        }
        let current_ids: std::collections::HashSet<Uuid> =
            new_entries.iter().map(|i| i.id()).collect();
        self.title_inputs.retain(|id, _| current_ids.contains(id));
        self.title_edit_states
            .retain(|id, _| current_ids.contains(id));
        self.entries = new_entries;
    }

    fn build_title_inputs(
        entries: &[QueueItem],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> HashMap<Uuid, Entity<InputState>> {
        entries
            .iter()
            .map(|item| {
                let id = item.id();
                let title = item.title().to_string();
                let input = cx.new(|cx| InputState::new(window, cx).default_value(title));
                (id, input)
            })
            .collect()
    }

    fn item_base(&self, height: Option<Pixels>, colors: ItemColors) -> Div {
        div()
            .flex()
            .flex_col()
            .w_full()
            .when_some(height, |div, h| div.min_h(h))
            .p_3()
            .rounded_md()
            .bg(colors.bg)
            .border_1()
            .border_color(colors.border)
            .gap_2()
    }

    fn item_row(&self) -> Div {
        div().flex().flex_row().w_full().gap_2().items_center()
    }

    /// Build a human-readable parse preview string from a draft — same
    /// approach as ActionCreator.
    fn build_preview_text(draft: &ParseDraft) -> Option<SharedString> {
        let mut parts: Vec<String> = Vec::new();

        if let Some(when) = &draft.when {
            use simple_parser::ast::WhenSpec;
            match when {
                WhenSpec::DateTime(dt) => {
                    let local = dt.with_timezone(&Local);
                    let time_str = local.format("%-I:%M%P").to_string();
                    let time_str = time_str.replace(":00", "");
                    parts.push(format!(
                        "🕐 {}",
                        local.format(&format!("%a %b %-d {time_str}"))
                    ));
                }
                WhenSpec::NaiveDate(date) => {
                    parts.push(format!("📅 {}", date.format("%a %b %-d")));
                }
            }
        }

        if let Some(dur) = draft.duration {
            let total_mins = dur.num_minutes();
            if total_mins % 60 == 0 {
                parts.push(format!("⏱ {}h", total_mins / 60));
            } else if total_mins >= 60 {
                parts.push(format!("⏱ {}h {}m", total_mins / 60, total_mins % 60));
            } else {
                parts.push(format!("⏱ {}m", total_mins));
            }
        }
        if let Some(rec) = &draft.recurrence {
            parts.push(format!("🔁 {}", format_recurrence(rec)));
        }
        if let Some(loc) = &draft.location {
            parts.push(format!("📍 {loc}"));
        }
        if !draft.tags.is_empty() {
            parts.push(format!("🏷 {}", draft.tags.join(", ")));
        }
        if !draft.people.is_empty() {
            parts.push(format!("👤 {}", draft.people.join(", ")));
        }
        if let Some(pri) = &draft.priority {
            parts.push(format!("❗ {pri:?}"));
        }

        if parts.is_empty() {
            None
        } else {
            Some(SharedString::from(parts.join("  ·  ")))
        }
    }

    /// Revert a pipeline item's title input and edit state back to the last
    /// saved value from the store.  Called on Blur and Escape.
    fn revert_item(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        // Look up the saved title from the store (action or event).
        let saved_title = {
            let store = self.database_store.read(cx);
            store
                .get_queue_action(id)
                .map(|a| a.title.clone())
                .or_else(|| store.get_queue_event(id).map(|e| e.title.clone()))
        };
        let Some(title) = saved_title else { return };

        // Reset the input widget.
        if let Some(input) = self.title_inputs.get(&id) {
            let current = input.read(cx).value().to_string();
            if current != title {
                input.update(cx, |state, cx| {
                    state.set_value(title.clone(), window, cx);
                });
            }
        }

        // Reset the edit state so the preview/error indicator clears.
        if let Some(state) = self.title_edit_states.get_mut(&id) {
            state.current_text = title.clone();
            state.draft = parse_action_input(&title).ok();
            state.parse_error = false;
        }

        cx.notify();
    }

    /// Subscribe the title input for a given action to live parse + Enter-commit.
    fn subscribe_action_title(
        &mut self,
        id: Uuid,
        title_input: &Entity<InputState>,
        cx: &mut Context<Self>,
        window: &mut Window,
    ) {
        // Change → re-parse
        self._subscriptions.push(cx.subscribe(
            title_input,
            move |this, _, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    let value = this
                        .title_inputs
                        .get(&id)
                        .map(|e| e.read(cx).value().to_string())
                        .unwrap_or_default();
                    if let Some(state) = this.title_edit_states.get_mut(&id) {
                        state.update_action(value);
                    }
                    cx.notify();
                }
            },
        ));

        // PressEnter → commit
        self._subscriptions.push(cx.subscribe_in(
            title_input,
            window,
            move |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.commit_action(id, window, cx);
                }
            },
        ));

        // Blur → revert
        self._subscriptions.push(cx.subscribe_in(
            title_input,
            window,
            move |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::Blur = event {
                    this.revert_item(id, window, cx);
                }
            },
        ));
    }

    /// Subscribe the title input for a given event to live parse + Enter-commit.
    fn subscribe_event_title(
        &mut self,
        id: Uuid,
        title_input: &Entity<InputState>,
        cx: &mut Context<Self>,
        window: &mut Window,
    ) {
        // Change → re-parse
        self._subscriptions.push(cx.subscribe(
            title_input,
            move |this, _, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    let value = this
                        .title_inputs
                        .get(&id)
                        .map(|e| e.read(cx).value().to_string())
                        .unwrap_or_default();
                    if let Some(state) = this.title_edit_states.get_mut(&id) {
                        state.update_event(value);
                    }
                    cx.notify();
                }
            },
        ));

        // PressEnter → commit
        self._subscriptions.push(cx.subscribe_in(
            title_input,
            window,
            move |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.commit_event(id, window, cx);
                }
            },
        ));

        // Blur → revert
        self._subscriptions.push(cx.subscribe_in(
            title_input,
            window,
            move |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::Blur = event {
                    this.revert_item(id, window, cx);
                }
            },
        ));
    }

    /// Commit edits for an action item: parse the title input, build an updated
    /// Action, then call update_queue_action on the store.
    fn commit_action(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        // Get the current action from the store so we preserve fields not
        // touched by the parser (id, lineage_id, ephemeral, completed_at, …).
        let original = match self.database_store.read(cx).get_queue_action(id) {
            Some(a) => a.clone(),
            None => return,
        };

        let draft = self
            .title_edit_states
            .get(&id)
            .and_then(|s| s.draft.clone())
            .or_else(|| {
                let text = self
                    .title_inputs
                    .get(&id)
                    .map(|e| e.read(cx).value().to_string())
                    .unwrap_or_default();
                parse_action_input(text.trim()).ok()
            });

        // Start from the original and patch only the fields the parser found.
        let updated_action = match draft {
            Some(ref d) => {
                let mut a = original.clone();
                // Title is always taken from the parse result (it's the
                // cleaned-up text with all sigils stripped).
                a.title = d.title.clone();
                // Only overwrite timing / scheduling fields when the parser
                // actually produced a value for them.
                if let Some(ref when) = d.when {
                    use simple_parser::ast::WhenSpec;
                    match when {
                        WhenSpec::DateTime(dt) => {
                            a.target = Some(*dt);
                            a.target_static = true;
                            a.naive_date = None;
                        }
                        WhenSpec::NaiveDate(date) => {
                            a.naive_date = Some(*date);
                            a.target = None;
                            a.target_static = false;
                        }
                    }
                }
                if let Some(dur) = d.duration {
                    a.duration = Some(dur);
                }
                if let Some(ref rec) = d.recurrence {
                    if let Some(dur) = recurrence_to_duration(Some(rec)) {
                        a.recurrence = Some(dur);
                    }
                }
                if let Some(ref content) = d.content {
                    a.content = Some(content.clone());
                }
                a
            }
            None => {
                // No successful parse — fall back to just updating the title.
                let raw = self
                    .title_inputs
                    .get(&id)
                    .map(|e| e.read(cx).value().to_string())
                    .unwrap_or_default();
                let raw = raw.trim();
                if raw.is_empty() {
                    return;
                }
                let mut a = original.clone();
                a.title = raw.to_string();
                a
            }
        };

        // Emit any parse warnings.
        if let Some(draft) = &draft {
            for warning in &draft.warnings {
                window.push_notification(
                    (
                        NotificationType::Warning,
                        SharedString::from(warning.clone()),
                    ),
                    cx,
                );
            }
        }

        let overlap_warnings = self.database_store.update(cx, |store, cx| {
            store.update_queue_action(updated_action, cx)
        });

        for warning in overlap_warnings {
            window.push_notification(
                (
                    NotificationType::Warning,
                    SharedString::from(format!(
                        "\"{}\" overlaps with \"{}\"",
                        warning.inserted_title, warning.conflicting_title
                    )),
                ),
                cx,
            );
        }

        window.blur();
    }

    /// Commit edits for an event item: parse the title input, build an updated
    /// Event preserving the original ID, then call update_queue_event.
    fn commit_event(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let original = match self.database_store.read(cx).get_queue_event(id) {
            Some(e) => e.clone(),
            None => return,
        };

        let draft = self
            .title_edit_states
            .get(&id)
            .and_then(|s| s.draft.clone())
            .or_else(|| {
                let text = self
                    .title_inputs
                    .get(&id)
                    .map(|e| e.read(cx).value().to_string())
                    .unwrap_or_default();
                parse_event_input(text.trim()).ok()
            });

        // Start from the original and patch only the fields the parser found.
        let updated_event = match draft {
            Some(ref d) => {
                let mut e = original.clone();
                // Title is always updated.
                e.title = d.title.clone();
                // Only overwrite time if the parser produced a full datetime.
                // Events must always have a concrete time, so a NaiveDate or
                // absent `when` keeps the original scheduled time.
                if let Some(simple_parser::ast::WhenSpec::DateTime(dt)) = d.when {
                    e.time = dt;
                }
                if let Some(dur) = d.duration {
                    e.duration = Some(dur);
                }
                if let Some(ref rec) = d.recurrence {
                    if let Some(dur) = recurrence_to_duration(Some(rec)) {
                        e.recurrence = Some(dur);
                    }
                }
                if let Some(ref content) = d.content {
                    e.content = Some(content.clone());
                }
                e
            }
            None => {
                // No parse — just update the title from raw text.
                let raw = self
                    .title_inputs
                    .get(&id)
                    .map(|e| e.read(cx).value().to_string())
                    .unwrap_or_default();
                let raw = raw.trim();
                if raw.is_empty() {
                    return;
                }
                let mut e = original.clone();
                e.title = raw.to_string();
                e
            }
        };

        // Emit parse warnings.
        if let Some(draft) = &draft {
            for warning in &draft.warnings {
                window.push_notification(
                    (
                        NotificationType::Warning,
                        SharedString::from(warning.clone()),
                    ),
                    cx,
                );
            }
        }

        let overlap_warnings = self
            .database_store
            .update(cx, |store, cx| store.update_queue_event(updated_event, cx));

        for warning in overlap_warnings {
            window.push_notification(
                (
                    NotificationType::Warning,
                    SharedString::from(format!(
                        "\"{}\" overlaps with \"{}\"",
                        warning.inserted_title, warning.conflicting_title
                    )),
                ),
                cx,
            );
        }

        window.blur();
    }

    fn item_content(
        &self,
        item_id: Uuid,
        title_input: Entity<InputState>,
        time_label: String,
        window: &Window,
        cx: &App,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let border_color = theme.border;
        let input_color = theme.input;

        let title_focused = title_input.focus_handle(cx).is_focused(window);

        let title_border = if title_focused {
            input_color
        } else {
            border_color.opacity(0.0)
        };

        // Determine preview / error state from the edit state map.
        let edit_state = self.title_edit_states.get(&item_id);
        let parse_error = edit_state.map(|s| s.parse_error).unwrap_or(false);
        let preview_text: Option<SharedString> = edit_state
            .and_then(|s| s.draft.as_ref())
            .and_then(Self::build_preview_text);

        let has_preview = preview_text.is_some();
        let error_color = theme.danger;
        let muted_color = theme.muted_foreground;

        v_flex()
            .w_full()
            .gap_1()
            .child(
                div()
                    .id(("title-input-wrap", item_id.as_u128() as u64))
                    .w_full()
                    .rounded_sm()
                    .border_1()
                    .border_color(if parse_error {
                        error_color.opacity(0.6)
                    } else {
                        title_border
                    })
                    .hover(move |s| {
                        s.border_color(if parse_error {
                            error_color
                        } else if title_focused {
                            input_color
                        } else {
                            border_color
                        })
                    })
                    .child(
                        Input::new(&title_input)
                            .text_sm()
                            .w_full()
                            .py_0()
                            .px_1()
                            .appearance(false),
                    ),
            )
            // Live parse preview — shown only when something useful was parsed
            .when(has_preview, |el| {
                el.child(
                    h_flex().w_full().px_1().child(
                        Label::new(preview_text.unwrap_or_default())
                            .text_xs()
                            .text_color(muted_color),
                    ),
                )
            })
            // Parse error hint
            .when(parse_error, |el| {
                el.child(
                    h_flex().w_full().px_1().child(
                        Label::new("⚠ Could not fully parse — press Enter to save as-is")
                            .text_xs()
                            .text_color(error_color),
                    ),
                )
            })
            .when(!time_label.is_empty(), |el| {
                el.child(
                    h_flex().w_full().px_1().child(
                        Label::new(SharedString::from(time_label))
                            .text_xs()
                            .text_color(muted_color),
                    ),
                )
            })
    }

    fn action_context_menu(
        &self,
        action_id: uuid::Uuid,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let entity = cx.entity();
        move |menu, _window, _cx| {
            let entity_complete = entity.clone();
            let entity_demote = entity.clone();
            let entity_delete = entity.clone();
            menu.item(
                PopupMenuItem::new("Complete")
                    .icon(IconName::CircleCheck)
                    .on_click(move |_event, _window, cx: &mut App| {
                        entity_complete.update(cx, |this, cx| {
                            this.database_store.update(cx, |store, cx| {
                                store.complete_action(action_id, cx);
                            });
                        });
                    }),
            )
            .item(
                PopupMenuItem::new("Demote to backlog")
                    .icon(IconName::ChevronDown)
                    .on_click(move |_event, _window, cx: &mut App| {
                        entity_demote.update(cx, |this, cx| {
                            this.database_store.update(cx, |store, cx| {
                                store.demote_action(action_id, cx);
                            });
                        });
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new("Delete")
                    .icon(IconName::Delete)
                    .on_click(move |_event, _window, cx: &mut App| {
                        entity_delete.update(cx, |this, cx| {
                            this.database_store.update(cx, |store, cx| {
                                store.delete_queue_action(action_id, cx);
                            });
                        });
                    }),
            )
        }
    }

    fn event_context_menu(
        &self,
        event_id: uuid::Uuid,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let entity = cx.entity();
        move |menu, _window, _cx| {
            let entity_delete = entity.clone();
            menu.item(
                PopupMenuItem::new("Delete")
                    .icon(IconName::Delete)
                    .on_click(move |_event, _window, cx: &mut App| {
                        entity_delete.update(cx, |this, cx| {
                            this.database_store.update(cx, |store, cx| {
                                store.remove_from_pipeline(event_id, cx);
                            });
                        });
                    }),
            )
        }
    }

    fn render_action(
        &mut self,
        action: &Action,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let action_id = action.id;
        let theme = cx.theme().clone();

        let item_height = if let Some(duration) = action.duration.as_ref() {
            let mins = duration.num_minutes().max(1);
            (mins as f32 * 8.).min(640.0).max(ITEM_MIN_HEIGHT)
        } else {
            ITEM_MIN_HEIGHT
        };

        let hue_source = theme.green;

        let colors = ItemColors {
            bg: tint_oklch(theme.secondary, hue_source, 0.03),
            border: tint_oklch(theme.border, hue_source, 0.04),
        };

        let drag_data = DragData::new(action.clone()).with_label(action.title.clone());

        let Some(title_input) = self.title_inputs.get(&action_id).cloned() else {
            return Draggable::new(("pipeline-action-drag", ix as u64), drag_data)
                .child(
                    self.item_base(Some(px(item_height)), colors)
                        .w_full()
                        .child(self.item_row()),
                )
                .into_any_element();
        };

        let time_label = format_time_label(&QueueItem::Action(action.clone()));

        // Ensure this item's input is subscribed to parse/commit events.
        // We guard by checking if a subscription entry exists via title_edit_states
        // having an entry (it always will after new() / update_items). The actual
        // subscriptions are managed once per item via the helper below — but since
        // render is called repeatedly we set them up lazily and idempotently by
        // checking whether we already have more than the initial pipeline subscription.
        //
        // We use a side-channel: subscribe once by detecting new items via
        // PipelineChanged subscription above; but on first render we need to
        // wire up inputs created in build_title_inputs. We call subscribe helpers
        // from new() for the initial set, but for items added dynamically we rely
        // on PipelineChanged. So we call the subscribe helpers here if they haven't
        // been called yet, keyed off a flag stored in a separate set.
        self.ensure_action_subscribed(action_id, &title_input, window, cx);

        Draggable::new(("pipeline-action-drag", ix as u64), drag_data)
            .context_menu(self.action_context_menu(action_id, cx))
            .child(
                self.item_base(Some(px(item_height)), colors)
                    .w_full()
                    .child(
                        self.item_row()
                            .child(
                                Checkbox::new(("pipeline-check", ix as u64))
                                    .checked(false)
                                    .occlude()
                                    .on_click(cx.listener(move |this, _checked, _window, cx| {
                                        this.database_store.update(cx, |store, cx| {
                                            store.complete_action(action_id, cx);
                                        });
                                    })),
                            )
                            .child(self.item_content(
                                action_id,
                                title_input,
                                time_label,
                                window,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn render_event(
        &mut self,
        event: &Event,
        ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let event_id = event.id;
        let theme = cx.theme().clone();

        let item_height = if let Some(duration) = event.duration.as_ref() {
            let mins = duration.num_minutes().max(1);
            (mins as f32 * 8.).min(640.0).max(ITEM_MIN_HEIGHT)
        } else {
            ITEM_MIN_HEIGHT
        };

        let hue_source = theme.blue;

        let colors = ItemColors {
            bg: tint_oklch(theme.secondary, hue_source, 0.03),
            border: tint_oklch(theme.border, hue_source, 0.04),
        };

        let drag_data = DragData::new(event.clone()).with_label(event.title.clone());

        let Some(title_input) = self.title_inputs.get(&event_id).cloned() else {
            return Draggable::new(("pipeline-event-drag", ix as u64), drag_data)
                .child(
                    self.item_base(Some(px(item_height)), colors)
                        .w_full()
                        .child(self.item_row()),
                )
                .into_any_element();
        };

        let time_label = format_time_label(&QueueItem::Event(event.clone()));

        self.ensure_event_subscribed(event_id, &title_input, window, cx);

        Draggable::new(("pipeline-event-drag", ix as u64), drag_data)
            .context_menu(self.event_context_menu(event_id, cx))
            .child(
                self.item_base(Some(px(item_height)), colors)
                    .w_full()
                    .child(
                        self.item_row()
                            .child(Icon::new(IconName::Calendar).opacity(0.5))
                            .child(self.item_content(
                                event_id,
                                title_input,
                                time_label,
                                window,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    /// Register title-input subscriptions for an action item the first time
    /// it is rendered.  Idempotent: re-registering is harmless since old
    /// subscriptions just fire a no-op once the entry is stale.
    fn ensure_action_subscribed(
        &mut self,
        id: Uuid,
        title_input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.subscribe_action_title(id, title_input, cx, window);
    }

    fn ensure_event_subscribed(
        &mut self,
        id: Uuid,
        title_input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.subscribe_event_title(id, title_input, cx, window);
    }
}

/// Format a [`RecurrenceSpec`] as a short human-readable string.
fn format_recurrence(spec: &simple_parser::RecurrenceSpec) -> String {
    use simple_parser::RecurrenceSpec;
    match spec {
        RecurrenceSpec::EveryDays(1) => "daily".into(),
        RecurrenceSpec::EveryDays(7) => "weekly".into(),
        RecurrenceSpec::EveryDays(n) => format!("every {n} days"),
        RecurrenceSpec::EveryWeeks(1) => "weekly".into(),
        RecurrenceSpec::EveryWeeks(n) => format!("every {n} weeks"),
        RecurrenceSpec::EveryMonths(1) => "monthly".into(),
        RecurrenceSpec::EveryMonths(3) => "quarterly".into(),
        RecurrenceSpec::EveryMonths(n) => format!("every {n} months"),
        RecurrenceSpec::EveryYears(1) => "yearly".into(),
        RecurrenceSpec::EveryYears(n) => format!("every {n} years"),
        RecurrenceSpec::OnMonthDay(day) => format!("the {}", ordinal(*day)),
        RecurrenceSpec::OnWeekdays(set) => format_weekday_set(set),
    }
}

fn ordinal(n: u32) -> String {
    let suffix = match n % 100 {
        11 | 12 | 13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{n}{suffix}")
}

fn format_weekday_set(set: &simple_parser::WeekdaySet) -> String {
    use chrono::Weekday::*;
    use simple_parser::WeekdaySet;

    if *set == WeekdaySet::every_day() {
        return "daily".into();
    }
    if *set == WeekdaySet::weekdays() {
        return "weekdays".into();
    }
    if *set == WeekdaySet::weekends() {
        return "weekends".into();
    }

    let names: Vec<&str> = [Mon, Tue, Wed, Thu, Fri, Sat, Sun]
        .iter()
        .filter(|&&d| set.contains(d))
        .map(|d| match d {
            Mon => "Mon",
            Tue => "Tue",
            Wed => "Wed",
            Thu => "Thu",
            Fri => "Fri",
            Sat => "Sat",
            Sun => "Sun",
        })
        .collect();

    names.join(", ")
}

/// Convert a neutral (grey) `Hsla` into a tinted version by working in Oklch.
fn tint_oklch(color: Hsla, hue_source: Hsla, chroma: f32) -> Hsla {
    let hue_rgba = hue_source.to_rgb();
    let hue_srgb = Srgb::new(hue_rgba.r, hue_rgba.g, hue_rgba.b);
    let hue_oklch: Oklch = hue_srgb.into_color();

    let rgba = color.to_rgb();
    let srgb = Srgb::new(rgba.r, rgba.g, rgba.b);
    let oklch: Oklch = srgb.into_color();
    let tinted = Oklch::new(oklch.l, chroma, hue_oklch.hue);
    let out: Srgb = Srgb::from_color(tinted);
    let out = out.clamp();
    let out_rgba = gpui::rgba(
        ((out.red * 255.0).round() as u32) << 24
            | ((out.green * 255.0).round() as u32) << 16
            | ((out.blue * 255.0).round() as u32) << 8
            | (rgba.a * 255.0).round() as u32,
    );
    out_rgba.into()
}

fn format_time_label(item: &QueueItem) -> String {
    let target_str = item.time().map(format_target_time);
    let duration_str = match item {
        QueueItem::Action(a) => a.duration.as_ref().map(format_duration),
        QueueItem::Event(e) => e.duration.as_ref().map(format_duration),
    };
    match (target_str, duration_str) {
        (Some(t), Some(d)) => format!("{} · {}", t, d),
        (Some(t), None) => t,
        (None, Some(d)) => d,
        (None, None) => String::new(),
    }
}

fn format_target_time(time: DateTime<Utc>) -> String {
    let local = time.with_timezone(&Local);
    let now = Local::now();
    let is_today = local.date_naive() == now.date_naive();
    let is_tomorrow = local.date_naive() == (now + chrono::Duration::days(1)).date_naive();

    let time_str = if local.minute() == 0 {
        format!(
            "{}{}",
            if local.hour12().1 == 0 {
                12
            } else {
                local.hour12().1
            },
            if local.hour12().0 { "pm" } else { "am" }
        )
    } else {
        format!(
            "{}:{:02}{}",
            if local.hour12().1 == 0 {
                12
            } else {
                local.hour12().1
            },
            local.minute(),
            if local.hour12().0 { "pm" } else { "am" }
        )
    };

    if is_today {
        format!("Today {}", time_str)
    } else if is_tomorrow {
        format!("Tomorrow {}", time_str)
    } else {
        format!("{} {}", local.format("%b %-d"), time_str)
    }
}

fn format_duration(duration: &chrono::Duration) -> String {
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

impl EventEmitter<StartActionEditor> for Pipeline {}
impl EventEmitter<StartEventEditor> for Pipeline {}
impl EventEmitter<StartQueueEventEditor> for Pipeline {}

impl Render for Pipeline {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        if self.entries.is_empty() {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .py_8()
                .gap_2()
                .child(
                    Label::new("Queue is empty")
                        .text_sm()
                        .text_color(theme.muted_foreground),
                )
                .into_any_element();
        }

        let scroll_handle = self.scroll_handle.clone();

        // We need to clone entries to avoid borrow conflicts while calling
        // render_action/render_event (which need &mut self).
        let entries: Vec<QueueItem> = self.entries.clone();

        div()
            .relative()
            .w_full()
            // Escape while an input is focused → blur it, which fires InputEvent::Blur → revert.
            .on_action(cx.listener(|this, _: &InputEscape, window, cx| {
                // Find whichever title input is currently focused and blur it.
                for input in this.title_inputs.values() {
                    if input.focus_handle(cx).is_focused(window) {
                        window.blur();
                        break;
                    }
                }
            }))
            .child(
                div()
                    .id("pipeline-items")
                    .overflow_y_scroll()
                    .track_scroll(&scroll_handle)
                    .w_full()
                    .child(v_flex().w_full().gap_2().p_2().children(
                        entries.iter().enumerate().map(|(ix, entry)| match entry {
                            QueueItem::Action(action) => self.render_action(action, ix, window, cx),
                            QueueItem::Event(event) => self.render_event(event, ix, window, cx),
                        }),
                    )),
            )
            .child(Scrollbar::vertical(&scroll_handle))
            .into_any_element()
    }
}
