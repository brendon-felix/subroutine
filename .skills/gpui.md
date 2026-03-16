# GPUI — Quick Reference

## Context Types

GPUI has four context types. They form a hierarchy and are passed as `cx` (and `window` when needed).

```rust
App (global)
  └─ Context<T>  (per-entity update, derefs into App)
       └─ AsyncApp / AsyncWindowContext  (held across await points in cx.spawn)
Window           (per-window, passed as separate `window` parameter)
```

### App — global context

```rust
Application::new().run(|cx: &mut App| {
    let entity = cx.new(|cx| MyState::default());
    cx.set_global(AppSettings { theme: Theme::Dark });
    cx.bind_keys([KeyBinding::new("cmd-s", Save, Some("Editor"))]);
    cx.open_window(WindowOptions::default(), |window, cx| {
        cx.new(|cx| Root::new(view, window, cx))
    });
});
```

### Context\<T\> — entity context

Available inside `Render::render()` and entity update closures.

```rust
impl MyView {
    fn do_stuff(&mut self, cx: &mut Context<Self>) {
        cx.notify();                             // trigger re-render
        cx.emit(MyEvent::Changed);               // emit typed event
        cx.spawn(async move |this, cx| { });     // spawn foreground task
        let entity = cx.entity();                // Entity<Self> (cloneable, no borrow)
        let weak   = entity.downgrade();         // WeakEntity<Self>
        let child  = cx.new(|cx| ChildState::default()); // create child entity
        cx.observe(&other, |this, observed, cx| { }).detach();
        cx.subscribe(&other, |this, _, event: &MyEvent, cx| { }).detach();
    }
}
```

### Window — window context

Passed as `window: &mut Window`, comes **before** `cx`. Used for focus, text measurement, drawing, cursor style, animation frames.

```rust
impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        let bounds  = window.bounds();
        window.focus(&self.focus_handle);
        div().child("content")
    }
}
```

### AsyncApp — async context

Provided by `cx.spawn`. Can be held across `.await` points. Entity access only (no focus, no rendering).

```rust
cx.spawn(async move |this, cx: &mut AsyncApp| {
    let data = fetch().await;
    this.update(cx, |view, cx| {
        view.data = Some(data);
        cx.notify();
    })
}).detach();
```

### Context capability matrix

| Operation             | App | Context\<T\> | Window | AsyncApp |
|-----------------------|-----|-------------|--------|----------|
| Create entities       | Yes | Yes         | No     | No       |
| Update entities       | Yes | Yes         | No     | Yes      |
| Read entities         | Yes | Yes         | No     | Yes      |
| Spawn tasks           | No  | Yes         | No     | No       |
| Notify / emit         | No  | Yes         | No     | No       |
| Open windows          | Yes | No          | No     | No       |
| Focus management      | No  | No          | Yes    | No       |
| Text measurement      | No  | No          | Yes    | No       |
| Bind keys             | Yes | No          | No     | No       |

---

## Entities

`Entity<T>` is a strong, reference-counted handle to state of type `T`. `WeakEntity<T>` is a weak handle that always returns `anyhow::Result`.

```rust
// Create
let entity: Entity<MyState> = cx.new(|cx| MyState { count: 0 });

// Read
let count = entity.read(cx).count;
let (a, b) = entity.read_with(cx, |state, cx| (state.a, state.b));

// Update (mutable)
entity.update(cx, |state, cx| {
    state.count += 1;
    cx.notify();
});

// Update with Window access (requires AsyncWindowContext or VisualTestContext)
entity.update_in(cx, |state, window, cx| {
    state.slider.update(cx, |s, cx| s.set_value(1.0, window, cx));
});

// Weak reference
let weak: WeakEntity<MyState> = entity.downgrade();
weak.update(cx, |state, cx| { cx.notify(); })?; // returns Result

// Identity
entity.entity_id()
```

### Critical entity rules

```rust
// ALWAYS use the inner cx inside update closures
entity.update(cx, |state, inner_cx| {
    inner_cx.notify(); // correct
    // cx.notify();    // WRONG — multiple borrow error
});

// NEVER nest entity updates — panics at runtime
entity1.update(cx, |_, cx| {
    entity2.update(cx, |_, cx| { }); // PANIC
});
// Do sequential updates instead:
entity1.update(cx, |_, cx| { });
entity2.update(cx, |_, cx| { });

// Avoid retain cycles: use weak refs in spawned closures
let weak = cx.entity().downgrade();
cx.spawn(async move |cx| {
    weak.update(cx, |view, cx| { cx.notify(); })
}).detach();
```

