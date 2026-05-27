# cargo-ndk + Rust JNI Bridge — Quick Reference

Building a Rust `cdylib` that exposes JNI functions to an Android Kotlin/Compose app, using `cargo-ndk` to cross-compile for Android ABIs.

---

## Toolchain Setup (one-time, macOS)

```sh
cargo install cargo-ndk

rustup target add \
    aarch64-linux-android \
    armv7-linux-androideabi \
    x86_64-linux-android
```

Install the NDK through Android Studio → SDK Manager → SDK Tools → NDK (Side by side). Note the exact path — it is needed for every build.

---

## Workspace Setup

The bridge crate must be a **member of the Cargo workspace**. Without this, `workspace = true` dep references fail silently.

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "crates/android-bridge",   # ← add this
    "crates/simple-core",
    # ... other members
]
```

### `crates/android-bridge/Cargo.toml`

```toml
[package]
name = "android-bridge"
version = "0.1.0"
edition = "2024"

[lib]
name = "android_bridge"    # → libandroid_bridge.so
crate-type = ["cdylib"]

[dependencies]
simple-core = { path = "../simple-core" }
anyhow      = { workspace = true }
serde       = { workspace = true }
serde_json  = { workspace = true }
uuid        = { workspace = true }
jni         = "0.21"
```

- `cdylib` is mandatory — `rlib` alone does not produce an `.so`.
- Do NOT add `rlib` to `crate-type` here — the bridge is only a shared library, not a Rust dependency.
- `simple-core` (or your core crate) should have `crate-type = ["cdylib", "rlib"]` so it can be used as a Rust dependency by the bridge.

---

## JNI Naming Convention — Critical Rules

JNI function names follow a strict scheme. **Getting this wrong compiles but silently fails at runtime with `UnsatisfiedLinkError`.**

```
Java_{package}_{class}_{method}
```

### Underscore escaping — most common gotcha

Underscores in package/class names are **escaped as `_1`** in the JNI symbol name:

| Kotlin | JNI symbol segment |
|---|---|
| `com.example.myapp` | `com_example_myapp` |
| `com.example.my_app` | `com_example_my_1app` |
| `com.example.subroutine_simple` | `com_example_subroutine_1simple` |

Full example for package `com.example.subroutine_simple`, class `RustBridge`, method `createAction`:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_1simple_RustBridge_createAction(
    mut env: JNIEnv,
    _class: JClass,
    title: JString,
) -> jstring {
    // ...
}
```

> If you rename a Kotlin package or class after writing JNI functions, every function name must be updated. Android Studio refactoring does NOT update Rust code.

### Other escaping rules

| Character | Escape |
|---|---|
| `.` (package separator) | `_` |
| `_` in identifier | `_1` |
| `;` (type descriptor) | `_2` |
| `[` (array) | `_3` |

---

## Bridge Source Pattern

Split every function into an `_impl` that uses `anyhow::Result` and a thin JNI wrapper. This keeps the logic testable and the JNI boilerplate minimal.

```rust
use anyhow::Result;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jstring};

// ── impl functions (no JNI types — testable) ──────────────────────────────────

fn create_action_impl(title: &str) -> Result<String> {
    let action = simple_core::Action::new(title);
    Ok(serde_json::to_string(&action)?)
}

// ── JNI helpers ───────────────────────────────────────────────────────────────

fn get_string(env: &mut JNIEnv, arg: &JString) -> Option<String> {
    env.get_string(arg).ok().map(|s| s.into())
}

fn return_json_or_throw(env: &mut JNIEnv, result: Result<String>, fallback: &str) -> jstring {
    match result {
        Ok(json) => env.new_string(json).expect("alloc").into_raw(),
        Err(err) => {
            let _ = env.throw_new("java/lang/RuntimeException", err.to_string());
            env.new_string(fallback).expect("alloc").into_raw()
        }
    }
}

fn return_bool_or_throw(env: &mut JNIEnv, result: Result<()>) -> jboolean {
    match result {
        Ok(()) => 1,
        Err(err) => {
            let _ = env.throw_new("java/lang/RuntimeException", err.to_string());
            0
        }
    }
}

// ── JNI exports ───────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_subroutine_1simple_RustBridge_createAction(
    mut env: JNIEnv,
    _class: JClass,
    title: JString,
) -> jstring {
    let Some(title) = get_string(&mut env, &title) else {
        return env.new_string("{}").expect("alloc").into_raw();
    };
    return_json_or_throw(&mut env, create_action_impl(&title), "{}")
}
```

### JNI rules

- `_class: JClass` is always present for static methods but **always unused** — name it `_class`.
- Never `unwrap()` in JNI functions — a Rust panic across the FFI boundary is **undefined behaviour**.
- On error: call `env.throw_new(...)` to schedule a Java exception, then return a safe fallback value. The exception is thrown on the Java side after the native function returns.
- All data crosses the boundary as **JSON strings** — avoid fighting C type system with complex types.
- `env.get_string(&jstring)` extracts a `JString` to a Rust `String`.
- `env.new_string(s).expect("...").into_raw()` converts a Rust `String` to a `jstring`.

---

## Build Commands

Run from the workspace root. The output path should point into the Android project's `jniLibs` directory.

