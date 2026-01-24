use anyhow::Result;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions,
    WindowBounds, WindowOptions, actions, point, px, size,
};
use gpui_component::{ActiveTheme, Root, Theme, ThemeMode, ThemeRegistry, input};

use crate::assets::Assets;
use crate::stores::TaskStore;
use crate::themes::{SwitchTheme, SwitchThemeMode};
use crate::views::RootView;
use crate::{components, themes};

actions!(
    app,
    [
        About,
        Open,
        Quit,
        ToggleSearch,
        // TestAction,
        // Tab,
        // TabPrev,
        // ShowPanelInfo,
        // ToggleListActiveHighlight
    ]
);

pub trait ResultExt<T> {
    fn log_err(self);
}

impl<T> ResultExt<T> for Result<T> {
    fn log_err(self) {
        if let Err(err) = self {
            eprintln!("Error: {}", err);
        }
    }
}

pub fn run() -> Result<()> {
    // let json = std::fs::read_to_string(STATE_FILE).unwrap_or_default();
    // let state = serde_json::from_str::<State>(&json).unwrap_or_default();

    Application::new().with_assets(Assets).run(init);
    Ok(())
}

pub fn init(cx: &mut App) {
    cx.activate(true);

    gpui_component::init(cx);
    themes::init(cx);
    components::init(cx);

    // // let theme_name = SharedString::from("Gruvbox Dark");
    // let theme_name = SharedString::from("Ayu Dark");
    // // let theme_name = SharedString::from("Solarized Dark");
    // // let theme_name = SharedString::from("Solarized Light");
    // // let theme_name = SharedString::from("Molokai Dark");
    // // Load and watch themes from ./themes directory
    // if let Err(e) = ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, move |cx| {
    //     if let Some(theme) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
    //         Theme::global_mut(cx).apply_config(&theme);
    //     }
    // }) {
    //     eprintln!("Failed to load themes from ./themes: {}", e);
    // }

    // let theme_name = "Ayu Dark";
    // let theme_name = "Gruvbox Dark";
    // let theme_name = "Molokai Light";
    // let theme_name = "Solarized Light";

    // if let Err(e) = ThemeRegistry::watch_dir(PathBuf::from("./assets/themes"), cx, move |cx| {
    //     if let Some(theme) = ThemeRegistry::global(cx).themes().get(theme_name).cloned() {
    //         // Theme::global_mut(cx).apply_config(&theme);
    //         Theme::global_mut(cx).apply_config(&theme);
    //     }
    // }) {
    //     eprintln!("Failed to load themes from ./themes: {}", e);
    // }

    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        // KeyBinding::new("cmd-p", ToggleCommandPalette, None),
    ]);
    let task_store = cx.new(move |cx| TaskStore::new(cx));
    // let ui_store = cx.new(|_cx| UiStateStore::new());

    let mut titlebar_options = TitlebarOptions::default();
    titlebar_options.appears_transparent = true;
    titlebar_options.traffic_light_position = Some(point(px(16.), px(16.)));

    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(titlebar_options),
        // titlebar: None,
        focus: true,
        show: true,
        window_min_size: Some(size(px(600.0), px(280.0))),
        ..Default::default()
    };

    update_app_menu(cx);

    cx.observe_global::<Theme>({
        move |cx| {
            update_app_menu(cx);
        }
    })
    .detach();

    let _window = cx
        .open_window(window_options, move |window, cx| {
            let app_view =
                // cx.new(|cx| RootView::new(task_store.clone(), ui_store.clone(), window, cx));
                cx.new(|cx| RootView::new(task_store.clone(), window, cx));
            let root = cx.new(|cx| Root::new(app_view, window, cx));
            root
        })
        .unwrap();

    cx.on_action(move |_action: &Quit, cx: &mut App| {
        cx.quit();
    });
}

fn theme_menu(cx: &App) -> MenuItem {
    let mut themes = ThemeRegistry::global(cx)
        .themes()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    let current_name = cx.theme().theme_name();
    MenuItem::Submenu(Menu {
        name: "Theme".into(),
        items: themes
            .iter()
            .map(|theme| {
                let checked = current_name == &theme.name;
                MenuItem::action(theme.name.clone(), SwitchTheme(theme.name.clone()))
                    .checked(checked)
            })
            .collect(),
    })
}

