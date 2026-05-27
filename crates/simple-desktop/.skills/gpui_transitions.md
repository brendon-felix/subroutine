# gpui_transitions — Quick Reference

## Setup

The crate is a local path dependency. Because it depends on `gpui` from crates.io, and the desktop crate depends on `gpui` from the Zed git repo, a `[patch.crates-io]` entry is **required** in the workspace `Cargo.toml` to unify the two versions. Without it, GPUI types from the two sources are treated as incompatible and `Lerp` impls and `WindowUseTransition` methods will not work.

```toml
# workspace Cargo.toml
[patch.crates-io]
gpui = { git = "https://github.com/zed-industries/zed" }
```

Import the extension trait wherever you create transitions:

```rust
use gpui_transitions::WindowUseTransition;
```

---

## Core Types

| Type | Role |
|------|------|
| `Transition<T>` | The handle you interact with — create, evaluate, and update |
| `TransitionState<T>` | Internal state held in a GPUI `Entity<TransitionState<T>>` — rarely used directly |
| `Lerp` | Trait that `T` must implement to be animated |
| `BoolLerp<N>` | Wrapper for animating boolean on/off states as smooth numeric values |
| `WindowUseTransition` | Extension trait on `Window` — provides `use_transition` and `use_keyed_transition` |

---

## Creating Transitions

### use_keyed_transition (recommended)

State is associated with the key and persists across renders as long as the key is unchanged. The `initial_goal` closure is only called when the state is first created.

```rust
use gpui_transitions::WindowUseTransition;
use std::time::Duration;

// In render() or prepaint():
let opacity = window.use_keyed_transition(
    "fade-in",                            // key: any impl Into<ElementId>
    cx,
    Duration::from_millis(200),
    |_window, _cx| 0.0_f32,              // initial goal
);
```

The key can be any type that implements `Into<ElementId>`: `&str`, `String`, `usize`, `SharedString`, tuple IDs, etc.

### use_transition (unkeyed)

State is recreated on each render using the call-site source location as the key. Use this when you have only one transition of a given type per render site and don't need to distinguish between them by a runtime value.

```rust
let opacity = window.use_transition(
    cx,
    Duration::from_millis(200),
    |_window, _cx| 0.0_f32,
);
```

---

## Evaluating the Current Value

`evaluate` returns a `Ref<'_, T>` — a borrowed reference to the current interpolated value, cached for the duration of the current frame. If the transition is still in progress it automatically requests an animation frame to continue animating.

```rust
let value: Ref<'_, f32> = opacity.evaluate(window, cx);
let value: f32 = *opacity.evaluate(window, cx);   // deref to copy
```

For reading just the progress delta (0.0 → 1.0, after easing):

```rust
let delta: f32 = opacity.evaluate_delta(cx);
```

For reading the end goal without triggering frame requests:

```rust
let goal: &f32 = opacity.read_goal(cx);
```

For reading the cached value from the last evaluated frame (returns `None` if not yet evaluated):

```rust
let cached: Ref<'_, Option<f32>> = opacity.read_cache();
```

---

## Updating the Goal

Call `transition.update(...)` to change the target. The transition then animates from the current interpolated position toward the new goal. You must call `cx.notify()` inside the closure to trigger a re-render — `update` does not do this automatically.

```rust
opacity.update(cx, |value, cx| {
    *value = 1.0;
    cx.notify();
});
```

`update` returns `true` if the goal actually changed (new value differs from previous goal), `false` otherwise. You can use this to avoid unnecessary notifies:

```rust
let changed = opacity.update(cx, |value, _cx| {
    *value = if hovered { 1.0 } else { 0.5 };
});
if changed {
    cx.notify();
}
```

---

## Easing

Chain `.with_easing(fn)` immediately after creating the transition. The easing function receives a delta in `[0.0, 1.0]` and returns a remapped delta in `[0.0, 1.0]`.

```rust
use gpui::ease_in_out; // or other easing fns from gpui

let opacity = window
    .use_keyed_transition("fade", cx, Duration::from_millis(150), |_, _| 0.0_f32)
    .with_easing(ease_in_out);
```

Common easing functions available from `gpui`:

