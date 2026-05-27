# gpui-component — Quick Reference

## Setup

Initialize the library once at app startup, before opening any windows. Wrap the root view in `Root`.

```rust
use gpui_component::{Root, init};

Application::new().run(|cx: &mut App| {
    gpui_component::init(cx); // must be first

    cx.open_window(WindowOptions::default(), |window, cx| {
        let view = cx.new(|cx| MyView::new(window, cx));
        cx.new(|cx| Root::new(view.into(), window, cx)) // Root wraps every window
    });
});
```

### Root overlay layers

The root view (first child of `Root`) must render the overlay layers or dialogs, sheets, and notifications will not appear:

```rust
impl Render for MyApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(/* main content */)
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
```

---

## Required Trait Imports

Many methods are gated behind traits. The compiler error will say "no method named X found" — always check these first.

| Trait | Methods unlocked | Import |
|-------|-----------------|--------|
| `gpui::InteractiveElement` | `.id()`, `.on_click()`, `.on_hover()`, `.on_action()`, `.on_key_down()`, event handlers | `use gpui::InteractiveElement;` |
| `gpui::StatefulInteractiveElement` | `.overflow_y_scroll()`, `.on_drag()` | `use gpui::StatefulInteractiveElement;` |
| `gpui::prelude::FluentBuilder` | `.when()`, `.when_some()`, `.map()` | `use gpui::prelude::FluentBuilder;` |
| `gpui::AppContext` | `cx.new()` from within `Context<T>` | `use gpui::AppContext as _;` |
| `gpui_component::ActiveTheme` | `cx.theme()` | `use gpui_component::ActiveTheme;` |
| `gpui_component::Sizable` | `.small()`, `.xsmall()`, `.large()` | `use gpui_component::Sizable;` |
| `gpui_component::Disableable` | `.disabled(bool)` | `use gpui_component::Disableable;` |
| `gpui_component::button::ButtonVariants` | `.primary()`, `.ghost()`, `.outline()`, `.danger()` | `use gpui_component::button::ButtonVariants;` |
| `gpui_component::WindowExt` | `window.push_notification()`, `window.open_dialog()`, `window.close_dialog()` | `use gpui_component::WindowExt;` |

---

## Layout Helpers

```rust
use gpui_component::{h_flex, v_flex};

// h_flex() = div().flex().flex_row().items_center()
h_flex().gap_2().justify_between().child(a).child(b)

// v_flex() = div().flex().flex_col()
v_flex().gap_4().p_4().child(a).child(b)

// overflow_y_scroll requires .id() first
div().id("scrollable").flex_1().overflow_y_scroll()

// .hover() — NOT .hover_bg() (does not exist)
div().id("item").hover(|s| s.bg(cx.theme().list_hover))
```

---

## Theming

```rust
use gpui_component::ActiveTheme;

let theme = cx.theme();

// Surfaces
theme.background           // main window background
theme.foreground           // main text
theme.secondary            // secondary surface
theme.secondary_foreground
theme.muted_foreground     // dimmed / hint text
theme.border               // standard border
theme.input                // input border color
theme.popover              // overlay / popover background
theme.popover_foreground
theme.group_box            // card / group background
theme.group_box_foreground
theme.list_hover           // list item hover background

// Accents and states
theme.primary              // primary brand color
theme.primary_foreground   // text on primary
theme.accent
theme.accent_foreground
theme.destructive          // error / danger
theme.success
theme.warning
theme.info

// Typography and geometry
theme.radius               // default border radius
theme.radius_lg            // larger radius
theme.mode                 // ThemeMode::Light or ThemeMode::Dark

// Named palette colors
theme.red_light
theme.yellow_light
theme.green_light
theme.magenta_light
```

### Applying a custom theme

```rust
use gpui_component::theme::{Theme, ThemeConfig, ThemeConfigColors, ThemeMode};
use std::rc::Rc;

let config = Rc::new(ThemeConfig {
    name: SharedString::from("My Theme"),
    mode: ThemeMode::Dark,
    is_default: true,
    font_family: Some("Inter".into()),
    mono_font_family: Some("JetBrains Mono".into()),
    font_size: Some(13.0),
    mono_font_size: Some(13.0),
    radius: Some(6),
    radius_lg: Some(8),
    shadow: Some(true),
    colors: serde_json::from_str(r#"{
        "background": "#0E1012",
        "foreground": "#B8C0CC",
        "border": "#2D3139",
        "primary.background": "#7CB4C8",
        "primary.foreground": "#F8FAFC"
    }"#).unwrap_or_default(),
    highlight: None,
});

Theme::global_mut(cx).apply_config(&config);
```

### Loading themes from disk (ThemeRegistry)

```rust
use gpui_component::{Theme, ThemeRegistry};

ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, move |cx| {
    if let Some(theme) = ThemeRegistry::global(cx).themes().get("Ayu Light").cloned() {
        Theme::global_mut(cx).apply_config(&theme);
    }
})?;
```

### Syncing with OS appearance

Call `Theme::sync_system_appearance` once a window is available to match the current OS light/dark mode, then observe future OS-level changes:

