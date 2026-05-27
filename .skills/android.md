# Android + Rust JNI — Quick Reference

Calling Rust logic from an Android (Kotlin/Compose) app via JNI. Pattern: a thin Rust `cdylib` crate wraps core Rust libraries and exposes C-ABI functions; Kotlin loads the `.so` and calls them as `external fun`.

See also: **`.skills/cargo-ndk-bridge.md`** for full cargo-ndk build commands and the Kotlin stub → native progression pattern.

---

## Architecture

### HTTP server (this project)

```
Compose UI
    ↓
ViewModel (StateFlow<UiState>)
    ↓
Repository (Dispatchers.IO)
    ├── RustBridge  → domain object construction / business logic
    └── RetrofitApi → HTTP server for persistence
```

### Local SQLite (alternative)

```
Repository (Dispatchers.IO)
    └── RustBridge → db_path → SQLite via Rust
```

**Rules:**
- Kotlin layer contains **no business logic** — UI, lifecycle, serialization only.
- The bridge is a thin shim: extract JNI args → call core → serialize → return.
- All data crossing the JNI boundary is **JSON strings**.
- In local-SQLite setups: the database path is **always passed from Kotlin** — Android's sandbox makes `dirs::data_dir()` useless inside Rust.

---

## Project Structure

```
simple-android/app/src/main/
  java/com/example/subroutine_simple/
    RustBridge.kt                    # System.loadLibrary + external fun
    data/
      models/Action.kt               # @Serializable data classes + extension properties
      network/SubroutineApi.kt       # Retrofit interface
      network/RetrofitClient.kt      # OkHttp + kotlinx.serialization converter
      repository/SubroutineRepository.kt
    ui/
      MainViewModel.kt               # AndroidViewModel, StateFlow<UiState>
      screens/QueueScreen.kt
      components/CreateActionSheet.kt
  jniLibs/                           # compiled .so — placed by cargo-ndk
    arm64-v8a/libandroid_bridge.so

crates/android-bridge/               # Rust cdylib — workspace member
  Cargo.toml                         # crate-type = ["cdylib"]
  src/android_bridge.rs              # impl functions + JNI wrappers
```

---

## HTTP Store with Retrofit

### Dependencies (`libs.versions.toml`)

```toml
[versions]
retrofit                  = "2.11.0"
retrofitKotlinxConverter  = "1.0.0"
okhttp                    = "4.12.0"
kotlinxSerializationJson  = "1.7.3"

[libraries]
retrofit                           = { group = "com.squareup.retrofit2",   name = "retrofit",                              version.ref = "retrofit" }
retrofit-kotlinx-serialization-converter = { group = "com.jakewharton.retrofit", name = "retrofit2-kotlinx-serialization-converter", version.ref = "retrofitKotlinxConverter" }
okhttp                             = { group = "com.squareup.okhttp3",     name = "okhttp",                                version.ref = "okhttp" }
okhttp-logging-interceptor         = { group = "com.squareup.okhttp3",     name = "logging-interceptor",                   version.ref = "okhttp" }
kotlinx-serialization-json         = { group = "org.jetbrains.kotlinx",    name = "kotlinx-serialization-json",            version.ref = "kotlinxSerializationJson" }

[plugins]
kotlin-serialization = { id = "org.jetbrains.kotlin.plugin.serialization", version.ref = "kotlin" }
```

### `RetrofitClient.kt`

```kotlin
import com.jakewharton.retrofit2.converter.kotlinx.serialization.asConverterFactory
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.logging.HttpLoggingInterceptor
import retrofit2.Retrofit

object RetrofitClient {
    private const val BASE_URL = "http://100.112.215.8:3000/"

    private val json = Json { ignoreUnknownKeys = true }

    private val okhttp = OkHttpClient.Builder()
        .addInterceptor(HttpLoggingInterceptor().apply { level = HttpLoggingInterceptor.Level.BASIC })
        .build()

    val api: SubroutineApi = Retrofit.Builder()
        .baseUrl(BASE_URL)
        .client(okhttp)
        .addConverterFactory(json.asConverterFactory("application/json".toMediaType()))
        .build()
        .create(SubroutineApi::class.java)
}
```

