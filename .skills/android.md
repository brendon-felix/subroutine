# Android + Rust JNI — Quick Reference

A general-purpose reference for calling Rust logic from an Android (Kotlin/Compose) app via JNI. The pattern: a thin Rust `cdylib` crate wraps your core Rust libraries and exposes C-ABI functions; Kotlin loads the `.so` and calls them as `external fun`.

---

## Architecture

```
Android (Kotlin/Compose)
    ↓  JNI — JSON strings over the boundary
android-bridge  (Rust cdylib)
    ↓
your-core  ←  your-db
```

**Rules:**
- The Kotlin layer contains **no business logic** — only UI, lifecycle, and serialization.
- The bridge crate is a thin shim: it extracts JNI arguments, calls core functions, serializes the result, and returns.
- All data crossing the boundary is **JSON strings**. This keeps JNI boilerplate minimal and avoids fighting the C type system.
- The database path is **always passed from Kotlin** — never resolved inside Rust. Android's sandbox means `dirs::data_dir()` returns a useless path.

---

## Project Structure

```
android/                        # Android Studio / Gradle project
  app/src/main/
    java/com/example/myapp/
      RustBridge.kt             # System.loadLibrary + external fun declarations
      MyRepository.kt           # data classes + wraps RustBridge, runs on Dispatchers.IO
      MyViewModel.kt            # AndroidViewModel, StateFlow<UiState>
      MainActivity.kt           # Compose entry point
    jniLibs/                    # compiled .so files — placed here by cargo-ndk
      arm64-v8a/libmybridge.so
      armeabi-v7a/libmybridge.so
      x86_64/libmybridge.so

crates/android-bridge/          # Rust cdylib crate
  Cargo.toml                    # crate-type = ["cdylib"]
  src/lib.rs                    # #[no_mangle] extern "C" JNI functions
```

---

## Rust Bridge Crate

### `Cargo.toml`

```toml
[package]
name = "android-bridge"
version = "0.1.0"
edition = "2024"

[lib]
name = "android_bridge"         # becomes libandroid_bridge.so
crate-type = ["cdylib"]

[dependencies]
jni = "0.21"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
your-core = { path = "../your-core" }
```

### JNI naming convention

Every exported function must follow this exact naming scheme — underscores replace dots:

```
Java_{package}_{class}_{method}
```

For package `com.example.myapp`, class `RustBridge`, method `fetchItems`:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_myapp_RustBridge_fetchItems(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
) -> jstring {
    // ...
}
```

### Helper pattern: impl functions + thin JNI wrappers

Keep JNI glue separate from logic. Inner `_impl` functions use `anyhow::Result` and are easy to test:

```rust
use anyhow::Result;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jstring};

// ── impl functions (no JNI types) ────────────────────────────────────────────

fn fetch_items_impl(db_path: &str) -> Result<String> {
    let items = your_core::fetch_items(db_path)?;
    Ok(serde_json::to_string(&items)?)
}

