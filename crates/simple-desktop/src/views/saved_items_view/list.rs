use chrono::Duration as ChronoDuration;
use gpui::{
    App, Context, Hsla, IntoElement, ParentElement, Pixels, SharedString, Styled, Task, Window,
    prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Colorize, IndexPath, h_flex,
    label::Label,
    list::{ListDelegate, ListItem, ListState},
    menu::{ContextMenuExt, PopupMenu, PopupMenuItem},
};
use simple_core::{Action, AnyItem, Event, Routine};
use uuid::Uuid;

use crate::{
    AppIcon,
    components::{DragData, Draggable},
    stores::AppDatabaseStore,
    utils::ButtonColors,
};

const ITEM_HEIGHT: Pixels = px(12. * 4.);

fn duration_str(duration: ChronoDuration) -> SharedString {
    let mut s = String::with_capacity(12);
    if duration.is_zero() {
        return SharedString::from("0s");
    }
    let n_hours = duration.num_hours();
    if n_hours > 0 {
        use std::fmt::Write;
        let _ = write!(s, "{}h ", n_hours);
        let n_minutes = duration.num_minutes() % 60;
        if n_minutes > 0 {
            let _ = write!(s, "{}m ", n_minutes);
            let n_seconds = duration.num_seconds() % 60;
            if n_seconds > 0 {
                let _ = write!(s, "{}s", n_seconds);
            }
        }
    }
    SharedString::from(s)
}

fn render_item_preview(
    colors: ButtonColors,
    title: SharedString,
    text_color: Hsla,
) -> impl IntoElement {
    h_flex()
        .size_full()
        .px_2()
        .gap_2()
        .py_0p5()
        .rounded_lg()
        .shadow_md()
        .border_1()
        .bg(colors.bg)
        .when_some(colors.border, |this, c| this.border_color(c))
        .child(Label::new(title).text_sm().text_color(text_color))
}

fn action_context_menu(
    action_id: Uuid,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _window, _cx| {
        menu.item(
            PopupMenuItem::new("Delete saved action")
                .icon(AppIcon::Trash)
                .on_click(move |_event, _window, cx: &mut App| {
                    let db_store = AppDatabaseStore::global(cx);
                    db_store.update(cx, |store, cx| {
                        store.delete_action(action_id, cx);
                    });
                }),
        )
    }
}

fn event_context_menu(
    event_id: Uuid,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _window, _cx| {
        menu.item(
            PopupMenuItem::new("Reschedule saved event")
                .icon(AppIcon::CalendarClock)
                .on_click(|_, _, _cx| {}),
        )
        .separator()
        .item(
            PopupMenuItem::new("Delete saved event")
                .icon(AppIcon::Trash)
                .on_click(move |_event, _window, cx| {
                    let db_store = AppDatabaseStore::global(cx);
                    db_store.update(cx, |store, cx| {
                        store.delete_event(event_id, cx);
                    });
                }),
        )
    }
}

fn routine_context_menu(
    routine_id: Uuid,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
    move |menu, _window, _cx| {
        menu.item(
            PopupMenuItem::new("Reschedule saved routine")
                .icon(AppIcon::CalendarClock)
                .on_click(|_, _, _cx| {}),
        )
        .separator()
        .item(
            PopupMenuItem::new("Delete saved routine")
                .icon(AppIcon::Trash)
                .on_click(move |_event, _window, cx| {
                    let db_store = AppDatabaseStore::global(cx);
                    db_store.update(cx, |store, cx| {
                        store.delete_routine(routine_id, cx);
                    });
                }),
        )
    }
}

pub struct SavedItemsList {
    pub selected: Option<IndexPath>,
    pub right_clicked: Option<IndexPath>,
    pub saved_actions: Vec<Action>,
    pub saved_events: Vec<Event>,
    pub saved_routines: Vec<Routine>,
    pub filtered_ids: Vec<Uuid>,
    pub loading: bool,
}

impl SavedItemsList {
    pub fn new() -> Self {
        Self {
            selected: None,
            right_clicked: None,
            saved_actions: vec![],
            saved_events: vec![],
            saved_routines: vec![],
            filtered_ids: vec![],
            loading: true,
        }
    }