### Lifecycle

```rust
impl MyComponent {
    fn new(cx: &mut Context<Self>) -> Self {
        cx.on_release(|this, _, _cx| {
            // runs when entity is dropped — clean up resources
        });
        Self { }
    }
}

// Observe when another entity is released
cx.observe_release(&other_entity, |this, released, cx| {
    this.handle_child_dropped(cx);
}).detach();
```

---

## Rendering

### Render trait (stateful views)

An `Entity<T>` where `T: Render` is sometimes called a "view".

```rust
use gpui::{Context, IntoElement, Render, Window, div, prelude::*};

pub struct MyView {
    count: usize,
    focus_handle: FocusHandle,
}

impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.is_focused(window);

        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .track_focus(&self.focus_handle)
            .child(format!("Count: {}", self.count))
            .when(is_focused, |el| el.border_color(cx.theme().primary))
    }
}
```

### RenderOnce trait (stateless components)

Used for one-shot components consumed during rendering. Takes `self`, not `&mut self`.

```rust
use gpui::{IntoElement, RenderOnce, Window, App, div, prelude::*};

#[derive(IntoElement)]
pub struct Badge {
    label: String,
    color: Hsla,
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .px(px(8.))
            .py(px(2.))
            .rounded(px(4.))
            .bg(self.color)
            .child(self.label)
    }
}

// Usage: div().child(Badge { label: "New".into(), color: green() })
```

| Need | Trait |
|------|-------|
| Persistent state, re-renders, tasks | `impl Render` |
| Stateless display / one-shot | `impl RenderOnce` + `#[derive(IntoElement)]` |
| Custom layout/paint control | `impl Element` |

### Conditional rendering

```rust
div()
    .when(condition, |el| el.child("shown when true"))
    .when_some(opt_value, |el, value| el.child(format!("{value}")))
    .children(items.iter().map(|item| div().child(item.name.clone())))
```

### Polymorphic element types (AnyElement)

When a `match` or `if` returns different element types, use `AnyElement`. `Box<dyn IntoElement>` does **not** work — `IntoElement` is not dyn-compatible.

```rust
use gpui::AnyElement;

let content: AnyElement = match self.page {
    Page::A => self.render_a(cx).into_any_element(),
    Page::B => self.render_b(cx).into_any_element(),
};
div().child(content)
```

### SharedString

Use `SharedString` (`&'static str` or `Arc<str>`) for text passed to GPUI APIs to avoid copying.

```rust
use gpui::SharedString;

SharedString::from("literal")
SharedString::from(format!("item-{ix}"))
"literal".into()   // works where Into<SharedString> is expected
```

---

## Styling and Layout

GPUI uses a fluent builder API on `div()`. Layout is **flexbox-based** with Tailwind-style method names.

```rust
use gpui::{div, px, prelude::*};

div()
    // Layout
    .flex()
    .flex_col()                    // column direction (default: row)
    .gap(px(8.))
    .items_center()                // cross-axis alignment
    .justify_between()             // main-axis justification

    // Sizing
    .w(px(200.))
    .h(px(100.))
    .w_full()
    .h_full()
    .size_full()
    .flex_1()                      // fill remaining space
    .flex_shrink_0()
    .min_w(px(100.))
    .max_h(px(400.))
    .overflow_hidden()
    .overflow_y_scroll()           // requires .id() on the element

    // Spacing
    .p(px(16.))
    .px(px(12.))
    .py(px(8.))
    .m(px(4.))

    // Visual
    .bg(color)
    .text_color(color)
    .text_size(px(14.))
    .font_weight(FontWeight::SEMIBOLD)
    .rounded(px(6.))
    .border_1()
    .border_color(color)
    .border_b_1()                  // bottom border only
    .border_r_1()                  // right border only
    .cursor_pointer()
    .text_ellipsis()               // truncate with ellipsis

    // Interactivity (requires .id())
    .id("my-element")
    .hover(|style| style.bg(hover_color))  // NOT .hover_bg() — that method does not exist
    .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, cx| { }))

    // Children
    .child("text")
    .child(other_element)
    .children(items.iter().map(|i| div().child(i.name.clone())))
```

### ElementId

Required on elements that need interactive state (hover, scroll position).