| Function | Description |
|----------|-------------|
| `ease_in_out` | Slow start, fast middle, slow end |
| `ease_out_quint` | Fast start, slow end |
| `linear` | Constant rate |
| `bounce` | Bounces at the end |
| `pulsating_between(a, b)` | Loops back and forth |

Any `impl Fn(f32) -> f32 + 'static` is accepted, so you can pass a custom curve.

---

## Continuous vs Non-Continuous Mode

By default transitions are **continuous**: when the goal changes mid-animation, the new animation starts from the current interpolated value (no snap). This produces seamless direction reversals.

In **non-continuous** mode the transition always restarts from the original initial value when the goal changes.

```rust
let slide = window
    .use_keyed_transition("slide", cx, Duration::from_millis(300), |_, _| 0.0_f32)
    .continuous(false);  // restart from initial on each goal change
```

---

## Resetting

`reset` clears all progress and sets both start and end goals back to the initial value. The transition returns to a "completed" state at the initial value.

```rust
opacity.reset(cx);
```

---

## Transition\<T\> Method Summary

| Method | Returns | Description |
|--------|---------|-------------|
| `.with_easing(fn)` | `Transition<T>` | Set easing function (builder, call at creation) |
| `.continuous(bool)` | `Transition<T>` | Set continuous mode (builder, call at creation) |
| `.evaluate(window, cx)` | `Ref<'_, T>` | Current interpolated value; schedules next frame if in progress |
| `.evaluate_delta(cx)` | `f32` | Progress 0.0–1.0 after easing, no frame scheduling |
| `.read_goal(cx)` | `&T` | The current end goal, no interpolation |
| `.read_cache()` | `Ref<'_, Option<T>>` | Last evaluated value, or `None` |
| `.update(cx, \|val, cx\| ...)` | `bool` | Update goal; returns `true` if value changed |
| `.reset(cx)` | `()` | Clear progress, return to initial value |
| `.entity_id()` | `EntityId` | ID of the underlying `TransitionState` entity |

---

## The Lerp Trait

`T` must implement `Lerp + Clone + PartialEq + 'static` to be used in a `Transition<T>`.

```rust
pub trait Lerp {
    fn lerp(&self, to: &Self, delta: f32) -> Self;
}
```

`delta = 0.0` → returns `self`. `delta = 1.0` → returns `to`. Values in between are interpolated.

### Built-in Lerp implementations

**Numeric primitives:** `f32`, `f64`, `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `i8`, `i16`, `i32`, `i64`, `i128`, `isize`

**GPUI unit types:** `Pixels`, `Rems`, `DevicePixels`, `Percentage`, `Radians`

**GPUI geometry types:** `Point<T>`, `Size<T>`, `Edges<T>`, `Corners<T>`, `Bounds<T>` (where `T: Lerp`)

**GPUI color types:** `Rgba`, `Colors`

**Wrapper:** `BoolLerp<N>` (where `N: Lerp + Copy + PartialOrd + From<u8>`)

### Implementing Lerp for custom types

```rust
use gpui_transitions::Lerp;

#[derive(Clone, PartialEq)]
struct MyValue {
    x: f32,
    y: f32,
}

impl Lerp for MyValue {
    fn lerp(&self, to: &Self, delta: f32) -> Self {
        Self {
            x: self.x.lerp(&to.x, delta),
            y: self.y.lerp(&to.y, delta),
        }
    }
}
```

---

## BoolLerp\<N\>

`BoolLerp<N>` wraps a numeric value to animate boolean on/off states smoothly. Converts directly from `bool`. Useful for animating opacity, visibility, or expansion.

```rust
use gpui_transitions::BoolLerp;

// Construct
let visible: BoolLerp<f32> = true.into();       // value = 1.0
let hidden:  BoolLerp<f32> = false.into();      // value = 0.0
let truthy = BoolLerp::<f32>::truthy();         // value = 1.0
let falsey = BoolLerp::<f32>::falsey();         // value = 0.0

// Read
let alpha: f32 = visible.value();               // 1.0 (or intermediate during animation)