    fn get_action(&self, ix: usize) -> Option<&Action> {
        self.saved_actions
            .iter()
            .filter(|action| self.filtered_ids.contains(&action.id))
            .nth(ix)
    }

    fn get_event(&self, ix: usize) -> Option<&Event> {
        self.saved_events
            .iter()
            .filter(|event| self.filtered_ids.contains(&event.id))
            .nth(ix)
    }

    fn get_routine(&self, ix: usize) -> Option<&Routine> {
        self.saved_routines
            .iter()
            .filter(|routine| self.filtered_ids.contains(&routine.id))
            .nth(ix)
    }

    fn _render_section_header(&self, section: usize, cx: &App) -> Option<impl IntoElement> {
        match section {
            0 => Some(Label::new("Saved actions")),
            1 => Some(Label::new("Saved events")),
            2 => Some(Label::new("Saved routines")),
            _ => None,
        }
        .map(|label| {
            h_flex()
                .bg(cx.theme().secondary)
                .w_full()
                .h_8()
                .px_2()
                .justify_between()
                .child(label.text_xs().text_color(cx.theme().muted_foreground))
            // .child(
            //     Button::new(("saved-section", section as u32))
            //         .ghost()
            //         .small()
            //         .map(|b| match section {
            //             0 => b.when_else(
            //                 self.actions_hidden,
            //                 |b| b.icon(IconName::ChevronUp),
            //                 |b| b.icon(IconName::ChevronDown),
            //             ),
            //             1 => b.when_else(
            //                 self.events_hidden,
            //                 |b| b.icon(IconName::ChevronUp),
            //                 |b| b.icon(IconName::ChevronDown),
            //             ),
            //             2 => b.when_else(
            //                 self.routines_hidden,
            //                 |b| b.icon(IconName::ChevronUp),
            //                 |b| b.icon(IconName::ChevronDown),
            //             ),
            //             _ => b,
            //         }),
            // )
        })
    }

    fn render_action(&self, ix: usize, cx: &App) -> Option<ListItem> {
        let action = self.get_action(ix)?;
        let element_id = ("saved-action", action.id.as_u64_pair().1);
        let preview_colors = ButtonColors::outline(cx.theme().button_primary, cx);
        let text_color = cx.theme().muted_foreground;
        let title = action.title.clone();
        let duration_str = action.duration.map(|d| duration_str(d));
        let preview_title = SharedString::new(title.clone());

        let drag_data = DragData::new(AnyItem::Action(action.clone()))
            .with_preview(move || {
                render_item_preview(preview_colors, preview_title.clone(), text_color)
                    .into_any_element()
            })
            .with_preview_size(gpui::size(px(64. * 4.), ITEM_HEIGHT));
        Some(
            ListItem::new(element_id.clone())
                .rounded_lg()
                .h_12()
                .mx_2()
                .my_1()
                .child(
                    Draggable::new(element_id.clone(), drag_data)
                        .size_full()
                        .p_2()
                        .context_menu(action_context_menu(action.id))
                        .child(
                            h_flex()
                                .justify_between()
                                .child(Label::new(title))
                                .when_some(duration_str, |this, duration| {
                                    this.child(
                                        Label::new(duration)
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                }),
                        ),
                ),
        )
    }

    fn render_event(&self, ix: usize, cx: &App) -> Option<ListItem> {
        let event = self.get_event(ix)?;
        let element_id = ("saved-event", event.id.as_u64_pair().1);
        let preview_colors = ButtonColors::outline(
            cx.theme()
                .button_primary
                .mix_oklab(cx.theme().foreground, 0.5),
            cx,
        );
        let text_color = cx.theme().muted_foreground;
        let preview_title = SharedString::new(event.title.clone());
        let drag_data = DragData::new(AnyItem::Event(event.clone()))
            .with_preview(move || {
                render_item_preview(preview_colors, preview_title.clone(), text_color)
                    .into_any_element()
            })
            .with_preview_size(gpui::size(px(64. * 4.), ITEM_HEIGHT));
        Some(
            ListItem::new(element_id.clone())
                .rounded_lg()
                .h_12()
                .mx_2()
                .my_1()
                .child(
                    Draggable::new(element_id.clone(), drag_data)
                        .size_full()
                        .p_2()
                        .context_menu(event_context_menu(event.id))
                        .child(Label::new(event.title.clone())),
                ),
        )
    }

    fn render_routine(&self, ix: usize, cx: &App) -> Option<ListItem> {
        let routine = self.get_routine(ix)?;
        let element_id = ("saved-routine", routine.id.as_u64_pair().1);
        let preview_colors = ButtonColors::outline(cx.theme().foreground, cx);
        let text_color = cx.theme().muted_foreground;
        let preview_title = SharedString::new(routine.title.clone());
        let drag_data = DragData::new(AnyItem::Routine(routine.clone()))
            .with_preview(move || {
                render_item_preview(preview_colors, preview_title.clone(), text_color)
                    .into_any_element()
            })
            .with_preview_size(gpui::size(px(64. * 4.), ITEM_HEIGHT));
        Some(
            ListItem::new(element_id.clone())
                .rounded_lg()
                .h_12()
                .mx_2()
                .my_1()
                .child(
                    Draggable::new(element_id.clone(), drag_data)
                        .size_full()
                        .p_2()
                        .context_menu(routine_context_menu(routine.id))
                        .child(Label::new(routine.title.clone())),
                ),
        )
    }
}

impl ListDelegate for SavedItemsList {
    type Item = ListItem;