```rust
div().id("my-button")               // string
div().id(("list-item", index))      // tuple — unique per list item
div().id(SharedString::from(...))   // SharedString
```

Without `.id()`, hover and scroll state resets every frame.

### Common layout patterns

```rust
// Horizontal row
div().flex().items_center().gap(px(8.)).child(icon).child(label)

// Vertical stack
div().flex().flex_col().gap(px(4.)).child(title).child(subtitle)

// Fixed sidebar + flexible content
div().flex().size_full()
    .child(div().w(px(200.)).h_full().child(sidebar))
    .child(div().flex_1().h_full().child(content))

// Scrollable list
div()
    .id("scroll-container")
    .flex_1()
    .overflow_y_scroll()
    .children(items.iter().enumerate().map(|(ix, item)| {
        div()
            .id(("list-item", ix))
            .px(px(12.))
            .py(px(8.))
            .hover(|s| s.bg(hover_color))
            .child(item.name.clone())
    }))
```

---

## Input Events

Event handlers are registered on elements. The `cx.listener` helper gives access to the current entity.

```rust
div()
    .on_click(|event: &ClickEvent, window, cx| { })
    .on_click(cx.listener(|this: &mut T, event, window, cx| {
        this.handle_click(window, cx);
    }))
    .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| { }))
    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { }))
    .on_hover(cx.listener(|this, hovered: &bool, window, cx| { }))
    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| { }))
    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
        match event.keystroke.key.as_str() {
            "enter" => this.submit(cx),
            _ => cx.propagate(), // bubble to parent
        }
    }))
    .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
```

---

## Actions

Actions are dispatched via keyboard bindings or imperatively in code.

### Define actions

```rust
// Simple actions (no data)
use gpui::actions;
actions!(editor, [MoveUp, MoveDown, Save, Quit]);

// Actions with data
#[derive(Clone, PartialEq, Action, Deserialize)]
#[action(namespace = editor)]
pub struct InsertText {
    pub text: String,
}
```

Doc comments on actions are displayed to the user in key-binding UI.

### Bind keys

```rust
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up",    MoveUp,   Some("Editor")), // context-scoped
        KeyBinding::new("cmd-s", Save,     Some("Editor")),
        KeyBinding::new("cmd-q", Quit,     None),           // global
        KeyBinding::new("escape", CloseModal, Some("Modal")),
    ]);
}
```

Key format: `"cmd-s"`, `"ctrl-c"`, `"alt-f"`, `"shift-tab"`, `"cmd-ctrl-f"`, `"f1"`–`"f12"`, `"up"` / `"down"` / `"enter"` / `"escape"` / `"space"` / `"backspace"`.

### Handle actions

```rust
impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Editor")          // must match binding context
            .on_action(cx.listener(Self::move_up))
            .on_action(cx.listener(Self::save))
            .child("content")
    }
}

impl Editor {
    fn move_up(&mut self, _: &MoveUp, cx: &mut Context<Self>) {
        self.cursor -= 1;
        cx.notify();
    }
}
```

### Dispatch actions imperatively

```rust
window.dispatch_action(Save.boxed_clone(), cx);
focus_handle.dispatch_action(&Save, window, cx);
```

---

## Focus Management

### Setup

```rust
struct MyView {
    focus_handle: FocusHandle,
}

impl MyView {
    fn new(cx: &mut Context<Self>) -> Self {
        Self { focus_handle: cx.focus_handle() }
    }
}

// Required to receive keyboard events
impl Focusable for MyView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
```

### Rendering with focus

```rust
impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle) // enable focus tracking
            .on_focus(cx.listener(|this, _, cx| { cx.notify(); }))
            .on_blur(cx.listener(|this, _, cx| { cx.notify(); }))
            .on_key_down(cx.listener(Self::on_key_down))
            .child("focusable content")
    }
}
```

### Focus operations

```rust
window.focus(&self.focus_handle);                 // give focus
cx.blur();                                         // remove focus

self.focus_handle.is_focused(window)               // exact match
self.focus_handle.contains_focused(window)         // this or any descendant
```

### Auto-focus on mount

```rust
impl MyDialog {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle); // focus immediately on creation
        Self { focus_handle }
    }
}
```

Elements with `.track_focus()` automatically participate in Tab / Shift-Tab navigation.

---

## Entity Events

### Define, declare, and emit