```rust
// In your open_window callback, after creating the root view:
let _appearance_sub = themes::apply_system_appearance(window, cx).detach();

// Implementation pattern:
pub fn apply_system_appearance(window: &mut Window, cx: &mut App) -> gpui::Subscription {
    // Only sync to system if user has not pinned a mode.
    if user_has_no_explicit_mode() {
        Theme::sync_system_appearance(Some(window), cx);
        cx.refresh_windows();
    }

    // Re-sync whenever the OS toggles dark/light mode.
    window.observe_window_appearance(|window, cx| {
        if user_has_no_explicit_mode() {
            Theme::sync_system_appearance(Some(window), cx);
            cx.refresh_windows();
        }
    })
}
```

The returned `gpui::Subscription` must be kept alive for the window's lifetime — store it in the root view or `.detach()` it if the window never closes independently.

### Persisting theme state

Observe `Theme` globally to write a state file whenever the user changes modes or themes, and read it back on the next launch:

```rust
cx.observe_global::<Theme>(|cx| {
    let snapshot = ThemeState {
        light_theme: Theme::global(cx).light_theme.name.clone(),
        dark_theme:  Theme::global(cx).dark_theme.name.clone(),
        // Store None when following system so next launch also follows system.
        mode: if Theme::global(cx).mode == ThemeMode::from(cx.window_appearance()) {
            None
        } else {
            Some(Theme::global(cx).mode)
        },
    };
    if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
        let _ = std::fs::write("target/state.json", json);
    }
})
.detach();
```

### Theme-switching actions

```rust
use gpui::actions;
use gpui_component::{Theme, ThemeMode};

actions!(themes, [SwitchSystemTheme]);

#[derive(gpui::Action, Clone, PartialEq)]
#[action(namespace = themes, no_json)]
pub struct SwitchThemeMode(pub ThemeMode);

cx.on_action(|switch: &SwitchThemeMode, cx| {
    Theme::change(switch.0, None, cx);
    cx.refresh_windows();
});

cx.on_action(|_: &SwitchSystemTheme, cx| {
    Theme::sync_system_appearance(None, cx);
    cx.refresh_windows();
});
```

---

## Divider

```rust
use gpui_component::divider::Divider;

Divider::horizontal()
    .color(cx.theme().border)
    .w_full()

Divider::horizontal()
    .label("Section")                    // centered label
    .color(cx.theme().muted_foreground)

Divider::vertical()                      // vertical line
```

---

## Icon

```rust
use gpui_component::{Icon, IconName};

Icon::new(IconName::Check)
Icon::new(IconName::Search).small()
Icon::new(IconName::Inbox).text_color(cx.theme().muted_foreground)
```

`IconName` is `Clone` but **not** `Copy`. Use `.clone()` when iterating:

```rust
let icon = icon_ref.clone(); // correct
// let icon = *icon_ref;     // COMPILE ERROR
```

### Available IconName variants

`ALargeSmall`, `ArrowDown`, `ArrowLeft`, `ArrowRight`, `ArrowUp`, `Asterisk`, `Battery`, `BatteryCharging`, `BatteryFull`, `BatteryLow`, `BatteryMedium`, `BatteryWarning`, `Bell`, `BookOpen`, `Bot`, `Building2`, `Calendar`, `CaseSensitive`, `ChartPie`, `Check`, `ChevronDown`, `ChevronLeft`, `ChevronRight`, `ChevronsUpDown`, `ChevronUp`, `CircleCheck`, `CircleUser`, `CircleX`, `Close`, `Copy`, `Cpu`, `Dash`, `Delete`, `Ellipsis`, `EllipsisVertical`, `ExternalLink`, `Eye`, `EyeOff`, `File`, `Folder`, `FolderClosed`, `FolderOpen`, `Frame`, `GalleryVerticalEnd`, `GitHub`, `Globe`, `HardDrive`, `Heart`, `HeartOff`, `Inbox`, `Info`, `Inspector`, `LayoutDashboard`, `Loader`, `LoaderCircle`, `Map`, `Maximize`, `MemoryStick`, `Menu`, `Minimize`, `Minus`, `Moon`, `Network`, `Palette`, `PanelBottom`, `PanelBottomOpen`, `PanelLeft`, `PanelLeftClose`, `PanelLeftOpen`, `PanelRight`, `PanelRightClose`, `PanelRightOpen`, `Pause`, `Play`, `Plus`, `Redo`, `Redo2`, `Replace`, `ResizeCorner`, `Search`, `Settings`, `Settings2`, `SortAscending`, `SortDescending`, `SquareTerminal`, `Star`, `StarFill`, `StarOff`, `Sun`, `ThumbsDown`, `ThumbsUp`, `TriangleAlert`, `Undo`, `Undo2`, `User`, `WindowClose`, `WindowMaximize`, `WindowMinimize`, `WindowRestore`

### Custom icons (AppIcon / IconNamed)

When you need icons beyond the built-in `IconName` set, implement `gpui_component::IconNamed` on your own enum and make it `RenderOnce`. The enum variants map to SVG paths relative to your asset bundle root. The derive macros `IntoElement` and `Clone` are required.

Place custom SVGs in e.g. `assets/icons/custom/` and include them in your `RustEmbed` struct with `#[include = "icons/**/*.svg"]`.