    // fn cancel(&mut self, window: &mut Window, cx: &mut Context<gpui_component::list::ListState<Self>>) {

    // }

    // fn confirm(&mut self, secondary: bool, window: &mut Window, cx: &mut Context<gpui_component::list::ListState<Self>>) {

    // }

    // fn has_more(&self, cx: &gpui::App) -> bool {

    // }

    fn items_count(&self, section: usize, _cx: &App) -> usize {
        match section {
            0 => self.saved_actions.len(),
            1 => self.saved_events.len(),
            2 => self.saved_routines.len(),
            _ => 0,
        }
    }

    // fn load_more(&mut self, window: &mut Window, cx: &mut Context<gpui_component::list::ListState<Self>>) {

    // }

    // fn load_more_threshold(&self) -> usize {

    // }

    fn loading(&self, _cx: &App) -> bool {
        self.loading
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        let action_ids = self
            .saved_actions
            .iter()
            .filter(|action| action.title.to_lowercase().contains(&query.to_lowercase()))
            .map(|a| a.id);
        let event_ids = self
            .saved_events
            .iter()
            .filter(|event| event.title.to_lowercase().contains(&query.to_lowercase()))
            .map(|e| e.id);
        let routine_ids = self
            .saved_routines
            .iter()
            .filter(|routine| routine.title.to_lowercase().contains(&query.to_lowercase()))
            .map(|r| r.id);
        self.filtered_ids = action_ids.chain(event_ids).chain(routine_ids).collect();
        Task::ready(())
    }

    // fn render_empty(
    //     &mut self,
    //     window: &mut Window,
    //     cx: &mut Context<gpui_component::list::ListState<Self>>,
    // ) -> impl IntoElement
    // {

    // }

    // fn render_initial(
    //     &mut self,
    //     window: &mut Window,
    //     cx: &mut Context<gpui_component::list::ListState<Self>>,
    // ) -> Option<gpui::AnyElement>
    // {

    // }

    fn render_item(
        &mut self,
        ix: gpui_component::IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        match ix.section {
            0 => self.render_action(ix.row, cx),
            1 => self.render_event(ix.row, cx),
            2 => self.render_routine(ix.row, cx),
            _ => None,
        }
    }

    // fn render_loading(
    //     &mut self,
    //     window: &mut Window,
    //     cx: &mut Context<gpui_component::list::ListState<Self>>,
    // ) -> impl IntoElement
    // {

    // }

    // fn render_section_footer(
    //     &mut self,
    //     section: usize,
    //     window: &mut Window,
    //     cx: &mut Context<gpui_component::list::ListState<Self>>,
    // ) -> Option<impl IntoElement>
    // {

    // }

    fn render_section_header(
        &mut self,
        section: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        self._render_section_header(section, cx)
    }

    fn sections_count(&self, _cx: &App) -> usize {
        3 // actions, events, routines
    }

    fn set_right_clicked_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.right_clicked = ix;
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
    }
}
