# Android Design

This document captures the design decisions and conventions for the Subroutine Android app.

---

## Purpose

The Android app is a native Kotlin/Compose client for Subroutine. It shares all business logic and persistence with the desktop and CLI clients by calling into the same `app-core` and `database` Rust crates via a thin JNI bridge. The Android layer is responsible only for UI and lifecycle — it contains no business logic of its own.

---

## Architecture

### Dependency Direction

```
android (Kotlin/Compose)
    ↓ JNI (JSON over the boundary)
android-bridge (Rust cdylib)
    ↓
app-core  ←  database
```

The Kotlin layer never contains business logic. If something can be computed in Rust, it is computed in Rust. The Android code only serializes calls and renders results.

### Crate: `crates/android-bridge`

A Rust `cdylib` crate that exposes JNI functions. It depends on `app-core` and `database` directly. The compiled output is a `.so` shared library placed into `android/app/src/main/jniLibs/` by `cargo-ndk`.

All JNI functions follow the naming convention:
```
Java_{package}_{class}_{method}
```

For example, `Java_com_example_subroutine_RustBridge_fetchSavedActions`.

### Data Boundary: JSON over JNI

All data crossing the JNI boundary is serialized as JSON strings. This keeps the JNI boilerplate minimal — each function either returns a JSON string or accepts one. The Kotlin side deserializes with `kotlinx.serialization`. The Rust side serializes with `serde_json`.

Rust `app-core` types that cross the boundary derive `serde::Serialize` and `serde::Deserialize`. `chrono::Duration` fields use a custom `duration_serde` module in `app-core/src/lib.rs` that serializes as seconds (`i64`), since `chrono::Duration` does not implement `Serialize` natively.

### Database Path

The database path is **always passed from Kotlin to Rust**, never resolved inside the bridge. This is because `database::connect_and_migrate` (which uses `dirs::data_dir`) resolves a macOS/Linux path that is meaningless on Android. The bridge uses `database::connect_and_migrate_at(path)` instead.

The path is resolved in Kotlin using:
```kotlin
context.getDatabasePath("subroutine.db").absolutePath
```

This resolves to `/data/data/com.example.subroutine/databases/subroutine.db` — Android's private, app-sandboxed database directory. The directory is created automatically by Android before the path is used. The WAL files (`-shm`, `-wal`) are created alongside it by SQLite.

### `RustBridge`

A Kotlin `object` that calls `System.loadLibrary("android_bridge")` in its `init` block and declares all `external fun` JNI signatures. It is the only place in the Kotlin codebase that knows the library exists.

### `ActionsRepository`

A plain class (not a `ViewModel`) that holds the `Context` reference, resolves the database path, calls `RustBridge`, and deserializes the JSON result. ViewModels own repository instances.

### ViewModels

ViewModels extend `AndroidViewModel` (not `ViewModel`) so they can hold an `Application` context for passing to the repository without leaking an `Activity`. All database I/O is dispatched to `Dispatchers.IO` via `viewModelScope.launch`. UI state is exposed as `StateFlow`.

---

## UI: Jetpack Compose + Material 3 Expressive

The app uses Jetpack Compose with the **Material 3 Expressive** component set (BOM `compose-bom-alpha`). The expressive API is opt-in and is enabled globally in `build.gradle.kts`:

```kotlin
kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_11)
        freeCompilerArgs.addAll(
            "-Xopt-in=androidx.compose.material3.ExperimentalMaterial3ExpressiveApi",
            "-Xopt-in=androidx.compose.material3.ExperimentalMaterial3Api"
        )
    }
}
```

This means no per-file `@OptIn` annotations are needed.

### Expressive components in use

| Component | Where used | Why |
|---|---|---|
| `HorizontalFloatingToolbar` | `ActionsScreen` | Replaces `TopAppBar` — hides on scroll down, returns on scroll up. Less static chrome. |
| `FloatingActionButtonMenu` + `ToggleFloatingActionButton` | `ActionsScreen` | Expandable FAB that morphs Add↔Close. Supports multiple actions (new action, new routine, etc.) without redesigning the screen. |
| `LoadingIndicator` | `ActionsScreen` | Organic animated loading dots instead of a static spinner. |
| `FilledIconButton` with `IconButtonDefaults.shapes()` | `SavedActionCard` | Morphing squircle↔circle shape on press/hover. |
| `ElevatedButton` + `TextButton` with `ButtonDefaults.shapes()` | `AddActionDialog` | Morphing shape on interactive buttons in dialogs. |

### `HorizontalFloatingToolbar` scroll behaviour