### `SubroutineApi.kt`

```kotlin
interface SubroutineApi {
    @GET("api/data")
    suspend fun getAllData(): AllData

    @PUT("api/actions/{id}")
    suspend fun upsertAction(@Path("id") id: String, @Body action: Action): Action

    @POST("api/actions/{id}/complete")
    suspend fun completeAction(@Path("id") id: String): CompleteResult
}
```

### `SubroutineRepository.kt`

```kotlin
class SubroutineRepository {
    private val api = RetrofitClient.api
    private val json = Json { ignoreUnknownKeys = true }

    suspend fun fetchAll(): Pair<List<Action>, List<Event>> = withContext(Dispatchers.IO) {
        val data = api.getAllData()
        Pair(data.actions, data.events)
    }

    suspend fun createAction(title: String): Action = withContext(Dispatchers.IO) {
        val actionJson = RustBridge.createAction(title)
        val action = json.decodeFromString<Action>(actionJson)
        api.upsertAction(action.id, action)
    }

    suspend fun completeAction(id: String): Action = withContext(Dispatchers.IO) {
        api.completeAction(id).completed
    }
}
```

### Allow cleartext HTTP to Tailscale / dev server

Add to `AndroidManifest.xml`:
```xml
<application
    android:networkSecurityConfig="@xml/network_security_config"
    ...>
```

`res/xml/network_security_config.xml`:
```xml
<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <domain-config cleartextTrafficPermitted="true">
        <domain includeSubdomains="false">100.112.215.8</domain>
    </domain-config>
</network-security-config>
```

---

## Data Classes and the Rust Enum Problem

Rust's serde default (external-tagged enums) produces JSON like `{"Queued": {...}}`, `{"Backlogged": null}`, or `"Skipped"`. kotlinx.serialization sealed classes cannot parse this format without a custom serializer.

**Solution: use `JsonElement` for enum fields and add extension properties.**

```kotlin
@Serializable
data class Action(
    val id: String,
    @SerialName("lineage_id") val lineageId: String,
    @SerialName("origin_routine_id") val originRoutineId: String? = null,
    val title: String,
    val content: String? = null,
    val duration: JsonElement? = null,   // Option<Duration> → null or {"secs":N,"nanos":N}
    val recurrence: JsonElement? = null,
    val saved: Boolean = false,
    val state: JsonElement,              // ActionState enum
)

val Action.isQueued: Boolean
    get() = state is JsonObject && (state as JsonObject).containsKey("Queued")

val Action.isBacklogged: Boolean
    get() = state is JsonObject && (state as JsonObject).containsKey("Backlogged")

val Action.scheduledTimeIso: String?
    get() {
        val obj = state as? JsonObject ?: return null
        val queued = obj["Queued"] as? JsonObject ?: return null
        return (queued["time"] as? JsonPrimitive)?.content
    }
```

**Extension property import gotcha** — extension properties are NOT imported with the data class. They always need their own explicit import:

```kotlin
// In MainViewModel.kt:
import com.example.subroutine_simple.data.models.Action    // does NOT pull in isQueued
import com.example.subroutine_simple.data.models.isQueued  // explicit import required
import com.example.subroutine_simple.data.models.isBacklogged
```

Always use `Json { ignoreUnknownKeys = true }` so new Rust fields don't crash older app versions.

---

## ViewModel + UI State