```rust
use gpui::EventEmitter;

#[derive(Clone)]
enum MyEvent {
    DataUpdated(String),
    ActionTriggered,
}

// Marker trait — required before cx.emit() will compile
impl EventEmitter<MyEvent> for MyComponent {}

impl MyComponent {
    fn update_data(&mut self, data: String, cx: &mut Context<Self>) {
        self.data = data.clone();
        cx.emit(MyEvent::DataUpdated(data));
        cx.notify();
    }
}
```

### Subscribe to events

```rust
// No window access needed
let sub = cx.subscribe(&source, |this, _emitter, event: &MyEvent, cx| {
    match event {
        MyEvent::DataUpdated(data) => this.handle_update(data, cx),
        MyEvent::ActionTriggered   => this.handle_action(cx),
    }
});

// Window access needed in handler (e.g., to call set_value on a slider or input)
let sub = cx.subscribe_in(&source, window, |this, _emitter, event: &MyEvent, window, cx| {
    this.slider.update(cx, |s, cx| s.set_value(1.0, window, cx));
});
```

Store subscriptions to keep them alive — they deregister when dropped:

```rust
struct MyView {
    _subscriptions: Vec<Subscription>,
}
// In constructor:
let mut subscriptions = Vec::new();
subscriptions.push(cx.subscribe(&entity, |this, _, event: &Event, cx| { }));
Self { _subscriptions: subscriptions }
```

### Observe state changes (fires on cx.notify())

```rust
cx.observe(&entity, |this, observed, cx| {
    let value = observed.read(cx).value;
    this.sync(value, cx);
}).detach();
```

| Mechanism | Fires when | Use for |
|-----------|-----------|---------|
| `cx.subscribe()` | `cx.emit(event)` called | Typed event handling |
| `cx.observe()`   | `cx.notify()` called     | React to any state change |

---

## Concurrency

All entity and UI rendering work occurs on a single **foreground thread**.

### Foreground tasks

```rust
impl MyView {
    fn fetch_data(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let data = fetch_from_api().await;
            this.update(cx, |view, cx| {
                view.data = Some(data);
                cx.notify();
            })
        }).detach();
    }
}
```

`this` inside `cx.spawn` is `WeakEntity<Self>` — automatically provided.

When `cx` is `Context<T>`, the spawn signature is `cx.spawn(async move |handle: WeakEntity<T>, cx| ...)`.

### Background tasks

```rust
cx.background_spawn(async move {
    heavy_computation()      // worker thread, no entity access
})
.then(cx.spawn(move |result, cx| {
    this.update(cx, |view, cx| {
        view.result = result;
        cx.notify();
    }).ok();
}))
.detach();
```

### Periodic tasks / timers

```rust
let task = cx.spawn(async move |this, cx: &mut AsyncApp| {
    loop {
        cx.background_executor().timer(Duration::from_secs(5)).await;
        if this.update(cx, |view, cx| { view.refresh(); cx.notify(); }).is_err() {
            break; // entity was dropped
        }
    }
});
self._refresh_task = task; // store to keep alive; drop to cancel
```

### Task lifecycle rules

- `Task<R>` is a future. Drop it to cancel.
- `.detach()` — fire-and-forget, runs indefinitely.
- `.detach_and_log_err(cx)` — fire-and-forget, logs errors.
- Store in a struct field (`_task: Task<()>`) to tie lifetime to the entity.
- Overwrite a stored `Task` to cancel the previous in-flight operation.

```rust
struct MyView {
    _refresh_task: Option<Task<()>>,  // stored: cancelled when view drops
}

// Debounced search: overwrite to cancel previous
self.search_task = Some(cx.spawn(async move |this, cx| {
    cx.background_executor().timer(Duration::from_millis(200)).await;
    let results = search(query).await;
    this.update(cx, |view, cx| { view.results = results; cx.notify(); })
}));
```

### Animation (declarative)

```rust
use gpui::Animation;

div().with_animation(
    "fade-in",
    Animation::new(Duration::from_millis(300)).with_easing(ease_in_out),
    |el, progress| el.opacity(progress),
)
// Easings: ease_in_out, ease_out_quint, linear, bounce, pulsating_between
// Options: .repeat() for looping
```

### Animation (manual frame-by-frame)

```rust
window.request_animation_frame(cx.listener(|this, _event, window, cx| {
    this.advance_animation();
    cx.notify();
    window.request_animation_frame(cx.listener(Self::on_frame)); // re-schedule
}));
```