```rust
use gpui::{App, IntoElement, RenderOnce, SharedString, Window};
use gpui_component::{Icon, IconNamed};

#[derive(Clone, Debug, IntoElement)]
pub enum AppIcon {
    House,
    Thermometer,
    Cpu,
    Fan,
    // add more variants as needed
}

impl IconNamed for AppIcon {
    fn path(self) -> SharedString {
        match self {
            Self::House       => "icons/custom/house.svg",
            Self::Thermometer => "icons/custom/thermometer.svg",
            Self::Cpu         => "icons/custom/cpu.svg",
            Self::Fan         => "icons/custom/fan.svg",
        }
        .into()
    }
}

impl RenderOnce for AppIcon {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        Icon::empty().path(self.path())
    }
}
```

Usage is identical to `IconName` — pass the variant anywhere an `impl IntoElement` or `impl Into<Icon>` is expected:

```rust
Icon::new(AppIcon::Thermometer).small()
Button::new("btn").icon(AppIcon::Fan).ghost()
SidebarMenuItem::new("CPU").icon(AppIcon::Cpu)
```

Because `AppIcon` is `Clone` (not `Copy`), clone it if you need it in multiple places in the same expression.

---

## Label

```rust
use gpui_component::label::Label;

Label::new("text")
Label::new("hint").text_sm().text_color(cx.theme().muted_foreground)
Label::new("long text").truncate()
```

---

## Button

```rust
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Sizable, Disableable};

Button::new("id")                       // ElementId only — NOT (id, label)
    .label("Click me")                  // label is a separate method
    .icon(IconName::Check)
    .primary()                          // .ghost() .outline() .danger() .warning() .success() .info() .link() .text()
    .outline()                          // combine with variant for outlined style
    .small()                            // .xsmall() .large() .compact()
    .disabled(true)
    .loading(true)                      // shows spinner, disables interaction
    .selected(true)
    .dropdown_caret(true)               // adds chevron at end
    .tooltip("Hint text")               // SharedString, NOT a closure
    .on_click(|event, window, cx| { })
    .on_click(cx.listener(|this, _, window, cx| { this.do_thing(window, cx); }))
```

### Button variants quick reference

| Method | Appearance |
|--------|-----------|
| `.primary()` | Filled brand color |
| `.ghost()` | Transparent, subtle hover |
| `.outline()` | Border only |
| `.danger()` | Filled destructive red |
| `.warning()` | Filled warning yellow |
| `.success()` | Filled success green |
| `.info()` | Filled info blue |
| `.link()` | Underline link style |
| `.text()` | Plain text, no background |

### Button with custom variant

```rust
use gpui_component::button::ButtonCustomVariant;

let custom = ButtonCustomVariant::new(cx)
    .color(cx.theme().magenta_light)
    .foreground(cx.theme().primary_foreground)
    .border(cx.theme().magenta_light)
    .hover(cx.theme().magenta_light.opacity(0.1))
    .active(cx.theme().magenta_light);

Button::new("btn").custom(custom).label("Custom")
```

### ButtonGroup

```rust
use gpui_component::button::ButtonGroup;

ButtonGroup::new("group")
    .child(Button::new("a").label("One"))
    .child(Button::new("b").label("Two"))
    .child(Button::new("c").label("Three"))

// Toggle group (multiple selection)
ButtonGroup::new("toggle-group")
    .multiple(true)
    .child(Button::new("a").label("Option A").selected(true))
    .child(Button::new("b").label("Option B"))
    .on_click(|selected_indices: &Vec<usize>, _, _| { })
```

### Tag (display-only)

Tags are **not interactive** — no `.on_click()`, no `.id()`. For clickable pill-style elements use a small ghost button instead.

```rust
use gpui_component::tag::Tag;

Tag::primary().child("Label")
Tag::secondary().outline().child("Outline")
Tag::danger().child("Error")
Tag::success().child("OK")
```

---

## Switch

```rust
use gpui_component::switch::Switch;

Switch::new("id")
    .checked(self.enabled)
    .label("Enable feature")
    .on_click(|checked: &bool, window, cx| { })
    .on_click(cx.listener(|this, checked: &bool, _, cx| {
        this.enabled = *checked;
        cx.notify();
    }))
```

---

## Checkbox

The project has a custom `Checkbox` in `src/components/checkbox.rs` that extends the standard one with animated check icons. Usage pattern:

```rust
use crate::components::checkbox::Checkbox;

Checkbox::new("my-check")
    .label("Accept terms")
    .checked(self.accepted)
    .check_started(self.accepted)       // drives the animation start frame
    .on_mouse_up(cx.listener(|this, checked: &bool, _, cx| {
        this.accepted = *checked;
        cx.notify();
    }))
    .small()
    .disabled(false)
```

---

## Progress

```rust
use gpui_component::progress::Progress;

Progress::new("upload").value(0.75_f32)   // value is f32 in [0.0, 1.0]
```

---

## Spinner

```rust
use gpui_component::spinner::Spinner;

Spinner::new()
Spinner::new().color(cx.theme().primary)
Spinner::new().icon(IconName::LoaderCircle)
```

---

## Tooltip

On plain elements — closure-based:

```rust
use gpui_component::tooltip::Tooltip;

div()
    .id("my-el")
    .tooltip(|window, cx| Tooltip::new("Helpful hint").build(window, cx))
```

On `Button` — `SharedString`, **not** a closure:

```rust
Button::new("btn").label("Save").tooltip("Cmd+S")
```

---

## Input / InputState

`InputState` requires `&mut Window` to set a value — it cannot be called from `cx.subscribe()` without `subscribe_in`. Always use `subscribe_in` if you need to set the value in response to an event.

```rust
use gpui_component::input::{Input, InputState, InputEvent};

// Create
let input = cx.new(|cx| {
    InputState::new(window, cx)
        .placeholder("Enter name...")
        .default_value("initial")
});

// Read value
let value: SharedString = input.read(cx).value();

// Set value (requires window)
input.update(cx, |state, cx| {
    state.set_value("new value", window, cx);
});

// Render
Input::new(&self.input)
    .cleanable(true)             // show clear button
    .appearance(false)           // strip default border/bg for custom containers
    .disabled(false)
    .small()

// With prefix / suffix
Input::new(&self.input)
    .prefix(Icon::new(IconName::Search).small())
    .suffix(Button::new("info").ghost().icon(IconName::Info).xsmall())

// Password input
let input = cx.new(|cx| InputState::new(window, cx).masked(true));
Input::new(&self.input).mask_toggle()  // toggle to reveal

// Validation
let input = cx.new(|cx| {
    InputState::new(window, cx)
        .validate(|s, _| s.parse::<f64>().is_ok())
        // or
        .pattern(regex::Regex::new(r"^[a-z]+$").unwrap())
});

// Input mask
let input = cx.new(|cx| {
    InputState::new(window, cx)
        .mask_pattern("(999)-999-9999")
});
```

### Subscribe to input events

```rust
cx.subscribe_in(&self.input, window, |this, state, event: &InputEvent, window, cx| {
    match event {
        InputEvent::Change => {
            let text = state.read(cx).value();
            this.handle_change(text, cx);
        }
        InputEvent::PressEnter { secondary } => { }
        InputEvent::Focus => { }
        InputEvent::Blur => { }
    }
});
```

---

## Slider / SliderState

`SliderState::set_value` requires `&mut Window`. Use `subscribe` (not `subscribe_in`) to read the new value from the event — it is already carried in `SliderEvent::Change`. Only use `subscribe_in` when you need to call `set_value` in response to an *external* event.

```rust
use gpui_component::slider::{Slider, SliderState, SliderEvent, SliderValue};

// Create
let slider = cx.new(|_cx| {
    SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(1.0)
        .default_value(50.0)
});

// Range slider
let range_slider = cx.new(|_cx| {
    SliderState::new()
        .min(0.0)
        .max(100.0)
        .default_value(20.0..80.0)
});

// Subscribe to changes
cx.subscribe(&self.slider, |this, _slider, event: &SliderEvent, cx| {
    match event {
        SliderEvent::Change(value) => {
            this.current_value = value.start();  // .end() for range end
            cx.notify();
        }
    }
});

// Read current value
let value: f32 = self.slider.read(cx).value().start();

// Set value programmatically (requires window — use subscribe_in)
cx.subscribe_in(&self.slider, window, |this, _, event: &SliderEvent, window, cx| {
    this.other_slider.update(cx, |s, cx| {
        s.set_value(new_value, window, cx);
    });
});

// Render
Slider::new(&self.slider)
Slider::new(&self.slider).vertical().h(px(200.))
Slider::new(&self.slider).disabled(true)
```

---

## Select / SelectState

```rust
use gpui_component::select::{Select, SelectState, SelectEvent, SearchableVec};
use gpui_component::IndexPath;

// Basic — built-in SelectItem for &'static str, String, SharedString
let state = cx.new(|cx| {
    SelectState::new(
        vec!["Apple", "Orange", "Banana"],
        Some(IndexPath::default()),  // initial selection; None for none
        window,
        cx,
    )
});
Select::new(&state).placeholder("Pick one...")

// Searchable
let items = SearchableVec::new(vec!["Rust", "Go", "Python"]);
let state = cx.new(|cx| {
    SelectState::new(items, None, window, cx).searchable(true)
});
Select::new(&state).icon(IconName::Search)

// Modifiers
Select::new(&state)
    .cleanable(true)
    .disabled(false)
    .small()
    .appearance(false)
    .w(px(200.))
    .menu_width(px(300.))
    .title_prefix("Language: ")
    .placeholder("Select...")
```

### Custom SelectItem

```rust
use gpui_component::select::SelectItem;

#[derive(Debug, Clone)]
struct Country {
    name: SharedString,
    code: SharedString,
}

impl SelectItem for Country {
    type Value = SharedString;

    fn title(&self) -> SharedString { self.name.clone() }
    fn value(&self) -> &Self::Value { &self.code }

    // Optional: custom rendered title in the selected trigger
    fn display_title(&self) -> Option<AnyElement> {
        Some(h_flex().gap_2()
            .child(self.code.clone())
            .child(self.name.clone())
            .into_any_element())
    }

    // Optional: custom search logic
    fn matches(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(&query.to_lowercase())
            || self.code.to_lowercase().contains(query)
    }
}
```

### Reading and mutating SelectState