```sh
# Development (arm64 device only — much faster)
ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/27.3.13750724 \
  cargo ndk \
    -t arm64-v8a \
    --platform 26 \
    -o simple-android/app/src/main/jniLibs \
    build -p android-bridge

# Release (all ABIs for distribution)
ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/27.3.13750724 \
  cargo ndk \
    -t arm64-v8a \
    -t armeabi-v7a \
    -t x86_64 \
    --platform 26 \
    -o simple-android/app/src/main/jniLibs \
    build -p android-bridge --release
```

- `--platform N` must match or be ≤ `minSdk` in `build.gradle.kts`.
- `x86` (32-bit) is omitted — modern emulators use `x86_64`.
- The NDK version in the path (`27.3.13750724`) must be installed via Android Studio → SDK Tools → NDK.
- The compiled `.so` files can be committed to the repo so teammates don't need Rust installed.

Output structure:
```
simple-android/app/src/main/jniLibs/
  arm64-v8a/libandroid_bridge.so
  armeabi-v7a/libandroid_bridge.so
  x86_64/libandroid_bridge.so
```

---

## Kotlin Side — Stub → Native Progression Pattern

Build with a Kotlin stub first so the app works without a compiled `.so`, then activate the native version once the bridge is ready. This avoids blocking UI work on the Rust build.

### Phase 1 — Kotlin stub

```kotlin
// RustBridge.kt
package com.example.subroutine_simple

import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

object RustBridge {

    // ── Native (uncomment when .so is built) ────────────────────────
    // init { System.loadLibrary("android_bridge") }
    // external fun createAction(title: String): String

    // ── Kotlin stub (remove when switching to native) ────────────────

    fun createAction(title: String): String {
        val id = java.util.UUID.randomUUID().toString()
        return buildJsonObject {
            put("id", id)
            put("lineage_id", id)
            put("origin_routine_id", JsonNull)
            put("title", title)
            put("content", JsonNull)
            put("duration", JsonNull)
            put("recurrence", JsonNull)
            put("saved", false)
            put("state", buildJsonObject { put("Backlogged", JsonNull) })
        }.toString()
    }
}
```

### Phase 2 — activate native

After running `cargo ndk ... build -p android-bridge --release`:

```kotlin
object RustBridge {
    init { System.loadLibrary("android_bridge") }

    external fun createAction(title: String): String
}
```

The stub body is replaced by the `external fun` declaration. The `init` block loads the `.so`. Method signature must exactly match the Rust JNI export.

---

## Repository Pattern with Bridge

The repository is the only layer that touches `RustBridge`. Call it on `Dispatchers.IO` since JNI calls block.

```kotlin
class SubroutineRepository {
    private val api = RetrofitClient.api
    private val json = Json { ignoreUnknownKeys = true }

    suspend fun createAction(title: String): Action = withContext(Dispatchers.IO) {
        // 1. Bridge constructs the domain object → JSON
        val actionJson = RustBridge.createAction(title)
        // 2. Kotlin decodes it into a data class
        val action = json.decodeFromString<Action>(actionJson)
        // 3. Persist via HTTP
        api.upsertAction(action.id, action)
    }
}
```

This pattern means:
- **Business logic** (UUID generation, state initialisation, validation) lives in Rust.
- **Network I/O** stays in Kotlin — no HTTP client in Rust.
- The stub and native implementation are **interchangeable** — the repository doesn't care.

---

## HTTP + Bridge Architecture

This project uses HTTP for persistence (not local SQLite), so the bridge is responsible only for domain logic, not storage:

```
Compose UI
    ↓
ViewModel (StateFlow)
    ↓
Repository (Dispatchers.IO)
    ├── RustBridge  → construct/validate domain objects
    └── RetrofitApi → persist via HTTP server
```

Contrast with a local-SQLite setup where the bridge also owns persistence:

```
Repository
    └── RustBridge → db_path → SQLite via Rust
```

Both patterns use the same JNI bridge mechanics — the difference is whether the `.so` calls into a database or just does pure logic.

---

## Serde and Type Mapping

### Rust enum → Kotlin

Rust's default serde external-tagged enum representation (`{"Variant": value}`) does not map cleanly to kotlinx.serialization sealed classes. Use `JsonElement` + extension properties instead:

```kotlin
// Kotlin — state: JsonElement
val Action.isQueued: Boolean
    get() = state is JsonObject && (state as JsonObject).containsKey("Queued")

val Action.scheduledTimeIso: String?
    get() {
        val obj = state as? JsonObject ?: return null
        val queued = obj["Queued"] as? JsonObject ?: return null
        return (queued["time"] as? JsonPrimitive)?.content
    }
```

Always import extension properties explicitly — they are not brought in with the data class import:
```kotlin
import com.example.subroutine_simple.data.models.isQueued     // explicit — required
import com.example.subroutine_simple.data.models.Action       // does NOT pull in isQueued
```

### `chrono::Duration` serialization

With `chrono = { version = "0.4", features = ["serde"] }` in the workspace:
- `Option<Duration>` where the value is `None` serializes cleanly as `null`.
- `Option<Duration>` where the value is `Some(d)` serializes as `{"secs": i64, "nanos": u32}`.
- For new objects created in Rust (like `Action::new()`), `duration` is always `None` → `null` in JSON.
- On the Kotlin side, represent `duration` as `JsonElement?` to remain format-agnostic.

### `uuid` serialization

With `uuid = { version = "1", features = ["serde"] }`, UUIDs serialize as lowercase hyphenated strings: `"550e8400-e29b-41d4-a716-446655440000"`.
