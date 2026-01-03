use std::path::PathBuf;

use anyhow::Result;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, Menu, MenuItem, SharedString, WindowBounds,
    WindowOptions, actions, px, size,
};
use gpui_component::{Root, Theme, ThemeRegistry};
use gpui_component_assets::Assets;

use crate::components;
use crate::stores::TaskStore;
use crate::views::RootView;

actions!(app, [Quit]);

pub fn run() -> Result<()> {
    // let json = std::fs::read_to_string(STATE_FILE).unwrap_or_default();
    // let state = serde_json::from_str::<State>(&json).unwrap_or_default();

    Application::new().with_assets(Assets).run(init);
    Ok(())
}

pub fn init(cx: &mut App) {
    cx.activate(true);

    gpui_component::init(cx);
    components::init(cx);

    let theme_name = SharedString::from("Molokai Dark");
    // Load and watch themes from ./themes directory
    if let Err(e) = ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, move |cx| {
        if let Some(theme) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme);
        }
    }) {
        eprintln!("Failed to load themes from ./themes: {}", e);
    }

    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        // KeyBinding::new("cmd-p", ToggleCommandPalette, None),
    ]);
    let task_store = cx.new(move |cx| TaskStore::new(cx));
    // let ui_store = cx.new(|_cx| UiStateStore::new());

    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        focus: true,
        show: true,
        ..Default::default()
    };

    cx.set_menus(app_menus());

    let _window = cx
        .open_window(window_options, move |window, cx| {
            let app_view =
                // cx.new(|cx| RootView::new(task_store.clone(), ui_store.clone(), window, cx));
                cx.new(|cx| RootView::new(task_store.clone(), window, cx));
            cx.new(|cx| Root::new(app_view, window, cx))
        })
        .unwrap();

    cx.on_action(move |_action: &Quit, cx: &mut App| {
        cx.quit();
    });
}

fn app_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Subroutine".into(),
            items: vec![
                // MenuItem::action("About", About),
                // MenuItem::Separator,
                // MenuItem::action("Open...", Open),
                // MenuItem::Separator,
                MenuItem::Submenu(Menu {
                    name: "Appearance".into(),
                    items: vec![
                        // MenuItem::action("Light", SwitchThemeMode(ThemeMode::Light)),
                        // MenuItem::action("Dark", SwitchThemeMode(ThemeMode::Dark)),
                    ],
                }),
                // theme_menu(cx),
                // language_menu(cx),
                MenuItem::Separator,
                MenuItem::action("Quit", Quit),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", gpui_component::input::Undo),
                MenuItem::action("Redo", gpui_component::input::Redo),
                MenuItem::separator(),
                MenuItem::action("Cut", gpui_component::input::Cut),
                MenuItem::action("Copy", gpui_component::input::Copy),
                MenuItem::action("Paste", gpui_component::input::Paste),
                MenuItem::separator(),
                MenuItem::action("Delete", gpui_component::input::Delete),
                MenuItem::action(
                    "Delete Previous Word",
                    gpui_component::input::DeleteToPreviousWordStart,
                ),
                MenuItem::action(
                    "Delete Next Word",
                    gpui_component::input::DeleteToNextWordEnd,
                ),
                MenuItem::separator(),
                MenuItem::action("Find", gpui_component::input::Search),
                MenuItem::separator(),
                MenuItem::action("Select All", gpui_component::input::SelectAll),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                // MenuItem::action("Close Window", CloseWindow),
                // MenuItem::separator(),
                // MenuItem::action("Toggle Search", ToggleSearch),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                // MenuItem::action("Close Window", CloseWindow),
                // MenuItem::separator(),
                // MenuItem::action("Toggle Search", ToggleSearch),
            ],
        },
        Menu {
            name: "Help".into(),
            items: vec![],
        },
    ]
}