The toolbar hides when the user scrolls down and reappears when they scroll up, using `FloatingToolbarDefaults.exitAlwaysScrollBehavior(exitDirection = Bottom)`. The `Scaffold` modifier must include `.nestedScroll(exitScrollBehavior)` to wire the scroll events through to the toolbar animation.

### `FloatingActionButtonMenu` visibility

The FAB uses `.animateFloatingActionButton(visible = fabVisible || fabMenuExpanded, alignment = Alignment.BottomEnd)`. `fabVisible` is `true` when `listState.firstVisibleItemIndex == 0`, i.e. the list is at the top. This means the FAB hides on scroll (matching the toolbar) but stays visible while the menu is open regardless of scroll position.

`BackHandler(fabMenuExpanded)` closes the menu on back press — required because the system back gesture would otherwise navigate away from the screen while the menu is open.

The FAB button uses `checkedProgress` (a `Float` from 0→1 driven by the toggle animation) to swap the icon at the midpoint rather than snapping instantly:
```kotlin
val imageVector by remember {
    derivedStateOf {
        if (checkedProgress > 0.5f) Icons.Filled.Close else Icons.Filled.Add
    }
}
Icon(
    painter = rememberVectorPainter(imageVector),
    modifier = Modifier.animateIcon({ checkedProgress }),
)
```

---

## Build

### Cross-compilation

The bridge is cross-compiled for Android using `cargo-ndk`. The command is:

```sh
ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/27.3.13750724 \
cargo ndk \
  -t arm64-v8a \
  -t armeabi-v7a \
  -t x86_64 \
  --platform 24 \
  -o android/app/src/main/jniLibs \
  build -p android-bridge --release
```

This must be re-run every time `android-bridge`, `app-core`, or `database` Rust code changes. The output `.so` files are committed to the repo in `android/app/src/main/jniLibs/` so that Android Studio builds work without requiring a local Rust toolchain.

`x86` (32-bit) is intentionally omitted — no current physical device requires it and all modern emulators default to `x86_64`.

### Gradle

The Android project is at `android/` (not inside `crates/`), because it is primarily a Kotlin/Gradle project. The Rust bridge crate that it consumes lives at `crates/android-bridge/` in the Cargo workspace.

The BOM used is `compose-bom-alpha` — the stable BOM does not include expressive components. This is intentional and expected to remain until expressive graduates to stable.

The `kotlinOptions` DSL block does not exist when only `kotlin.compose` is applied as a plugin (without `kotlin.android`). Use `kotlin { compilerOptions { } }` instead, which is the AGP 8+ / KGP 2.x API and works with the compose plugin alone.

---

## UI State pattern

Each screen's state is modelled as a sealed interface:

```kotlin
sealed interface ActionsUiState {
    data object Loading : ActionsUiState
    data class Success(val actions: List<SavedAction>) : ActionsUiState
    data class Error(val message: String) : ActionsUiState
}
```

The ViewModel exposes a `StateFlow<UiState>` and the composable uses `when (val state = uiState)` to switch between states. Errors from the Rust layer propagate as JVM exceptions (thrown via `env.throw_new`) and are caught with `runCatching` in the ViewModel.

---

## Data classes

Kotlin data classes mirror the JSON shape produced by `serde_json` serializing the Rust `app-core` types. Field names use `@SerialName` to map Rust's `snake_case` to Kotlin's `camelCase`:

```kotlin
@Serializable
data class SavedAction(
    val id: String,
    val title: String,
    val content: String? = null,
    @SerialName("target_time") val targetTime: String? = null,
    val context: ActionContext,
    val constraints: SavedConstraints,
    val recurrence: RecurrenceRule? = null,
)
```

`Json { ignoreUnknownKeys = true }` is used in the repository so that new fields added to Rust structs do not crash older app versions.

`chrono::Duration` serializes as an `i64` (seconds). The corresponding Kotlin fields are `Long?`.

`TimesOfDay` (a bitflags `u8`) serializes as an `Int?`. If the Kotlin side ever needs to interpret it, it should treat it as a bitmask.

---

## Module structure

```
android/
├── app/
│   └── src/main/java/com/example/subroutine/
│       ├── MainActivity.kt         — entry point, sets Compose content
│       ├── RustBridge.kt           — System.loadLibrary + external fun declarations
│       ├── ActionsRepository.kt    — data classes + repository wrapping RustBridge
│       ├── ActionsViewModel.kt     — StateFlow, coroutines, I/O dispatch
│       └── ui/theme/               — generated Material theme
```

As the app grows, screens move to a `screens/` package and the repository splits by domain (`ActionsRepository`, `PipelineRepository`, etc.).