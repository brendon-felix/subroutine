use anyhow::Result;
use gpui::{
    App, AppContext, Application, Bounds, Focusable, KeyBinding, WindowBounds, WindowOptions,
    actions, px, size,
};

use crate::stores::{TaskStore, UiStateStore};
use crate::views::RootView;

actions!(app, [ToggleCommandPalette, ToggleFocusedView, Quit]);

pub struct Subroutine;

impl Subroutine {
    pub fn run() -> Result<()> {
        Application::new().run(move |cx: &mut App| {
            cx.activate(true);

            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-p", ToggleCommandPalette, None),
            ]);
            let task_store = cx.new(move |cx| TaskStore::new(cx));
            let ui_store = cx.new(|_cx| UiStateStore::new());

            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
            let window_options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                show: true,
                ..Default::default()
            };

            let window = cx
                .open_window(window_options, move |_window, cx| {
                    cx.new(move |cx| RootView::new(task_store.clone(), ui_store.clone(), cx))
                })
                .unwrap();

            window
                .update(cx, |root_view, window, cx| {
                    window.focus(&root_view.focus_handle(cx));
                })
                .unwrap();

            cx.on_action(move |_action: &Quit, cx: &mut App| {
                cx.quit();
            });
        });
        Ok(())
    }
}