fn insert_item_impl(db_path: &str, title: &str) -> Result<()> {
    your_core::insert_item(db_path, title)?;
    Ok(())
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
pub extern "C" fn Java_com_example_myapp_RustBridge_fetchItems(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
) -> jstring {
    let Some(db_path) = get_string(&mut env, &db_path) else {
        return env.new_string("[]").expect("alloc").into_raw();
    };
    return_json_or_throw(&mut env, fetch_items_impl(&db_path), "[]")
}

#[unsafe(no_mangle)]
pub extern "C" fn Java_com_example_myapp_RustBridge_insertItem(
    mut env: JNIEnv,
    _class: JClass,
    db_path: JString,
    title: JString,
) -> jboolean {
    let (Some(db_path), Some(title)) = (
        get_string(&mut env, &db_path),
        get_string(&mut env, &title),
    ) else {
        return 0;
    };
    return_bool_or_throw(&mut env, insert_item_impl(&db_path, &title))
}
```

### JNI rules

- `env.get_string(&jstring)` to extract Rust `String` from a JNI argument.
- Return `jstring` via `env.new_string(value).expect("...").into_raw()`.
- Return `jboolean` as `1u8` (success) / `0u8` (failure).
- On error: `env.throw_new("java/lang/RuntimeException", msg)` **then** return a safe fallback — never panic across the JNI boundary.
- `_class: JClass` is always present for static methods but unused.
- Never use `unwrap()` in JNI-exposed functions — a panic across the FFI boundary is undefined behaviour.

---

## Building `.so` Files (cargo-ndk)

```sh
# Install once
cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

# Build (run from workspace root after any Rust change)
ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/27.3.13750724 \
  cargo ndk \
    -t arm64-v8a \
    -t armeabi-v7a \
    -t x86_64 \
    --platform 24 \
    -o android/app/src/main/jniLibs \
    build -p android-bridge --release
```

- `--platform 24` must match `minSdk` in `build.gradle.kts`.
- `x86` (32-bit) is omitted — modern emulators use `x86_64`.
- The NDK path must point to a version installed via Android Studio.
- The output `.so` files can be committed to the repo so CI / teammates don't need Rust installed.

---

## Kotlin Side

### `RustBridge.kt` — loader and declarations only

```kotlin
object RustBridge {
    init {
        System.loadLibrary("android_bridge")
    }

    external fun fetchItems(dbPath: String): String          // returns JSON array
    external fun insertItem(dbPath: String, title: String): Boolean
    external fun deleteItem(dbPath: String, id: String): Boolean
}
```

### Database path — always from Kotlin

```kotlin
// In a Repository or ViewModel that holds a Context:
val dbPath: String
    get() = context.getDatabasePath("myapp.db").absolutePath
// Resolves to: /data/data/com.example.myapp/databases/myapp.db
// Android creates the directory automatically before first use.
```

### Data classes + deserialization

Mirror the JSON shape that `serde_json` produces from your Rust structs. Use `@SerialName` for `snake_case` → `camelCase` mapping.

```kotlin
@Serializable
data class Item(
    val id: String,
    val title: String,
    val content: String? = null,
    @SerialName("created_at") val createdAt: String,
)

// In the repository — always use ignoreUnknownKeys so new Rust fields don't crash old app versions
private val json = Json { ignoreUnknownKeys = true }

fun fetchItems(): List<Item> {
    val raw = RustBridge.fetchItems(dbPath)
    return json.decodeFromString(raw)
}
```

### Repository pattern

```kotlin
class ItemRepository(private val appContext: Context) {

    private val json = Json { ignoreUnknownKeys = true }

    private val dbPath: String
        get() = appContext.getDatabasePath("myapp.db").absolutePath

    // Run all JNI calls on Dispatchers.IO — they block the thread
    suspend fun fetchItems(): List<Item> = withContext(Dispatchers.IO) {
        json.decodeFromString(RustBridge.fetchItems(dbPath))
    }

    suspend fun insertItem(title: String): Boolean = withContext(Dispatchers.IO) {
        RustBridge.insertItem(dbPath, title)
    }

    suspend fun deleteItem(id: String): Boolean = withContext(Dispatchers.IO) {
        RustBridge.deleteItem(dbPath, id)
    }
}
```

### ViewModel + UI state

```kotlin
sealed interface ItemsUiState {
    data object Loading : ItemsUiState
    data class Success(val items: List<Item>) : ItemsUiState
    data class Error(val message: String) : ItemsUiState
}

class ItemsViewModel(application: Application) : AndroidViewModel(application) {

    private val repository = ItemRepository(application)

    private val _uiState = MutableStateFlow<ItemsUiState>(ItemsUiState.Loading)
    val uiState: StateFlow<ItemsUiState> = _uiState

    init { loadItems() }

    fun loadItems() {
        viewModelScope.launch {
            _uiState.value = ItemsUiState.Loading
            runCatching { repository.fetchItems() }
                .onSuccess { _uiState.value = ItemsUiState.Success(it) }
                .onFailure { _uiState.value = ItemsUiState.Error(it.message ?: "Unknown error") }
        }
    }

    fun addItem(title: String) {
        viewModelScope.launch {
            runCatching { repository.insertItem(title) }
                .onSuccess { loadItems() }
                .onFailure { _uiState.value = ItemsUiState.Error(it.message ?: "Insert failed") }
        }
    }
}
```

### Collecting state in Compose

```kotlin
@Composable
fun ItemsScreen(viewModel: ItemsViewModel = viewModel()) {
    val uiState by viewModel.uiState.collectAsStateWithLifecycle()

    when (val state = uiState) {
        is ItemsUiState.Loading -> LoadingIndicator()
        is ItemsUiState.Error   -> ErrorView(state.message, onRetry = viewModel::loadItems)
        is ItemsUiState.Success -> ItemList(state.items, onDelete = viewModel::deleteItem)
    }
}
```

---

## Gradle Configuration (AGP 8+ / KGP 2.x)

### `build.gradle.kts`

```kotlin
plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "com.example.myapp"
    compileSdk { version = release(36) }

    defaultConfig {
        minSdk = 24
        targetSdk = 36
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }

    // Use kotlin { compilerOptions { } } — NOT kotlinOptions { }
    // kotlinOptions is unavailable when only kotlin.compose is applied (no kotlin.android).
    kotlin {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_11)
            // Opt-ins here instead of per-file annotations:
            freeCompilerArgs.addAll(
                "-opt-in=androidx.compose.material3.ExperimentalMaterial3ExpressiveApi",
                "-opt-in=androidx.compose.material3.ExperimentalMaterial3Api",
            )
        }
    }

    buildFeatures { compose = true }
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))   // use compose-bom-alpha for Expressive
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.kotlinx.serialization.json)
    // ... other deps
}
```

### Common gotchas

- Use `kotlin { compilerOptions { } }` — `kotlinOptions { }` does not exist with the compose plugin alone.
- Use `jvmTarget.set(JvmTarget.JVM_11)` (typed enum), not `jvmTarget = "11"` (string).
- Use `freeCompilerArgs.addAll(...)`, not `+= listOf(...)`.
- Material 3 Expressive requires `compose-bom-alpha` — the stable BOM does not include it.

---

## Serde Gotchas for Cross-Boundary Types

```toml
# workspace Cargo.toml — enable serde features
uuid     = { version = "1", features = ["v4", "serde"] }
bitflags = { version = "2", features = ["serde"] }
chrono   = { version = "0.4", features = ["serde"] }
```

- `chrono::Duration` does **not** implement `Serialize`. Use a helper module:

```rust
// In your core crate — serialize Duration as i64 seconds
mod duration_serde {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        d.num_seconds().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::seconds(i64::deserialize(d)?))
    }

    pub mod option {
        use super::*;
        pub fn serialize<S: Serializer>(d: &Option<Duration>, s: S) -> Result<S::Ok, S::Error> {
            d.map(|d| d.num_seconds()).serialize(s)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Duration>, D::Error> {
            Ok(Option::<i64>::deserialize(d)?.map(Duration::seconds))
        }
    }
}

// Usage:
#[derive(Serialize, Deserialize)]
struct MyStruct {
    #[serde(with = "duration_serde")]
    pub duration: chrono::Duration,
    #[serde(with = "duration_serde::option")]
    pub optional_duration: Option<chrono::Duration>,
}
```

- Kotlin receives `Duration` as `Long` (seconds). `BitFlags` values arrive as `Int`.
- Use `Json { ignoreUnknownKeys = true }` on the Kotlin side so adding new Rust fields doesn't crash deployed app versions.