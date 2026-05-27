use std::path::PathBuf;

use gpui::{Action, Anchor, App, SharedString};
use gpui_component::{
    ActiveTheme as _, Theme, ThemeMode, ThemeRegistry, notification::NotificationSettings,
    scroll::ScrollbarShow,
};
use serde::{Deserialize, Serialize};

const STATE_FILE: &str = "../../target/state.json";
// const THEMES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/themes");
const THEMES_DIR: &str = "../../assets/themes";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct State {
    light_theme: SharedString,
    dark_theme: SharedString,
    mode: ThemeMode,
    scrollbar_show: Option<ScrollbarShow>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            light_theme: SharedString::from("Default Light"),
            dark_theme: SharedString::from("Default Light"),
            mode: ThemeMode::Light,
            scrollbar_show: None,
        }
    }
}

pub fn init(cx: &mut App) {
    let json = std::fs::read_to_string(STATE_FILE).unwrap_or_default();
    tracing::info!("Load themes...");

    let state = serde_json::from_str::<State>(&json).unwrap_or_default();
    let mode = state.mode;
    let applied_theme = match mode {
        ThemeMode::Light => state.light_theme.clone(),
        ThemeMode::Dark => state.dark_theme.clone(),
    };
    if let Err(e) = ThemeRegistry::watch_dir(PathBuf::from(THEMES_DIR), cx, move |cx| {
        if let Some(theme) = ThemeRegistry::global(cx)
            .themes()
            .get(&state.light_theme)
            .cloned()
        {
            Theme::global_mut(cx).light_theme = theme;
        }
        if let Some(theme) = ThemeRegistry::global(cx)
            .themes()
            .get(&state.dark_theme)
            .cloned()
        {
            Theme::global_mut(cx).dark_theme = theme;
        }
        if let Some(theme) = ThemeRegistry::global(cx)
            .themes()
            .get(&applied_theme)
            .cloned()
        {
            Theme::global_mut(cx).apply_config(&theme);
        }
    }) {
        eprintln!("Failed to load themes: {}", e);
    }

    if let Some(scrollbar_show) = state.scrollbar_show {
        Theme::global_mut(cx).scrollbar_show = scrollbar_show;
    }
    Theme::global_mut(cx).notification = NotificationSettings {
        placement: Anchor::BottomRight,
        ..Default::default()
    };

    cx.refresh_windows();

    cx.observe_global::<Theme>(|cx| {
        let snapshot = State {
            light_theme: Theme::global(cx).light_theme.name.clone(),
            dark_theme: Theme::global(cx).dark_theme.name.clone(),
            mode: Theme::global(cx).mode,
            scrollbar_show: Some(cx.theme().scrollbar_show),
        };

        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            let _ = std::fs::write(STATE_FILE, json);
        }
    })
    .detach();

    cx.on_action(|switch: &SwitchTheme, cx| {
        // println!("Switching theme to {}", switch.0);

        if let Some(theme_config) = ThemeRegistry::global(cx).themes().get(&switch.0).cloned() {
            Theme::global_mut(cx).apply_config(&theme_config);
            cx.refresh_windows();
        }
    });

    cx.on_action(|switch: &SwitchThemeMode, cx| {
        // println!("Switching theme mode to {:?}", switch.0);
        Theme::change(switch.0, None, cx);
        cx.refresh_windows();
    });
}

#[derive(Action, Clone, PartialEq)]
#[action(namespace = themes, no_json)]
pub(crate) struct SwitchTheme(pub(crate) SharedString);

#[derive(Action, Clone, PartialEq)]
#[action(namespace = themes, no_json)]
pub(crate) struct SwitchThemeMode(pub(crate) ThemeMode);