---

## Global State

```rust
use gpui::Global;

#[derive(Clone)]
struct AppSettings {
    theme: Theme,
    language: String,
}
impl Global for AppSettings {}

// Set once at startup
cx.set_global(AppSettings { theme: Theme::Dark, language: "en".into() });

// Read anywhere
let settings = cx.global::<AppSettings>();

// Update (does NOT auto-trigger re-renders — notify manually)
cx.update_global::<AppSettings, _>(|settings, cx| {
    settings.theme = Theme::Light;
});
```

**Use Globals for:** app-wide config, shared services, feature flags, read-mostly data.
**Use Entities for:** component state, frequently changing data, anything needing `cx.notify()` reactivity or subscriptions.

### Private Globals wrapping an Entity

The most common pattern for a singleton service (driver, store, session) is to wrap an `Entity<T>` in a private newtype that implements `Global`. This keeps the `Global` type out of the public API — callers get a cloned `Entity<T>` back rather than direct access to the wrapper.

```rust
use gpui::{App, Entity, Global};

pub struct Driver { /* ... */ }

// Private wrapper — never exposed beyond this module
struct GlobalDriver(Entity<Driver>);
impl Global for GlobalDriver {}

impl Driver {
    /// Call once at startup (e.g. inside the `app.run(init)` callback).
    pub fn initialize_global(cx: &mut App) -> anyhow::Result<()> {
        if cx.has_global::<GlobalDriver>() {
            return Ok(());
        }
        let driver = cx.new(|_cx| Driver::new()?);
        cx.set_global(GlobalDriver(driver));
        Ok(())
    }

    /// Cheap clone of the entity handle — safe to call from anywhere.
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalDriver>().0.clone()
    }
}
```

Usage from a view or another global:

```rust
let driver = Driver::global(cx);
let value  = driver.read(cx).some_field;
driver.update(cx, |d, cx| { d.do_thing(); cx.notify(); });
```

Key rules:
- **Guard with `cx.has_global` before `cx.set_global`** so double-init is a no-op rather than a panic.
- **Return `anyhow::Result` from `initialize_global`** — surface early-init failures to the caller instead of panicking.
- **Wrap heavy non-GPUI resources in `Arc`** when the inner type needs to be cloned cheaply across threads (e.g. a driver library handle).

### Observing a Global

```rust
cx.observe_global::<AppSettings>(|cx| {
    // fires whenever cx.set_global / cx.update_global is called
    let settings = cx.global::<AppSettings>();
    // persist, propagate, refresh windows, etc.
})
.detach();
```

---

## Custom Elements (Low-Level)

Prefer `Render`/`RenderOnce` for standard UI. Use `impl Element` only for custom layout algorithms, performance-critical rendering (terminal, canvas), or direct GPU draw calls.

### Three-phase rendering

```
request_layout  →  (LayoutId, RequestLayoutState)
prepaint        →  PrepaintState
paint           →  (side effects: paint calls, mouse event handlers)
```

Hitboxes must be created in **prepaint**, never in paint.

### Minimal implementation

```rust
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior,
    InspectorElementId, IntoElement, LayoutId, Pixels, Size, Style, Window, px, size,
};

pub struct MyElement { /* fields */ }

impl IntoElement for MyElement {
    type Element = Self;
    fn into_element(self) -> Self::Element { self }
}

impl Element for MyElement {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> { None }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            size: Size { width: gpui::relative(1.).into(), height: gpui::relative(1.).into() },
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) -> Hitbox {
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut (),
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.paint_quad(gpui::fill(bounds, cx.theme().background));

        window.on_mouse_event({
            let hitbox = hitbox.clone();
            move |event: &gpui::MouseDownEvent, phase, window, _cx| {
                if hitbox.is_hovered(window) && phase.bubble() {
                    // handle click
                }
            }
        });

        window.set_cursor_style(gpui::CursorStyle::PointingHand, hitbox);
    }
}
```

### Paint operations

```rust
// Rectangle
window.paint_quad(gpui::fill(bounds, color));

// Text: shape then paint
let run = TextRun { len: text.len(), font, color: text_color, background_color: None,
                    underline: None, strikethrough: None };
let shaped = window.text_system().shape_line(SharedString::from(text), font_size, &[run], None);
// shaped.width — measured Pixels
shaped.paint(origin, line_height, window, cx)?;

// Other
window.paint_path(path, fill_color);
window.paint_svg(bounds, svg_path, color);
window.paint_image(bounds, image);
window.paint_shadows(bounds, shadows);
```

