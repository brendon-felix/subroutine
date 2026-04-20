use anyhow::Result;
use gpui::{
    App, AppContext, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds,
    WindowOptions, actions, point, px, size,
};
use gpui_component::{ActiveTheme, Root, Theme, ThemeMode, ThemeRegistry, input};

use crate::assets::AppAssets;
use crate::stores::DatabaseStore;
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
    themes::init(cx);
    components::init(cx);

    // if let Err(e) = load_embedded_fonts(cx) {
    //     eprintln!("Failed to load embedded fonts: {}", e);
    // }

    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    let database_store = cx.new(move |cx| DatabaseStore::new(cx));
    database_store.update(cx, |store, cx| {
        store.initialize(cx);
    });

    let mut titlebar_options = TitlebarOptions::default();
    titlebar_options.appears_transparent = true;
    titlebar_options.traffic_light_position = Some(point(px(10.), px(10.)));

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
            let app_view = cx.new(|cx| RootView::new(database_store.clone(), window, cx));
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
