use anyhow::Result;
use gpui::{
    App, AppContext, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, actions, point, px, size,
};
use gpui_component::{ActiveTheme, Root, Theme, ThemeMode, ThemeRegistry, TitleBar, input};

use crate::{
    assets::AppAssets,
    components,
    stores::AppDatabaseStore,
    themes::{self, SwitchTheme, SwitchThemeMode},
    views::RootView,
};

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

// pub trait ResultExt<T> {
//     fn log_err(self);
// }

// impl<T> ResultExt<T> for Result<T> {
//     fn log_err(self) {
//         if let Err(err) = self {
//             eprintln!("Error: {}", err);
//         }
//     }
// }

pub fn run() -> Result<()> {
    // let json = std::fs::read_to_string(STATE_FILE).unwrap_or_default();
    // let state = serde_json::from_str::<State>(&json).unwrap_or_default();

    gpui_platform::application()
        .with_assets(AppAssets)
        .run(init);
    Ok(())
}

// fn load_embedded_fonts(cx: &App) -> anyhow::Result<()> {
//     let font_paths = cx.asset_source().list("fonts")?;
//     let mut embedded_fonts = Vec::new();
//     for font_path in font_paths {
//         if font_path.ends_with(".ttf") {
//             let font_bytes = cx
//                 .asset_source()
//                 .load(font_path.as_str())?
//                 .expect("Should never be None");
//             embedded_fonts.push(font_bytes);
//         }
//     }
//     cx.text_system().add_fonts(embedded_fonts)
// }

pub fn init(cx: &mut App) {
    cx.activate(true);

    gpui_component::init(cx);
    components::init(cx);
    themes::init(cx);
    // components::init(cx);

    // if let Err(e) = load_embedded_fonts(cx) {
    //     eprintln!("Failed to load embedded fonts: {}", e);
    // }

    let url = server_url_from_env().unwrap_or_else(|| "http://localhost:3000".into());

    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        // KeyBinding::new("cmd-w", actions::CloseWindow, None),
    ]);
    #[cfg(target_os = "windows")]
    cx.bind_keys([
        KeyBinding::new("ctrl-q", Quit, None),
        // KeyBinding::new("ctrl-w", actions::CloseWindow, None),
    ]);
    AppDatabaseStore::initialize_global(url, cx);
    // let db_store = AppDatabaseStore::global(cx);
    // db_store.update(cx, |store, cx| {
    //     store.refresh_pipeline(cx);
    // });

    let mut titlebar_options = if cfg!(target_os = "macos") {
        TitlebarOptions {
            traffic_light_position: Some(point(px(16.), px(16.))),
            ..Default::default()
        }
    } else {
        TitleBar::title_bar_options()
    };
    titlebar_options.title = Some("Subroutine".into());

    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(titlebar_options),
        tabbing_identifier: Some("subroutine".to_string()),
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
            let app_view = cx.new(|cx| RootView::new(window, cx));
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
        disabled: false,
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

fn server_url_from_env() -> Option<String> {
    load_dotenv();

    if let Ok(url) = std::env::var("SUBROUTINE_SERVER_URL") {
        return Some(url);
    }

    let host = std::env::var("SUBROUTINE_HOST").ok()?;
    let port: u16 = std::env::var("SUBROUTINE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    Some(format!("http://{host}:{port}"))
}

fn load_dotenv() {
    let candidates = [
        std::path::PathBuf::from("database.env"),
        std::path::PathBuf::from("../../database.env"),
        std::path::PathBuf::from("../../../database.env"),
    ];
    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, val)) = line.split_once('=') {
                    if std::env::var(key.trim()).is_err() {
                        unsafe { std::env::set_var(key.trim(), val.trim()) };
                    }
                }
            }
            break;
        }
    }
}

fn update_app_menu(cx: &App) {
    let mode = cx.theme().mode;
    cx.set_menus(vec![
        Menu {
            disabled: false,
            name: "Subroutine".into(),
            items: vec![
                MenuItem::action("About", About),
                MenuItem::Separator,
                MenuItem::action("Open...", Open),
                MenuItem::Separator,
                MenuItem::Submenu(Menu {
                    disabled: false,
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
            disabled: false,
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
            disabled: false,
            name: "Window".into(),
            items: vec![MenuItem::action("Toggle Search", ToggleSearch)],
        },
        Menu {
            disabled: false,
            name: "Help".into(),
            items: vec![MenuItem::action("Open Website", Open)],
        },
    ]);
}
