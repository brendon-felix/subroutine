use gpui::prelude::ParentElement;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, Render, Styled, Subscription, Window, actions, div, px, rgb,
};

use crate::stores::task_store::ApiError;
use crate::stores::{TaskStore, UiStateStore};
use crate::views::{SidebarView, TaskListView};

actions!(root_view, [CloseModal, Escape]);

pub struct RootView {
    // task_store: Entity<TaskStore>,
    // ui_store: Entity<UiStateStore>,
    sidebar: Entity<SidebarView>,
    task_list: Entity<TaskListView>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl RootView {
    pub fn new(
        task_store: Entity<TaskStore>,
        ui_store: Entity<UiStateStore>,
        cx: &mut Context<Self>,
    ) -> Self {
        let sidebar = cx.new(|cx| SidebarView::new(ui_store.clone(), cx));
        let task_list = cx.new(|cx| TaskListView::new(task_store.clone(), ui_store.clone(), cx));

        let focus_handle = cx.focus_handle();
        let mut subscriptions = Vec::new();

        // // Subscribe to task store events
        // subscriptions.push(cx.subscribe(
        //     &task_store,
        //     |_this, _task_store, _event: &TasksUpdated, cx| {
        //         cx.notify();
        //     },
        // ));

        subscriptions.push(cx.subscribe(
            &task_store,
            |_this, _task_store, event: &ApiError, cx| {
                eprintln!("API Error: {}", event.message);
                cx.notify();
            },
        ));

        // // Subscribe to UI state events
        // subscriptions.push(cx.subscribe(
        //     &ui_store,
        //     |this, _ui_store, _event: &CommandPaletteToggled, cx| {
        //         this.handle_command_palette_toggle(cx);
        //     },
        // ));

        Self {
            // task_store,
            // ui_store,
            sidebar,
            task_list,
            focus_handle,
            _subscriptions: subscriptions,
        }
    }
}

impl Focusable for RootView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<()> for RootView {}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x191919))
            .text_color(rgb(0xC8C8C8))
            .flex()
            .relative()
            .track_focus(&self.focus_handle)
            // .on_action(cx.listener(|this, _: &CloseModal, _window, cx| {
            //     this.ui_store.update(cx, |ui_state, cx| {
            //         ui_state.close_all_modals();
            //         cx.notify();
            //     });
            // }))
            .child(
                // Sidebar
                div()
                    .id("sidebar-container")
                    .w(px(280.0))
                    .h_full()
                    .bg(rgb(0x191919))
                    .border_r_1()
                    .border_color(rgb(0x303030))
                    .child(self.sidebar.clone()),
            )
            .child(
                // Main content
                div().flex_1().h_full().flex().flex_col().child(
                    // Task list
                    div()
                        .id("task-list-container")
                        .flex_1()
                        .bg(rgb(0x191919))
                        .overflow_y_hidden()
                        .child(self.task_list.clone()),
                ),
            )

        // if let Some(command_palette) = &self.command_palette {
        //     result = result.child(
        //         div()
        //             .id("command-palette-overlay")
        //             .absolute()
        //             .top_0()
        //             .left_0()
        //             .size_full()
        //             .bg(rgba(0x00000080))
        //             .flex()
        //             .items_center()
        //             .justify_center()
        //             .child(command_palette.clone()),
        //     );
        // }

        // result
    }
}
