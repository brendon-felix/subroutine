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

---

## kotlinx.serialization `encodeDefaults` gotcha — CRITICAL

**Default is `false` in kotlinx.serialization 1.6+.** Fields whose value equals their declared default are **silently omitted** from the serialized JSON.

This causes HTTP 422 from the Rust/axum server because serde requires all non-`Option` fields to be present.

**Always set `encodeDefaults = true` in the `Json {}` instance used for Retrofit:**

```kotlin
// RetrofitClient.kt
private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }
```

This applies to the converter factory (request body serialization). The repository's local `Json` instance used only for *decoding* `RustBridge` stub output doesn't technically need it, but it's harmless to add.

Affected fields in this project: `val saved: Boolean = false` on `Action` and `Event`. Without `encodeDefaults = true`, `saved = false` is dropped from PUT bodies, causing the server to reject with `missing field 'saved'`.

---

## HTTP error body extraction

Retrofit's `HttpException.message` only contains the status line (e.g. `"HTTP 422 Unprocessable Entity"`). To see the server's actual error detail, read the response body:

```kotlin
import retrofit2.HttpException

try {
    api.upsertAction(action.id, action)
} catch (e: HttpException) {
    val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
    Log.e("Repo", "HTTP ${e.code()}: $errorBody")
    throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
}
```

The axum server includes a human-readable serde error in the 422 body, e.g.:
> `Failed to deserialize the JSON body into the target type: missing field 'saved' at line 1 column 154`

Change OkHttp logging to `Level.BODY` to see full request/response in Logcat during debugging:

```kotlin
HttpLoggingInterceptor().apply { level = HttpLoggingInterceptor.Level.BODY }
```

---

## Navigation with Navigation3

Route keys live in a dedicated `NavRoutes.kt`. Each is a `@Serializable` data class or data object implementing `NavKey`:

```kotlin
import androidx.navigation3.runtime.NavKey
import kotlinx.serialization.Serializable

@Serializable data object MainRoute : NavKey
@Serializable data class EditActionRoute(val actionId: String) : NavKey
@Serializable data class EditEventRoute(val eventId: String)   : NavKey
```

Wiring in `MainActivity`:

```kotlin
val backStack = rememberNavBackStack(MainRoute)

NavDisplay(
    backStack = backStack,
    onBack = { backStack.removeLastOrNull() },
    entryProvider = entryProvider {
        entry<MainRoute> {
            MainScreen(
                onEditAction = { id -> backStack.add(EditActionRoute(id)) },
                onEditEvent  = { id -> backStack.add(EditEventRoute(id)) },
            )
        }
        entry<EditActionRoute> { route ->
            EditActionScreen(actionId = route.actionId, onBack = { backStack.removeLastOrNull() })
        }
        entry<EditEventRoute> { route ->
            EditEventScreen(eventId = route.eventId, onBack = { backStack.removeLastOrNull() })
        }
    },
)
```

Pass navigation callbacks as plain lambdas down to screens/components — screens never touch the back stack directly.

---

## Edit screen pattern

All edit screens follow the same structure:

1. **Resolve the entity** from `UiState.Success` at the top of the composable. If `null`, call `onBack()` and `return` immediately.
2. **Delegate to a pure `Content` composable** that owns all local state with `rememberSaveable(entity.id)`.
3. **ViewModel save methods** accept the ID + new values, resolve the entity internally, call the repository, then `loadActions()` and invoke `onSuccess` (usually `onBack`).
4. **`_saving: MutableStateFlow<Boolean>`** in the ViewModel — set around repository calls, disable all buttons while `true`.

```kotlin
// ViewModel
private val _saving = MutableStateFlow(false)
val saving: StateFlow<Boolean> = _saving.asStateFlow()

fun saveEvent(eventId: String, title: String, content: String?, onSuccess: () -> Unit) {
    val event = resolveEvent(_uiState.value, eventId) ?: return
    viewModelScope.launch {
        _saving.value = true
        runCatching { repository.updateEvent(event, title, content) }
            .onSuccess { loadActions(); onSuccess() }
            .onFailure { e -> _uiState.value = UiState.Error(e.message ?: "Save failed") }
        _saving.value = false
    }
}
```

```kotlin
// Screen entrypoint — resolves entity, delegates to Content
@Composable
fun EditEventScreen(eventId: String, viewModel: MainViewModel, onBack: () -> Unit) {
    val uiState by viewModel.uiState.collectAsStateWithLifecycle()
    val saving  by viewModel.saving.collectAsStateWithLifecycle()

    val event = (uiState as? ActionsUiState.Success)
        ?.queueItems
        ?.filterIsInstance<QueueItem.EventItem>()
        ?.firstOrNull { it.event.id == eventId }
        ?.event
        ?: run { onBack(); return }

    EditEventContent(
        event   = event,
        saving  = saving,
        onBack  = onBack,
        onSave  = { t, c -> viewModel.saveEvent(eventId, t, c, onBack) },
        onDelete = { viewModel.deleteEvent(eventId, onBack) },
    )
}
```

Delete actions should always show an `AlertDialog` confirmation before calling the ViewModel.