// Toggle (returns new BoolLerp with inverted value: 1 - value)
let toggled = visible.toggle();                 // value = 0.0

// Use in a transition
let visibility = window
    .use_keyed_transition("visible", cx, Duration::from_millis(200), |_, _| BoolLerp::<f32>::falsey())
    .with_easing(ease_in_out);

// Drive it from a bool
visibility.update(cx, |val, cx| {
    *val = self.is_visible.into();
    cx.notify();
});

// Consume the animated value
let alpha = visibility.evaluate(window, cx).value();
div().opacity(alpha).child("content")
```

---

## Common Patterns

### Hover fade

```rust
impl Render for MyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hover_opacity = window
            .use_keyed_transition("hover", cx, Duration::from_millis(120), |_, _| 0.0_f32)
            .with_easing(ease_in_out);

        let alpha = *hover_opacity.evaluate(window, cx);

        div()
            .id("my-element")
            .on_hover(cx.listener(move |_this, hovered: &bool, window, cx| {
                hover_opacity.update(cx, |val, cx| {
                    *val = if *hovered { 1.0 } else { 0.0 };
                    cx.notify();
                });
            }))
            .child(
                div().opacity(alpha).child("fades in on hover")
            )
    }
}
```

### Slide-in panel

```rust
let slide = window
    .use_keyed_transition("panel-slide", cx, Duration::from_millis(250), |_, _| -200.0_f32)
    .with_easing(ease_out_quint);

let offset_x = *slide.evaluate(window, cx);

// Trigger when panel becomes visible
if panel_just_opened {
    slide.update(cx, |val, cx| { *val = 0.0; cx.notify(); });
}

div()
    .left(px(offset_x))
    .child("panel content")
```

### Animated color

```rust
use gpui::Rgba;

let color_transition = window
    .use_keyed_transition(
        "status-color",
        cx,
        Duration::from_millis(400),
        |_, _| Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }, // red
    )
    .with_easing(ease_in_out);

let current_color = *color_transition.evaluate(window, cx);