---

## Notify

Call `cx.notify()` whenever state changes in a way that affects rendering.

```rust
// Batch: mutate all fields first, notify once
self.field1 = a;
self.field2 = b;
cx.notify();  // single re-render

// Conditional: skip notify if nothing changed
fn set_value(&mut self, new: i32, cx: &mut Context<Self>) {
    if self.value != new {
        self.value = new;
        cx.notify();
    }
}
```

---

## Per-Frame Keyed State (use_keyed_state)

`Window::use_keyed_state` stores arbitrary non-animated values across renders, keyed by an `ElementId`-compatible key. State is initialised once via a closure and then persists as long as the same key is used.

```rust
// In render() or prepaint():
let is_open = window.use_keyed_state(
    "my-section-open",          // key: any impl Into<ElementId>
    cx,
    |_window, _cx| false,       // initialiser — runs only on first call for this key
);

// Read
let open: bool = *is_open.read(cx);

// Write (must call cx.notify() to trigger re-render)
is_open.update(cx, |val, cx| {
    *val = true;
    cx.notify();
});
```

The return type is `Entity<T>`, so you can subscribe to it, observe it, or clone the handle. This is the idiomatic alternative to storing per-item open/closed or hover state inside a parent view's struct when the state is purely local to a render site.

**vs. `use_keyed_transition`:** Use `use_keyed_state` for discrete, non-animated values. Use `use_keyed_transition` (from the `gpui_transitions` crate) for smoothly interpolated values.

---

## App Setup Patterns

### Embedding assets with rust-embed

Use the `RustEmbed` derive macro to bundle fonts, SVG icons, and images into the binary. Implement `AssetSource` so GPUI can load them by path.

```rust
use gpui::{App, AssetSource, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "./assets"]
#[include = "fonts/"]
#[include = "icons/**/*.svg"]
#[include = "images/**/*.png"]
pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow::anyhow!("Asset not found: {}", path))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| if p.starts_with(path) { Some(p.into()) } else { None })
            .collect())
    }
}

// Register at startup:
gpui_platform::application()
    .with_assets(AppAssets)
    .run(init);
```

### Loading embedded fonts

After `AppAssets` is registered, iterate the `fonts/` prefix and push all `.ttf` bytes into the text system:

```rust
fn load_embedded_fonts(cx: &App) -> anyhow::Result<()> {
    let font_paths = cx.asset_source().list("fonts")?;
    let mut embedded_fonts = Vec::new();
    for font_path in font_paths {
        if font_path.ends_with(".ttf") {
            if let Some(bytes) = cx.asset_source().load(font_path.as_str())? {
                embedded_fonts.push(bytes);
            }
        }
    }
    cx.text_system().add_fonts(embedded_fonts)
}
```

Call this inside the `init` callback after `gpui_component::init(cx)`.

### Application menus

```rust
use gpui::{Menu, MenuItem, actions};

actions!(app, [About, Quit, CloseWindow]);

fn app_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "MyApp".into(),
            items: vec![
                MenuItem::action("About", About),
                MenuItem::Separator,
                MenuItem::Submenu(Menu {
                    name: "Appearance".into(),
                    items: vec![
                        MenuItem::action("Light", SwitchLightTheme),
                        MenuItem::action("Dark", SwitchDarkTheme),
                    ],
                }),
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
                MenuItem::action("Cut",   gpui_component::input::Cut),
                MenuItem::action("Copy",  gpui_component::input::Copy),
                MenuItem::action("Paste", gpui_component::input::Paste),
                MenuItem::separator(),
                MenuItem::action("Select All", gpui_component::input::SelectAll),
            ],
        },
    ]
}

// Register in init:
cx.set_menus(app_menus());
```

### QuitMode and window lifecycle

```rust
use gpui::QuitMode;

gpui_platform::application()
    .with_quit_mode(QuitMode::LastWindowClosed) // quit when the last window closes
    .run(init);
```

Inside `open_window`, attach a close handler:

```rust
window.on_window_should_close(cx, |_, cx| {
    cx.quit();
    true
});
```

---

## Error Handling Utilities (ResultExt pattern)

Define a `ResultExt` extension trait to give `Result<T>` three cheap combinators without pulling in extra crates:

```rust
use anyhow::Result;

pub trait ResultExt<T> {
    /// Log the error to stderr and discard it.
    fn log_err(self);
    /// Show the error as a toast notification and return None.
    fn notify_err(self, window: &mut Window, cx: &mut App) -> Option<T>;
    /// Show a fatal dialog then panic.
    fn panic_on_err(self) -> T;
}

impl<T> ResultExt<T> for Result<T> {
    fn log_err(self) {
        if let Err(err) = self {
            eprintln!("Error: {err}");
        }
    }

    fn notify_err(self, window: &mut Window, cx: &mut App) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(err) => {
                let msg = err.to_string();
                eprintln!("Error: {msg}");
                window.push_notification(
                    (gpui_component::notification::NotificationType::Error,
                     gpui::SharedString::from(msg)),
                    cx,
                );
                None
            }
        }
    }

    fn panic_on_err(self) -> T {
        self.unwrap_or_else(|err| panic!("{err}"))
    }
}
```

Usage:

```rust
entity.update(cx, |view, cx| { cx.notify(); }).log_err();

load_embedded_fonts(cx).panic_on_err();

Driver::initialize_global(cx)
    .notify_err(window, cx);
```

---

## Recent API Changes — Never Use the Old APIs

| Old (NEVER use)      | New (always use)                          |
|----------------------|-------------------------------------------|
| `Model<T>`, `View<T>` | `Entity<T>`                              |
| `ModelContext<T>`    | `Context<T>`                              |
| `WindowContext`      | `Window` + `&mut App` (or `Context<T>`)  |
| `ViewContext<T>`     | `Window` + `Context<T>`                  |
| `cx.spawn(|cx| ...)` | `cx.spawn(async move \|cx\| ...)` (async closure) |
| `use_keyed_transition` (gpui_transitions) | `Window::use_keyed_state(key, cx, init)` |
| `.hover_bg(color)`   | `.hover(\|style\| style.bg(color))`      |

---

## Rust 2024 Borrow Checker Gotchas

### impl IntoElement helpers in render loops

In Rust 2024, `impl IntoElement` in return position captures all input lifetimes. Calling a helper that takes `&mut Context<Self>` inside a loop causes a multi-borrow error. Fix: obtain `cx.entity()` before the loop and use `entity.update(cx, ...)` in closures; change the helper to take `&App` instead.

```rust
// In render():
let entity = cx.entity();  // obtain once — doesn't borrow cx
let mut cards = Vec::new();
for (ix, item) in self.items.iter().enumerate() {
    cards.push(self.render_card(entity.clone(), ix, item, cx));
}

// Helper: takes &App (coerced from &Context<T>), not &mut Context<Self>
fn render_card(&self, entity: Entity<Self>, ix: usize, item: &Item, cx: &App) -> impl IntoElement {
    div().on_click(move |_, _, cx| {
        entity.update(cx, |this, cx| { this.selected = ix; cx.notify(); });
    })
}
```

### &str in impl IntoElement helpers

Returning `impl IntoElement` from a helper that takes `label: &str` causes "borrowed data escapes" errors in Rust 2024. Accept `impl Into<SharedString>` and convert immediately.

```rust
fn render_row(label: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let label: SharedString = label.into();  // convert before any element builder
    div().child(Label::new(label))
}
```

---

## Error Handling Rules

- Never use `.unwrap()`. Propagate with `?` or handle explicitly.
- Never silently discard errors with `let _ =` on fallible operations.
  - Propagate with `?` when the caller should handle it.
  - Use `.log_err()` or similar when you want visibility but not propagation.
  - Use `match` or `if let Err(...)` for custom logic.
- When async operations fail, propagate errors to the UI layer.

```rust
// WRONG
let _ = entity.update(cx, |view, cx| { cx.notify(); });

// RIGHT — propagate
entity.update(cx, |view, cx| { cx.notify(); })?;

// RIGHT — log
entity.update(cx, |view, cx| { cx.notify(); }).log_err();
```

---

## gpui_transitions Crate

Provides animated value interpolation. Requires a `[patch.crates-io]` entry in the workspace `Cargo.toml` to unify the crates.io `gpui` used by the crate with the git `gpui` used by the desktop crate:

```toml
[patch.crates-io]
gpui = { git = "https://github.com/zed-industries/zed" }
```