// fn language_menu(_: &App) -> MenuItem {
//     let locale = rust_i18n::locale().to_string();
//     MenuItem::Submenu(Menu {
//         name: "Language".into(),
//         items: vec![
//             MenuItem::action("English", SelectLocale("en".into())).checked(locale == "en"),
//             MenuItem::action("简体中文", SelectLocale("zh-CN".into())).checked(locale == "zh-CN"),
//         ],
//     })
// }

fn update_app_menu(cx: &App) {
    let mode = cx.theme().mode;
    cx.set_menus(vec![
        Menu {
            name: "Subroutine".into(),
            items: vec![
                MenuItem::action("About", About),
                MenuItem::Separator,
                MenuItem::action("Open...", Open),
                MenuItem::Separator,
                MenuItem::Submenu(Menu {
                    name: "Appearance".into(),
                    items: vec![
                        MenuItem::action("Light", SwitchThemeMode(ThemeMode::Light))
                            .checked(!mode.is_dark()),
                        MenuItem::action("Dark", SwitchThemeMode(ThemeMode::Dark))
                            .checked(mode.is_dark()),
                    ],
                }),
                theme_menu(cx),
                // language_menu(cx),
                MenuItem::Separator,
                MenuItem::action("Quit", Quit),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", input::Undo),
                MenuItem::action("Redo", input::Redo),
                MenuItem::separator(),
                MenuItem::action("Cut", input::Cut),
                MenuItem::action("Copy", input::Copy),
                MenuItem::action("Paste", input::Paste),
                MenuItem::separator(),
                MenuItem::action("Delete", input::Delete),
                MenuItem::action("Delete Previous Word", input::DeleteToPreviousWordStart),
                MenuItem::action("Delete Next Word", input::DeleteToNextWordEnd),
                MenuItem::separator(),
                MenuItem::action("Find", input::Search),
                MenuItem::separator(),
                MenuItem::action("Select All", input::SelectAll),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![MenuItem::action("Toggle Search", ToggleSearch)],
        },
        Menu {
            name: "Help".into(),
            items: vec![MenuItem::action("Open Website", Open)],
        },
    ]);
    // vec![
    //     Menu {
    //         name: "Subroutine".into(),
    //         items: vec![
    //             // MenuItem::action("About", About),
    //             // MenuItem::Separator,
    //             // MenuItem::action("Open...", Open),
    //             // MenuItem::Separator,
    //             MenuItem::Submenu(Menu {
    //                 name: "Appearance".into(),
    //                 items: vec![
    //                     // MenuItem::action("Light", SwitchThemeMode(ThemeMode::Light)),
    //                     // MenuItem::action("Dark", SwitchThemeMode(ThemeMode::Dark)),
    //                 ],
    //             }),
    //             // theme_menu(cx),
    //             // language_menu(cx),
    //             MenuItem::Separator,
    //             MenuItem::action("Quit", Quit),
    //         ],
    //     },
    //     Menu {
    //         name: "Edit".into(),
    //         items: vec![
    //             MenuItem::action("Undo", gpui_component::input::Undo),
    //             MenuItem::action("Redo", gpui_component::input::Redo),
    //             MenuItem::separator(),
    //             MenuItem::action("Cut", gpui_component::input::Cut),
    //             MenuItem::action("Copy", gpui_component::input::Copy),
    //             MenuItem::action("Paste", gpui_component::input::Paste),
    //             MenuItem::separator(),
    //             MenuItem::action("Delete", gpui_component::input::Delete),
    //             MenuItem::action(
    //                 "Delete Previous Word",
    //                 gpui_component::input::DeleteToPreviousWordStart,
    //             ),
    //             MenuItem::action(
    //                 "Delete Next Word",
    //                 gpui_component::input::DeleteToNextWordEnd,
    //             ),
    //             MenuItem::separator(),
    //             MenuItem::action("Find", gpui_component::input::Search),
    //             MenuItem::separator(),
    //             MenuItem::action("Select All", gpui_component::input::SelectAll),
    //         ],
    //     },
    //     Menu {
    //         name: "View".into(),
    //         items: vec![
    //             // MenuItem::action("Close Window", CloseWindow),
    //             // MenuItem::separator(),
    //             // MenuItem::action("Toggle Search", ToggleSearch),
    //         ],
    //     },
    //     Menu {
    //         name: "Window".into(),
    //         items: vec![
    //             // MenuItem::action("Close Window", CloseWindow),
    //             // MenuItem::separator(),
    //             // MenuItem::action("Toggle Search", ToggleSearch),
    //         ],
    //     },
    //     Menu {
    //         name: "Help".into(),
    //         items: vec![],
    //     },
    // ]
}