// Animate to green when status changes
color_transition.update(cx, |color, cx| {
    *color = Rgba { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    cx.notify();
});

div().bg(current_color).child("status indicator")
```

### Progress bar

```rust
let progress = window
    .use_keyed_transition("upload-progress", cx, Duration::from_millis(300), |_, _| 0.0_f32)
    .with_easing(ease_in_out);

let animated_value = *progress.evaluate(window, cx);

// Update as data arrives
progress.update(cx, |val, cx| {
    *val = self.bytes_done as f32 / self.bytes_total as f32;
    cx.notify();
});

div()
    .w_full()
    .h(px(4.))
    .bg(border_color)
    .child(
        div()
            .w(relative(animated_value))
            .h_full()
            .bg(accent_color)
    )
```

### Multiple transitions on one view

Use distinct keys to keep each transition's state separate:

```rust
let opacity = window
    .use_keyed_transition("card-opacity", cx, Duration::from_millis(150), |_, _| 0.0_f32)
    .with_easing(ease_in_out);

let scale = window
    .use_keyed_transition("card-scale", cx, Duration::from_millis(200), |_, _| 0.95_f32)
    .with_easing(ease_out_quint);

let current_opacity = *opacity.evaluate(window, cx);
let current_scale   = *scale.evaluate(window, cx);
```

---

## Key Rules

1. **Always call `cx.notify()` inside `.update()`** — the transition does not trigger re-renders automatically.
2. **`.evaluate()` must be called in `render()` or `prepaint()`**, not in event handlers or async tasks — it accesses `Window` and may schedule animation frames.
3. **Use `use_keyed_transition` over `use_transition`** for any transition that must survive re-renders or that you need to identify by a runtime key.
4. **Chain `.with_easing()` and `.continuous()` at creation time** — these are builder methods that return a new `Transition<T>`. Calling them later on a cloned handle creates a new wrapper but does not modify the shared state.
5. **`Transition<T>` is `Clone`** — you can clone the handle to pass it into closures (e.g., inside `.on_hover()`). All clones share the same underlying `Entity<TransitionState<T>>`.
6. **`Transition<T>` is not `Send` or `Sync`** — it must stay on the GPUI foreground thread. Do not move it into `cx.background_spawn` closures.

---

## Custom Easing Functions

`.with_easing()` accepts any `impl Fn(f32) -> f32 + 'static`. You can define a module of reusable curves and re-export them. The important distinction is whether a function takes `t: f32` directly (pass as a bare function) or returns a closure (call it first):

```rust
// Bare functions — pass directly:
.with_easing(ease_in_out)
.with_easing(ease_out_cubic)

// Closure-returning functions — call them to get the Fn:
.with_easing(ease_out_quint())
.with_easing(pulsating_between(0.3, 1.0))
```

### Reference implementations

```rust
mod transitions {
    use std::f32::consts::PI;

    // Quadratic
    pub fn ease_in(t: f32)     -> f32 { t * t }
    pub fn ease_out(t: f32)    -> f32 { 1.0 - (1.0 - t).powi(2) }
    pub fn ease_in_out(t: f32) -> f32 {
        if t < 0.5 { 2.0 * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
    }

    // Cubic
    pub fn ease_in_cubic(t: f32)     -> f32 { t * t * t }
    pub fn ease_out_cubic(t: f32)    -> f32 { 1.0 - (1.0 - t).powi(3) }
    pub fn ease_in_out_cubic(t: f32) -> f32 {
        if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
    }

    // Quint — returns a closure, so call it: .with_easing(ease_out_quint())
    pub fn ease_out_quint() -> impl Fn(f32) -> f32 {
        |delta| 1.0 - (1.0 - delta).powi(5)
    }

    // Back (overshoots then settles)
    pub fn ease_in_back(t: f32) -> f32 {
        let (c1, c3) = (1.70158_f32, 2.70158_f32);
        (c3 * t * t * t - c1 * t * t).clamp(0.0, 1.0)
    }
    pub fn ease_out_back(t: f32) -> f32 {
        let (c1, c3) = (1.70158_f32, 2.70158_f32);
        (1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)).clamp(0.0, 1.0)
    }

    // Elastic
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

    // Bounce
    pub fn ease_out_bounce(t: f32) -> f32 {
        let (n1, d1) = (7.5625_f32, 2.75_f32);
        if      t < 1.0 / d1 { n1 * t * t }
        else if t < 2.0 / d1 { let t = t - 1.5   / d1; n1 * t * t + 0.75 }
        else if t < 2.5 / d1 { let t = t - 2.25  / d1; n1 * t * t + 0.9375 }
        else                  { let t = t - 2.625 / d1; n1 * t * t + 0.984375 }
    }
    pub fn ease_in_bounce(t: f32) -> f32 { 1.0 - ease_out_bounce(1.0 - t) }

    // Pulsating — loops back and forth between min and max
    pub fn pulsating_between(min: f32, max: f32) -> impl Fn(f32) -> f32 {
        move |t| min + ((t * 2.0 * PI).sin() * 0.5 + 0.5) * (max - min)
    }
}
pub use transitions::*;
```

---

## use_keyed_state — Non-Animated Sibling

`Window::use_keyed_state` is the non-animated counterpart to `use_keyed_transition`. Use it when you need per-item state that persists across renders but does not need interpolation (open/closed, hover booleans, scroll positions, etc.).

```rust
// In render() or prepaint():
let is_open = window.use_keyed_state(
    "section-open",          // key: any impl Into<ElementId>
    cx,
    |_window, _cx| false,    // initialiser — runs only once per key
);

// Read
let open: bool = *is_open.read(cx);

// Write (must cx.notify() to trigger re-render)
is_open.update(cx, |val, cx| {
    *val = !*val;
    cx.notify();
});
```

The return value is an `Entity<T>`, so you can also subscribe to it or clone the handle into closures. This avoids adding boolean flags to the parent view's struct for state that is purely local to one render site.

| | `use_keyed_state` | `use_keyed_transition` |
|---|---|---|
| Animated | No | Yes |
| Return type | `Entity<T>` | `Transition<T>` |
| Evaluate | `.read(cx)` | `.evaluate(window, cx)` |
| Requires `Lerp` | No | Yes |
| Schedules animation frames | No | Yes (when in progress) |