```rust
use gpui_transitions::WindowUseTransition;

// In a render/prepaint context:
let hover = window
    .use_keyed_transition("hover", cx, Duration::from_millis(150), |_, _| 0.0_f32)
    .with_easing(ease_in_out);

let value = *hover.evaluate(window, cx);          // current interpolated value
hover.update(cx, |v, cx| { *v = 1.0; cx.notify(); }); // set new goal
```

`Lerp` is implemented for: `f32`, `f64`, `Point<T>`, `Size<T>`, `Pixels`, `Rgba`, and other GPUI geometry types.

---

## Custom Easing Functions

The `gpui_transitions` crate accepts any `impl Fn(f32) -> f32 + 'static` as an easing function. You can define your own module of reusable easing curves:

```rust
mod transitions {
    use std::f32::consts::PI;

    pub fn ease_in(t: f32) -> f32 { t * t }
    pub fn ease_out(t: f32) -> f32 { 1.0 - (1.0 - t).powi(2) }
    pub fn ease_in_out(t: f32) -> f32 {
        if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
    }

    /// Returns a closure — call it to get the function: `.with_easing(ease_out_quint())`
    pub fn ease_out_quint() -> impl Fn(f32) -> f32 {
        |delta| 1.0 - (1.0 - delta).powi(5)
    }

    pub fn ease_in_cubic(t: f32)     -> f32 { t * t * t }
    pub fn ease_out_cubic(t: f32)    -> f32 { 1.0 - (1.0 - t).powi(3) }
    pub fn ease_in_out_cubic(t: f32) -> f32 {
        if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
    }

    pub fn ease_in_elastic(t: f32) -> f32 {
        if t == 0.0 || t == 1.0 { return t; }
        let c4 = (2.0 * PI) / 3.0;
        (-(2.0_f32.powf(10.0 * (t - 1.0))) * ((t - 1.0) * c4 - PI / 2.0).sin()).clamp(0.0, 1.0)
    }
    pub fn ease_out_elastic(t: f32) -> f32 {
        if t == 0.0 || t == 1.0 { return t; }
        let c4 = (2.0 * PI) / 3.0;
        (2.0_f32.powf(-10.0 * t) * (t * c4 - PI / 2.0).sin() + 1.0).clamp(0.0, 1.0)
    }

    pub fn ease_out_bounce(t: f32) -> f32 {
        let (n1, d1) = (7.5625, 2.75);
        if      t < 1.0 / d1 { n1 * t * t }
        else if t < 2.0 / d1 { let t = t - 1.5  / d1; n1 * t * t + 0.75 }
        else if t < 2.5 / d1 { let t = t - 2.25 / d1; n1 * t * t + 0.9375 }
        else                  { let t = t - 2.625/ d1; n1 * t * t + 0.984375 }
    }
    pub fn ease_in_bounce(t: f32)     -> f32 { 1.0 - ease_out_bounce(1.0 - t) }

    pub fn ease_in_back(t: f32) -> f32 {
        let (c1, c3) = (1.70158, 2.70158);
        (c3 * t * t * t - c1 * t * t).clamp(0.0, 1.0)
    }
    pub fn ease_out_back(t: f32) -> f32 {
        let (c1, c3) = (1.70158, 2.70158);
        (1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)).clamp(0.0, 1.0)
    }

    /// Loops back and forth between `min` and `max`.
    pub fn pulsating_between(min: f32, max: f32) -> impl Fn(f32) -> f32 {
        move |t| min + ((t * 2.0 * PI).sin() * 0.5 + 0.5) * (max - min)
    }
}
pub use transitions::*;
```

Note that some functions return a **closure** (e.g. `ease_out_quint()`, `pulsating_between(a, b)`) while most take `t: f32` directly. Pass closure-returning functions by calling them: `.with_easing(ease_out_quint())`.

---

## Rust Coding Conventions

- Prefer `?` over `unwrap()` / `expect()`.
- Be careful with indexing — prefer `.get(i)` to avoid out-of-bounds panics.
- Use full words for variable names — no abbreviations like `q` for `queue`.
- Do not write comments that summarize code. Only comment when explaining a non-obvious "why".
- Use variable shadowing to scope clones in async contexts:

```rust
executor.spawn({
    let task_ran = task_ran.clone();
    async move {
        *task_ran.borrow_mut() = true;
    }
});
```

- When creating new crates, prefer specifying the library root path in `Cargo.toml`:

```toml
[lib]
path = "src/my_crate.rs"
```

- Run `./script/clippy` instead of `cargo clippy`.