```rust
// Read selected value
let value: Option<&SharedString> = state.read(cx).selected_value();

// Set by index
state.update(cx, |state, cx| {
    state.set_selected_index(Some(IndexPath::default().row(2)), window, cx);
});

// Set by value
state.update(cx, |state, cx| {
    state.set_selected_value(&"US".into(), window, cx);
});

// Update items
state.update(cx, |state, cx| {
    state.set_items(new_items, window, cx);
});

// Subscribe
cx.subscribe_in(&state, window, |this, _, event: &SelectEvent, window, cx| {
    if let SelectEvent::Confirm(value) = event {
        // value is Option<&Value>
    }
});
```

---

## Form / Field

```rust
use gpui_component::form::{field, v_form, h_form};

// Vertical form (label above input)
v_form()
    .child(field().label("Name").child(Input::new(&name)))
    .child(field().label("Email").required(true).child(Input::new(&email)))
    .child(field().label("Bio").description("Max 100 words").child(Input::new(&bio)))

// Horizontal form (label left, input right)
h_form()
    .label_width(px(120.))
    .child(field().label("Name").child(Input::new(&name)))

// Multi-column grid
v_form()
    .columns(2)
    .child(field().label("First").child(Input::new(&first)))
    .child(field().label("Last").child(Input::new(&last)))
    .child(field().label("Bio").col_span(2).child(Input::new(&bio)))

// Column positioning
v_form()
    .columns(3)
    .child(field().label("A").col_start(1).col_span(2).child(input_a))
```

### Field modifiers

| Method | Description |
|--------|-------------|
| `.label("text")` | Label text |
| `.required(true)` | Shows asterisk |
| `.description("...")` | Help text below |
| `.visible(bool)` | Conditional visibility |
| `.col_span(n)` | Span n columns |
| `.col_start(n)` | Start at column n |
| `.label_indent(false)` | Suppress label indentation (for button rows) |
| `.items_start()` | Align label to top for tall content |

### Buttons row in a form

```rust
field()
    .label_indent(false)
    .child(
        h_flex().gap_2()
            .child(Button::new("save").primary().label("Save").on_click(cx.listener(|this, _, _, cx| this.submit(cx))))
            .child(Button::new("cancel").ghost().label("Cancel"))
    )
```

---

## Tabs / TabBar

```rust
use gpui_component::tab::{Tab, TabBar};

TabBar::new("tabs")
    .selected_index(self.active_tab)
    .on_click(cx.listener(|this, index: &usize, _, cx| {
        this.active_tab = *index;
        cx.notify();
    }))
    .child(Tab::new().label("Account"))
    .child(Tab::new().label("Profile"))
    .child(Tab::new().label("Settings").disabled(true))
```

### TabBar variants

```rust
TabBar::new("tabs").underline()    // underline indicator
TabBar::new("tabs").pill()         // rounded pill tabs
TabBar::new("tabs").outline()      // outlined tabs
TabBar::new("tabs").segmented()    // segmented control style
```

### TabBar sizes: `.xsmall()`, `.small()`, `.large()`

### Tab with icon

```rust
Tab::new().label("Inbox").prefix(IconName::Inbox)
Tab::new().icon(IconName::Settings)  // icon-only tab
```

### Prefix / suffix controls on the tab bar

```rust
TabBar::new("tabs")
    .prefix(Button::new("back").ghost().xsmall().icon(IconName::ArrowLeft))
    .suffix(Button::new("more").ghost().xsmall().icon(IconName::Ellipsis))
    .menu(true)                        // overflow dropdown when tabs are many
    .child(Tab::new().label("Tab 1"))
```

### Tab with close button

```rust
Tab::new().label(name.clone())
    .suffix(
        Button::new(format!("close-{ix}"))
            .ghost().xsmall().icon(IconName::Close)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.close_tab(ix, cx);
            }))
    )
```

---

## List / ListDelegate

The List uses a delegate pattern. Implement `ListDelegate` for your data type.

```rust
use gpui_component::list::{List, ListState, ListDelegate, ListItem, ListEvent, ListSeparatorItem};
use gpui_component::IndexPath;

struct MyDelegate {
    items: Vec<String>,
    selected: Option<IndexPath>,
}

impl ListDelegate for MyDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, cx: &App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        self.items.get(ix.row).map(|item| {
            ListItem::new(ix)
                .child(Label::new(item.clone()))
                .selected(Some(ix) == self.selected)
        })
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
        cx.notify();
    }
}

// Create state
let state = cx.new(|cx| ListState::new(MyDelegate { items, selected: None }, window, cx));

// Render
List::new(&state).max_h(px(400.))
```

### Searchable list

```rust
// Enable search UI
let state = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));

// Handle search in delegate
fn perform_search(
    &mut self,
    query: &str,
    _window: &mut Window,
    _cx: &mut Context<ListState<Self>>,
) -> Task<()> {
    self.filtered = self.all.iter()
        .filter(|s| s.to_lowercase().contains(query))
        .cloned()
        .collect();
    Task::ready(())
}
```

### Sections

```rust
fn sections_count(&self, _cx: &App) -> usize { 3 }

fn items_count(&self, section: usize, _cx: &App) -> usize {
    self.sections[section].len()
}

fn render_section_header(
    &mut self, section: usize, _window: &mut Window, cx: &mut Context<ListState<Self>>,
) -> Option<impl IntoElement> {
    Some(div().px_2().py_1().text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(format!("Section {}", section + 1)))
}
```