```kotlin
sealed interface ActionsUiState {
    data object Loading : ActionsUiState
    data class Success(
        val queueItems: List<QueueItem>,
        val backlogged: List<Action>,
    ) : ActionsUiState
    data class Error(val message: String) : ActionsUiState
}

class MainViewModel : ViewModel() {
    private val repository = SubroutineRepository()

    private val _uiState = MutableStateFlow<ActionsUiState>(ActionsUiState.Loading)
    val uiState: StateFlow<ActionsUiState> = _uiState.asStateFlow()

    private val _showCreateSheet = MutableStateFlow(false)
    val showCreateSheet: StateFlow<Boolean> = _showCreateSheet.asStateFlow()

    init { loadActions() }

    fun loadActions() {
        viewModelScope.launch {
            _uiState.value = ActionsUiState.Loading
            runCatching { repository.fetchAll() }
                .onSuccess { (actions, events) ->
                    _uiState.value = ActionsUiState.Success(
                        queueItems = buildQueueItems(actions, events),
                        backlogged = actions.filter { it.isBacklogged },
                    )
                }
                .onFailure { e ->
                    _uiState.value = ActionsUiState.Error(e.message ?: "Unknown error")
                }
        }
    }

    fun createAction(title: String) {
        if (title.isBlank()) return
        viewModelScope.launch {
            runCatching { repository.createAction(title.trim()) }
                .onSuccess { _showCreateSheet.value = false; loadActions() }
                .onFailure { e -> _uiState.value = ActionsUiState.Error(e.message ?: "Failed") }
        }
    }

    fun openCreateSheet() { _showCreateSheet.value = true }
    fun closeCreateSheet() { _showCreateSheet.value = false }
}
```

### Collecting state in Compose — use `collectAsStateWithLifecycle`

```kotlin
// Preferred — lifecycle-aware, stops collecting when UI is not visible
val uiState by viewModel.uiState.collectAsStateWithLifecycle()

// Also works but collects even when backgrounded
val uiState by viewModel.uiState.collectAsState()
```

Requires `implementation(libs.androidx.lifecycle.runtime.compose)` in `build.gradle.kts`.

---

## RustBridge.kt — Stub → Native Pattern

Keep a Kotlin stub while the Rust bridge is in development. The repository calls `RustBridge.createAction(title)` the same way regardless of which implementation is active.

```kotlin
object RustBridge {

    // ── Phase 2: uncomment when .so is built ──────────────────────────
    // init { System.loadLibrary("android_bridge") }
    // external fun createAction(title: String): String

    // ── Phase 1: Kotlin stub ───────────────────────────────────────────

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

---

## Gradle Configuration (AGP 8+ / KGP 2.x)

```kotlin
plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)   // required for kotlinx.serialization
}

android {
    namespace = "com.example.subroutine_simple"
    compileSdk { version = release(36) }

    defaultConfig {
        minSdk = 26
        targetSdk = 36
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }

    kotlin {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_11)
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
    implementation(libs.androidx.compose.material.icons.extended)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.retrofit)
    implementation(libs.retrofit.kotlinx.serialization.converter)
    implementation(libs.okhttp)
    implementation(libs.okhttp.logging.interceptor)
}
```

### Common gotchas

- Use `kotlin { compilerOptions { } }` — `kotlinOptions { }` does not exist with the compose plugin alone.
- Use `jvmTarget.set(JvmTarget.JVM_11)` (typed enum), not `jvmTarget = "11"` (string).
- Use `freeCompilerArgs.addAll(...)`, not `+= listOf(...)`.
- Material 3 Expressive requires `compose-bom-alpha` — the stable BOM does not include it.
- Add `INTERNET` permission to `AndroidManifest.xml` for HTTP access.

---

## Serde Gotchas for Cross-Boundary Types

```toml
# workspace Cargo.toml
uuid     = { version = "1", features = ["v4", "v7", "serde"] }
chrono   = { version = "0.4", features = ["serde"] }
bitflags = { version = "2", features = ["serde"] }
```

- **`chrono::Duration` with serde feature**: `Option<Duration>` where `value = None` serializes as `null` (no issue). `Some(d)` serializes as `{"secs": i64, "nanos": u32}`. For new domain objects created via `Action::new()`, duration is always `None`. On the Kotlin side, use `JsonElement?` so the format doesn't matter.
- **UUIDs**: serialize as lowercase hyphenated strings (`"550e8400-..."`). Kotlin receives them as `String`.
- **`BitFlags`**: serialize as integers. Kotlin receives as `Int`.
- Always use `Json { ignoreUnknownKeys = true }` on the Kotlin side so adding new Rust fields doesn't crash older app versions.