Sections with `items_count` of 0 are automatically hidden (no header rendered).

### Suffix actions on list items

```rust
ListItem::new(ix)
    .child(Label::new(item.clone()))
    .suffix(|_, _| {
        Button::new("action").ghost().small().icon(IconName::Ellipsis)
    })
```

### List events

```rust
cx.subscribe(&state, |this, _, event: &ListEvent, cx| {
    match event {
        ListEvent::Select(ix)  => { /* item highlighted */ }
        ListEvent::Confirm(ix) => { /* Enter pressed on item */ }
        ListEvent::Cancel      => { /* Escape pressed */ }
    }
});
```

### Scrolling control

```rust
state.update(cx, |state, cx| {
    state.scroll_to_item(IndexPath::new(5), ScrollStrategy::Center, window, cx);
    state.scroll_to_selected_item(window, cx);
    state.set_selected_index(Some(IndexPath::new(2)), window, cx);
});
```

---

## Dialog

Requires `Root::render_dialog_layer` in the root render. `WindowExt` is from `gpui_component`, NOT from `gpui`.

```rust
use gpui_component::{WindowExt, dialog::DialogButtonProps};

// Simple dialog
window.open_dialog(cx, |dialog, _, _| {
    dialog.title("Confirm").child("Are you sure?")
});

// Dialog with footer buttons
window.open_dialog(cx, |dialog, _, _| {
    dialog
        .title("Edit")
        .child(v_flex().gap_3().child(Input::new(&self.input)))
        .footer(|_, _, _, _| {
            vec![
                Button::new("cancel").ghost().label("Cancel")
                    .on_click(|_, window, cx| window.close_dialog(cx)),
                Button::new("save").primary().label("Save")
                    .on_click(|_, window, cx| window.close_dialog(cx)),
            ]
        })
});

// Dialog options
window.open_dialog(cx, |dialog, _, _| {
    dialog
        .overlay(true)              // dimmed backdrop (default true)
        .overlay_closable(true)     // click backdrop to close (default true)
        .keyboard(true)             // ESC closes (default true)
        .close_button(true)         // X button (default true)
        .child("content")
});

// Close programmatically
window.close_dialog(cx);
```

### Declarative dialog API

```rust
use gpui_component::dialog::{Dialog, DialogHeader, DialogTitle, DialogDescription, DialogFooter};

Dialog::new(cx)
    .trigger(Button::new("open").outline().label("Open"))
    .content(|content, _, cx| {
        content
            .child(
                DialogHeader::new()
                    .child(DialogTitle::new().child("Account Settings"))
                    .child(DialogDescription::new().child("Update your preferences."))
            )
            .child(v_flex().gap_4().py_4().child(Input::new(&self.input)))
            .child(
                DialogFooter::new()
                    .border_t_1().border_color(cx.theme().border)
                    .child(Button::new("cancel").outline().label("Cancel")
                        .on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("save").primary().label("Save"))
            )
    })
```

---

## Notification (Toast)

Requires `Root::render_notification_layer` in the root render. `WindowExt` is from `gpui_component`.

```rust
use gpui_component::{WindowExt, notification::{Notification, NotificationType}};

// Simple string
window.push_notification("Saved!", cx);

// With type
window.push_notification((NotificationType::Error, "Something failed"), cx);
window.push_notification((NotificationType::Success, "Done!"), cx);
window.push_notification((NotificationType::Warning, "Check this"), cx);
window.push_notification((NotificationType::Info, "FYI"), cx);

// Builder pattern
window.push_notification(
    Notification::new()
        .title("Upload Complete")
        .message("Your file was uploaded.")
        .with_type(NotificationType::Success)
        .autohide(false),   // stays until dismissed
    cx,
);

// With action button
window.push_notification(
    Notification::error("Connection failed.")
        .title("Error")
        .autohide(false)
        .action(|_, cx| {
            Button::new("retry").primary().label("Retry")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.dismiss(window, cx);
                }))
        }),
    cx,
);

// Unique notification (replaceable / removable by type ID)
struct UploadProgress;

window.push_notification(
    Notification::info("Uploading...").id::<UploadProgress>().autohide(false),
    cx,
);
// Later, replace with success:
window.push_notification(
    Notification::success("Done!").id::<UploadProgress>(),
    cx,
);
// Or remove:
window.remove_notification::<UploadProgress>(cx);
```

---

## Popover (gpui-component built-in)

```rust
use gpui_component::popover::Popover;
use gpui::Corner;

Popover::new("my-popover")
    .trigger(Button::new("btn").label("Open").outline())
    .anchor(Corner::BottomLeft)        // TopLeft, TopRight, BottomLeft, BottomRight
    .child("Popover content")

// Dynamic content (called every render — avoid creating entities here)
Popover::new("p")
    .trigger(Button::new("btn").outline().label("Open"))
    .anchor(Corner::BottomLeft)
    .content(|_, _, _| {
        div().p_2().child("Built dynamically")
    })

// Dismiss from inside content
Popover::new("p")
    .trigger(Button::new("btn").outline().label("Open"))
    .content(|_, cx| {
        Button::new("close").label("Close")
            .on_click(cx.listener(|_, _, _, cx| {
                cx.emit(gpui_component::DismissEvent);
            }))
    })

// Controlled open state
Popover::new("p")
    .open(self.is_open)
    .on_open_change(cx.listener(|this, open: &bool, _, cx| {
        this.is_open = *open;
        cx.notify();
    }))
    .trigger(Button::new("btn").outline().label("Open"))
    .child("Content")

// Right-click trigger
Popover::new("context-menu")
    .mouse_button(gpui::MouseButton::Right)
    .anchor(Corner::BottomRight)
    .trigger(Button::new("el").ghost().label("Right-click me"))
    .child("Context menu items here")
```

---

## Common Patterns

### Entity creation from Context\<T\>

```rust
// Requires: use gpui::AppContext as _;
let input = cx.new(|cx| InputState::new(window, cx).placeholder("text"));
```

### Storing subscriptions

```rust
struct MyView {
    _subscriptions: Vec<Subscription>,
}

// In constructor:
let mut subscriptions = Vec::new();
subscriptions.push(cx.subscribe(&entity, |this, _, event: &MyEvent, cx| { }));
subscriptions.push(cx.subscribe_in(&slider, window, |this, _, event: &SliderEvent, window, cx| { }));
Self { _subscriptions: subscriptions }
```

### Conditional rendering

```rust
div()
    .when(condition, |this| this.child("shown when true"))
    .when_some(opt_value, |this, value| this.child(format!("{value}")))
    .map(|this| if condition { this.bg(cx.theme().accent) } else { this })
```

### Polymorphic element types (AnyElement)

`Box<dyn IntoElement>` does **not** work — `IntoElement` is not dyn-compatible.

```rust
use gpui::AnyElement;

let content: AnyElement = match self.page {
    Page::A => self.render_a(cx).into_any_element(),
    Page::B => self.render_b(cx).into_any_element(),
};
div().child(content)
```

### Rust 2024 borrow checker — impl IntoElement in render loops

Calling a helper that takes `&mut Context<Self>` in a loop causes a multi-borrow error. Fix: get `cx.entity()` before the loop and pass it into closures.

```rust
// In render():
let entity = cx.entity();
let mut cards: Vec<_> = self.items.iter().enumerate().map(|(ix, item)| {
    self.render_card(entity.clone(), ix, item, cx)
}).collect();

// Helper takes &App (coerced from &Context<T>), not &mut Context<Self>:
fn render_card(&self, entity: Entity<Self>, ix: usize, item: &Item, cx: &App) -> impl IntoElement {
    div().on_click(move |_, _, cx| {
        entity.update(cx, |this, cx| { this.selected = ix; cx.notify(); });
    })
}
```

### Rust 2024 borrow checker — &str in impl IntoElement helpers

Returning `impl IntoElement` from a helper that takes `label: &str` causes lifetime escape errors. Accept `impl Into<SharedString>` and convert immediately.

```rust
fn render_row(label: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let label: SharedString = label.into();
    div().child(Label::new(label))
}
```

### Closure ownership in lists

When closures in a loop capture a variable, clone before each closure:

```rust
for item in &self.items {
    let item_for_click = item.id.clone();
    let item_for_delete = item.id.clone();
    div()
        .on_click(cx.listener(move |_, _, _, cx| { use_id(&item_for_click); }))
        .on_action(cx.listener(move |_, _: &Delete, _, cx| { use_id(&item_for_delete); }))
}
```

### Collapsible section (without the Collapsible component)

```rust
v_flex()
    .child(
        h_flex().justify_between().items_center()
            .child(Label::new("Section"))
            .child(
                Button::new("toggle")
                    .icon(if self.expanded { IconName::ChevronUp } else { IconName::ChevronDown })
                    .ghost().xsmall()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.expanded = !this.expanded;
                        cx.notify();
                    }))
            )
    )
    .when(self.expanded, |this| this.child(/* expanded content */))
```

### Loading data from stores in constructors

Store methods that spawn tasks take `&self, cx: &Context<StoreType>`. You must call them via `.update()` to get the right context type.

```rust
// CORRECT — cx here is Context<MyStore>
database_store.update(cx, |store, cx| {
    store.load_data(cx);
});

// WRONG — cx here is Context<MyView>, type mismatch
// database_store.read(cx).load_data(cx);
```

---

## Sidebar

### Basic Sidebar setup

`Sidebar` is a generic component. Its type parameter is the item type — typically `SidebarMenu` for grouped nav, or `SidebarMenuItem` directly. Items must implement the `SidebarItem` and `Collapsible` traits.

```rust
use gpui_component::{Side, Collapsible};
// import your project's sidebar types:
use crate::ui::components::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem, SidebarToggleButton};

Sidebar::<SidebarMenu>::new("sidebar")
    .side(Side::Left)
    .collapsible(true)
    .header(my_header_element)
    .footer(my_footer_element)
    .child(
        SidebarMenu::new()
            .child(SidebarMenuItem::new("Home").icon(AppIcon::House))
            .child(SidebarMenuItem::new("Settings").icon(IconName::Settings))
    )
```

### SidebarMenuItem

```rust
SidebarMenuItem::new("Label")
    .icon(AppIcon::Thermometer)          // any impl Into<Icon>
    .active(is_selected)                 // highlighted background
    .on_click(|_, window, cx| { })
    .suffix(|window, cx| {               // trailing element (e.g. a button)
        Button::new("action").xsmall().ghost().icon(IconName::Plus)
    })
    .children([                          // nested submenu
        SidebarMenuItem::new("Sub-item A"),
        SidebarMenuItem::new("Sub-item B"),
    ])
    .disable(is_disabled)
    .context_menu(|menu, window, cx| {
        menu.entry("Rename", None, |window, cx| { })
            .entry("Delete", None, |window, cx| { })
    })
```

Sub-items are shown/hidden via a caret toggle button. The open/closed state is stored with `window.use_keyed_state` so it survives re-renders.

### SidebarToggleButton

```rust
SidebarToggleButton::new()
    .side(Side::Left)
    .collapsed(self.sidebar_collapsed)
    .tooltip_with_action("Toggle Sidebar", &ToggleSidebar, None)
    .on_click(cx.listener(|this, _, _, cx| {
        this.sidebar_collapsed = !this.sidebar_collapsed;
        cx.notify();
    }))
```

### Animated sidebar collapse (transition)

Drive `Sidebar::transition()` with a `gpui_transitions` float (0.0 = collapsed, 1.0 = expanded). The sidebar interpolates its own width between `COLLAPSED_WIDTH` (48 px) and `DEFAULT_WIDTH` (255 px).

```rust
// In your root view struct:
struct RootView {
    sidebar_collapsed: bool,
    sidebar_transition: f32,   // cached evaluated value for this frame
    // ...
}

// In render():
let transition = window
    .use_keyed_transition("sidebar-slide", cx, Duration::from_millis(150), |_, _| {
        !self.sidebar_collapsed as u8 as f32   // initial goal matches current state
    })
    .with_easing(ease_out);

self.sidebar_transition = *transition.evaluate(window, cx);

// Toggle action handler (e.g. inside .on_action):
this.sidebar_collapsed = !this.sidebar_collapsed;
let new_goal = !this.sidebar_collapsed as u8 as f32;
transition.update(cx, |value, cx| {
    *value = new_goal;
    cx.notify();
});

// Pass the evaluated value to the Sidebar:
Sidebar::<SidebarMenu>::new("sidebar")
    .transition(self.sidebar_transition)
    .child(/* ... */)
```

---

## Command Palette

A command palette is a modal overlay with a fuzzy-searchable list of commands. The pattern used in `npcd-rs` defines three pieces: a `Command` data type, a `CommandPaletteState` entity (which owns input and list state), and a `CommandPalette` `RenderOnce` wrapper.

### Command type

```rust
#[derive(Clone)]
pub struct Command {
    pub name: String,
    pub description: Option<String>,
    pub on_select: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl Command {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), description: None, on_select: None }
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into()); self
    }
    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f)); self
    }
    pub fn matches(&self, query: &str) -> bool {
        self.name.to_lowercase().contains(&query.to_lowercase())
    }
}
```

### CommandPaletteExt trait

Define a trait so any view can declare its own command list and open the palette with one call:

```rust
pub trait CommandPaletteExt: Sized {
    fn commands(&self, cx: &mut Context<Self>) -> Vec<Command>;

    fn command_palette(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<CommandPaletteState> {
        let commands = self.commands(cx);
        cx.new(|cx| CommandPaletteState::new(commands, window, cx))
    }
}
```

### Wiring into the root view

Store an `Option<Entity<CommandPaletteState>>` in the view struct. Toggle it with an action:

```rust
struct RootView {
    cmd_palette: Option<Entity<CommandPaletteState>>,
    focus_handle: FocusHandle,
    // ...
}

// Keybinding (in constructor):
cx.bind_keys([KeyBinding::new("ctrl-k", ToggleCommandPalette, None)]);

// Toggle handler:
fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if self.cmd_palette.is_some() {
        self.cmd_palette = None;
        window.focus(&self.focus_handle, cx);
    } else {
        self.cmd_palette = Some(self.command_palette(window, cx));
    }
}

// In render() — render the palette as an absolute overlay when open:
div()
    .size_full()
    .track_focus(&self.focus_handle)
    .on_action(cx.listener(|this, _: &ToggleCommandPalette, window, cx| {
        this.toggle_command_palette(window, cx);
    }))
    .on_action(cx.listener(|this, _: &SelectCommand, window, cx| {
        if let Some(palette) = &this.cmd_palette {
            let executed = cx.update_entity(palette, |p, cx| p.execute_selected(window, cx));
            if executed { this.cmd_palette = None; window.focus(&this.focus_handle, cx); }
        }
        cx.notify();
    }))
    .child(/* main content */)
    .when_some(self.cmd_palette.as_ref(), |this, palette| {
        this.child(CommandPalette::new(palette.clone()))
    })
```

### CommandPalette RenderOnce wrapper

```rust
#[derive(IntoElement)]
pub struct CommandPalette {
    state: Entity<CommandPaletteState>,
}

impl CommandPalette {
    pub fn new(state: Entity<CommandPaletteState>) -> Self {
        Self { state }
    }
}

impl RenderOnce for CommandPalette {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("command-palette")
            .size_full()
            .absolute()
            .inset_0()
            .child(self.state.clone())
    }
}
```

The `CommandPaletteState` entity implements `Render` itself (using the `popover()` overlay shell), so rendering its `Entity<CommandPaletteState>` as a child is all that's needed.
