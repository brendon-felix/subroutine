# Wear OS Standalone App — Quick Reference

A general-purpose reference for building a **standalone** Wear OS app with Kotlin/Compose. A standalone watch app runs independently — it does not require a companion phone app to function, though it may optionally communicate with one via the Wearable Data Layer.

---

## Dependencies

Wear Compose libraries are **versioned independently** — do NOT use `androidx.compose:compose-bom` for them.

```toml
# gradle/libs.versions.toml
[versions]
agp                      = "9.0.1"
kotlin                   = "2.1.21"
coreKtx                  = "1.17.0"
lifecycleRuntimeKtx      = "2.10.0"
lifecycleViewmodelCompose = "2.10.0"
activityCompose          = "1.12.4"
coreSplashscreen         = "1.0.1"
wearComposeMaterial3     = "1.6.0-beta01"
composeFoundation        = "1.6.0-beta01"
wearComposeNavigation    = "1.6.0-beta01"
wearToolingPreview       = "1.0.0"
playServicesWearable     = "18.0.0"        # only if communicating with phone
coroutinesPlayServices   = "1.9.0"        # only if communicating with phone
kotlinxSerializationJson = "1.8.1"
horologistComposeLayout  = "0.8.3-alpha"
tiles                    = "1.4.0"
composeBom               = "2025.06.02"   # mobile BOM — debug tooling only

[libraries]
androidx-core-ktx                  = { group = "androidx.core",           name = "core-ktx",                    version.ref = "coreKtx" }
androidx-lifecycle-runtime-ktx     = { group = "androidx.lifecycle",      name = "lifecycle-runtime-ktx",       version.ref = "lifecycleRuntimeKtx" }
androidx-lifecycle-viewmodel-compose = { group = "androidx.lifecycle",    name = "lifecycle-viewmodel-compose", version.ref = "lifecycleViewmodelCompose" }
androidx-activity-compose          = { group = "androidx.activity",       name = "activity-compose",            version.ref = "activityCompose" }
androidx-core-splashscreen         = { group = "androidx.core",           name = "core-splashscreen",           version.ref = "coreSplashscreen" }
androidx-wear-compose-material3    = { group = "androidx.wear.compose",   name = "compose-material3",          version.ref = "wearComposeMaterial3" }
androidx-wear-compose-foundation   = { group = "androidx.wear.compose",   name = "compose-foundation",         version.ref = "composeFoundation" }
androidx-wear-compose-navigation   = { group = "androidx.wear.compose",   name = "compose-navigation",         version.ref = "wearComposeNavigation" }
androidx-wear-tooling-preview      = { group = "androidx.wear",           name = "wear-tooling-preview",       version.ref = "wearToolingPreview" }
horologist-compose-layout          = { group = "com.google.android.horologist", name = "horologist-compose-layout", version.ref = "horologistComposeLayout" }
androidx-tiles                     = { group = "androidx.wear.tiles",     name = "tiles",                      version.ref = "tiles" }
play-services-wearable             = { group = "com.google.android.gms",  name = "play-services-wearable",     version.ref = "playServicesWearable" }
kotlinx-coroutines-play-services   = { group = "org.jetbrains.kotlinx",   name = "kotlinx-coroutines-play-services", version.ref = "coroutinesPlayServices" }
kotlinx-serialization-json         = { group = "org.jetbrains.kotlinx",   name = "kotlinx-serialization-json", version.ref = "kotlinxSerializationJson" }
# Mobile BOM — only used for ui-tooling-preview in debug builds
androidx-compose-bom               = { group = "androidx.compose",        name = "compose-bom-alpha",          version.ref = "composeBom" }
androidx-compose-ui-tooling-preview = { group = "androidx.compose.ui",   name = "ui-tooling-preview" }

[plugins]
android-application  = { id = "com.android.application",               version.ref = "agp" }
kotlin-compose       = { id = "org.jetbrains.kotlin.plugin.compose",   version.ref = "kotlin" }
kotlin-serialization = { id = "org.jetbrains.kotlin.plugin.serialization", version.ref = "kotlin" }
```

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
        minSdk = 30       // Wear OS 3.0 = API 30; Wear OS 4 = API 33
        targetSdk = 36
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
        // Benchmark build: R8-optimized but debuggable — use this for scroll perf testing.
        // Debug builds skip R8 and baseline profiles, causing severe Compose jank on hardware.
        create("benchmark") {
            initWith(getByName("release"))
            isDebuggable = true
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlin {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_11)
        }
    }

    useLibrary("wear-sdk")
    buildFeatures { compose = true }
}

dependencies {
    implementation(libs.play.services.wearable)
    implementation(libs.kotlinx.coroutines.play.services)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.core.splashscreen)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.wear.compose.material3)
    implementation(libs.androidx.wear.compose.foundation)
    implementation(libs.androidx.wear.compose.navigation)
    implementation(libs.horologist.compose.layout)

    // Do NOT add androidx.compose.material3 here — that is the mobile variant
    // and conflicts with the Wear-specific versions.
    // ui-tooling-preview via mobile BOM is fine for IDE previews only:
    debugImplementation(platform(libs.androidx.compose.bom))
    debugImplementation(libs.androidx.compose.ui.tooling.preview)
}
```

---

## Screen Structure

### AppScaffold + ScreenScaffold

- One `AppScaffold` per `Activity` — holds `TimeText` at the app level, persisting across navigation.
- One `ScreenScaffold` per screen/route — wires up `ScrollIndicator` and `TimeText` scroll-away automatically.
- Always pass `scrollState` and `contentPadding` from `rememberResponsiveColumnPadding` into `ScreenScaffold`, then pass the lambda's `contentPadding` down to `TransformingLazyColumn`. Never hardcode padding.

```kotlin
@Composable
fun MyScreen() {
    val listState = rememberTransformingLazyColumnState()
    val transformationSpec = rememberTransformationSpec()
    val contentPadding = rememberResponsiveColumnPadding(
        first = ColumnItemType.ListHeader,
        last  = ColumnItemType.Button,
    )

    AppScaffold(timeText = { TimeText() }) {
        ScreenScaffold(
            scrollState   = listState,
            contentPadding = contentPadding,
        ) { contentPadding ->
            TransformingLazyColumn(
                state          = listState,
                contentPadding = contentPadding,
            ) {
                // items...
            }
        }
    }
}
```

`rememberResponsiveColumnPadding` is from **Horologist** (`com.google.android.horologist.compose.layout`), not from `androidx.wear.compose.material3`.

---

## TransformingLazyColumn

The standard scrolling list for Wear OS Material 3. It applies scaling and morphing animations as items approach screen edges. **Without the transformation modifiers the list falls back to an unoptimised path and stutters badly.**

### Three required pieces for every item

1. `Modifier.transformedHeight(this, transformationSpec)` — lets layout pre-calculate height changes per frame.
2. `transformation = SurfaceTransformation(transformationSpec)` — applies scale/morph effect (on `Card`, `Button`, `ListHeader`, etc.).
3. `rememberTransformationSpec()` — shared across all items on a screen, created once at the screen level.

### Full canonical setup

```kotlin
import com.google.android.horologist.compose.layout.ColumnItemType
import com.google.android.horologist.compose.layout.rememberResponsiveColumnPadding
import androidx.wear.compose.material3.lazy.rememberTransformationSpec
import androidx.wear.compose.material3.lazy.transformedHeight
import androidx.wear.compose.material3.SurfaceTransformation

@Composable
fun MyListScreen() {
    val listState        = rememberTransformingLazyColumnState()
    val transformationSpec = rememberTransformationSpec()
    val contentPadding   = rememberResponsiveColumnPadding(
        first = ColumnItemType.ListHeader,
        last  = ColumnItemType.Button,
    )

    AppScaffold(timeText = { TimeText() }) {
        ScreenScaffold(scrollState = listState, contentPadding = contentPadding) { contentPadding ->
            TransformingLazyColumn(state = listState, contentPadding = contentPadding) {

                item {
                    ListHeader(
                        modifier = Modifier
                            .fillMaxWidth()
                            .transformedHeight(this, transformationSpec),
                        transformation = SurfaceTransformation(transformationSpec),
                    ) { Text("Header") }
                }

                items(myList, key = { it.id }) { item ->
                    MyCard(item = item, transformationSpec = transformationSpec)
                }

                item {
                    Button(
                        onClick = { },
                        modifier = Modifier
                            .fillMaxWidth()
                            .transformedHeight(this, transformationSpec),
                        transformation = SurfaceTransformation(transformationSpec),
                    ) { Text("Action") }
                }
            }
        }
    }
}
```

### Item composables

Make item composables receivers of `TransformingLazyColumnItemScope` so `transformedHeight` can reference the item scope:

```kotlin
@Composable
fun TransformingLazyColumnItemScope.MyCard(
    item: MyItem,
    transformationSpec: TransformationSpec,
) {
    // Non-clickable card (1.6+) — no onClick, no MutableInteractionSource allocation per frame
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .transformedHeight(this, transformationSpec),
        transformation = SurfaceTransformation(transformationSpec),
    ) {
        Text(item.title)
    }
}
```

For `Text` and other elements that have no `transformation` parameter, only apply `transformedHeight`:

```kotlin
Text(
    text = "...",
    modifier = Modifier.fillMaxWidth().transformedHeight(this, transformationSpec),
)
```

### Non-clickable Card (1.6+)

In `wear-compose-material3` 1.6.0+, `Card` has non-clickable overloads. Use them for purely presentational items — avoids allocating `MutableInteractionSource` + ripple state per item per frame.

```kotlin
// Non-clickable — no onClick parameter:
Card(
    modifier       = Modifier.fillMaxWidth().transformedHeight(this, transformationSpec),
    transformation = SurfaceTransformation(transformationSpec),
) { /* content */ }

// Clickable — only when the card itself must be tapped:
Card(
    onClick        = { },
    modifier       = Modifier.fillMaxWidth().transformedHeight(this, transformationSpec),
    transformation = SurfaceTransformation(transformationSpec),
) { /* content */ }
```

---

## Scroll Performance — Key Rules

1. **Missing `transformedHeight`** — the most impactful issue. Apply to every item without exception.
2. **Lambda allocations per recompose** — `onClick = { viewModel.action(item.id) }` creates a new lambda every recomposition. Fix with `remember(item.id) { { viewModel.action(item.id) } }` or hoist to a `@Stable` callbacks holder via `remember(viewModel)`.
3. **`Loading` state on user actions** — setting `uiState = Loading` in response to a button tap collapses the list to a spinner mid-scroll. Use **optimistic UI** instead: update local state immediately, let the server/phone response reconcile.
4. **Wide recomposition scope** — `collectAsState()` at the top of a composable containing the entire list causes the whole tree to recompose on every state change. Extract the list into a child composable to narrow the scope.
5. **Debug builds are slow** — always test scroll performance with the `benchmark` build variant on a physical watch. Debug builds skip R8 and baseline profiles, producing severe jank that is not representative of release performance.

```sh
# Build benchmark variant (R8 on, debuggable)
./gradlew :app:assembleBenchmark
adb -t <watch-transport-id> install app/build/outputs/apk/benchmark/app-benchmark.apk
```

---

## Architecture Pattern

```
WearMessageListenerService   — WearableListenerService; forwards background messages via broadcast
MyRepository                 — sends Wearable messages or reads local data; decodes responses
MyViewModel                  — AndroidViewModel; owns StateFlow<UiState>; optimistic mutations
Screen composables           — collect StateFlow; call ViewModel; no direct repo access
```

### UI state model

```kotlin
sealed interface MyUiState {
    data object Loading                          : MyUiState
    data object NoPhone                          : MyUiState  // if phone comms needed
    data class  Refreshing(val data: MyData)     : MyUiState  // keep list visible during reload
    data class  Success(val data: MyData)        : MyUiState
    data class  Error(val message: String)       : MyUiState
}
```

- Use `Refreshing` (not `Loading`) when data is already displayed and the user requests a reload — keep the list on screen, show only a small spinner.
- Never transition to `Loading` from a user action on a list item.
- Optimistic mutations: remove/update the item in local state immediately, then send the network request. The authoritative response reconciles.

---

## Phone ↔ Watch Communication (Wearable Data Layer)

The **Wearable Message API** (`MessageClient`) is suitable for small, request/response payloads. The **Data Layer API** (`DataClient`) is better for persistent state sync.

### Message path conventions

```
/myfeature/load              Watch → Phone   request data
/myfeature/load/response     Phone → Watch   JSON response
/myfeature/action/complete   Watch → Phone   mutation (body = ID as UTF-8 bytes)
```

### Finding the phone node

Capability advertisement via `CapabilityClient` can be unreliable for sideloaded/debug apps. Enumerate nodes directly:

```kotlin
suspend fun findPhoneNodeId(context: Context): String? {
    val nodes = Wearable.getNodeClient(context).connectedNodes.await()
    return nodes.firstOrNull { it.isNearby }?.id ?: nodes.firstOrNull()?.id
}
```

### Repository (watch side)

```kotlin
class MyWearRepository(private val context: Context) {

    private val messageClient by lazy { Wearable.getMessageClient(context) }
    private val nodeClient    by lazy { Wearable.getNodeClient(context) }
    private val json = Json { ignoreUnknownKeys = true }

    suspend fun findPhoneNodeId(): String? {
        val nodes = nodeClient.connectedNodes.await()
        return nodes.firstOrNull { it.isNearby }?.id ?: nodes.firstOrNull()?.id
    }

    suspend fun requestLoad(nodeId: String) {
        messageClient.sendMessage(nodeId, "/myfeature/load", byteArrayOf()).await()
    }

    suspend fun sendComplete(nodeId: String, id: String) {
        messageClient.sendMessage(nodeId, "/myfeature/action/complete", id.toByteArray()).await()
    }

    fun decodeResponse(bytes: ByteArray): Result<MyPayload> =
        runCatching { json.decodeFromString<MyPayload>(bytes.decodeToString()) }
}
```

### WearableListenerService (background message receiver)

The `MessageClient` foreground listener only fires when the app is alive. For background delivery, implement a `WearableListenerService` that broadcasts the raw bytes locally:

```kotlin
class MyWearMessageListenerService : WearableListenerService() {

    override fun onMessageReceived(event: MessageEvent) {
        if (event.path == "/myfeature/load/response") {
            sendBroadcast(
                Intent(ACTION_RESPONSE).apply {
                    putExtra(EXTRA_PAYLOAD, event.data)
                    setPackage(packageName)
                }
            )
        }
    }

    companion object {
        const val ACTION_RESPONSE = "com.example.myapp.ACTION_RESPONSE"
        const val EXTRA_PAYLOAD   = "payload"
    }
}
```

Declare in `AndroidManifest.xml`:

```xml
<service
    android:name=".MyWearMessageListenerService"
    android:exported="true">
    <intent-filter>
        <action android:name="com.google.android.gms.wearable.MESSAGE_RECEIVED" />
        <data
            android:scheme="wear"
            android:host="*"
            android:pathPrefix="/myfeature" />
    </intent-filter>
</service>
```

### ViewModel — foreground + background listener wiring

Register both the `MessageClient` listener (foreground) and the `BroadcastReceiver` (background) in the same ViewModel. Suppress duplicate delivery from the background path when the foreground path already handled it:

```kotlin
class MyViewModel(application: Application) : AndroidViewModel(application) {

    private val repository = MyWearRepository(application)

    private val _uiState = MutableStateFlow<MyUiState>(MyUiState.Loading)
    val uiState: StateFlow<MyUiState> = _uiState

    private var phoneNodeId: String? = null
    private val lastForegroundHandledAt = AtomicLong(0L)

    // Foreground: fires on main thread — dispatch decode to IO immediately.
    private val messageListener = MessageClient.OnMessageReceivedListener { event ->
        if (event.path == "/myfeature/load/response") {
            lastForegroundHandledAt.set(System.currentTimeMillis())
            val data = event.data
            viewModelScope.launch(Dispatchers.IO) { handleResponse(data) }
        }
    }

    // Background: suppress if foreground handled the same message recently.
    private val broadcastReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val payload = intent.getByteArrayExtra(MyWearMessageListenerService.EXTRA_PAYLOAD) ?: return
            val age = System.currentTimeMillis() - lastForegroundHandledAt.get()
            if (age < 2_000L) return  // foreground already handled it
            viewModelScope.launch(Dispatchers.IO) { handleResponse(payload) }
        }
    }

    init {
        ContextCompat.registerReceiver(
            application, broadcastReceiver,
            IntentFilter(MyWearMessageListenerService.ACTION_RESPONSE),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        // Register listener and send first request in one coroutine — no gap between them.
        viewModelScope.launch(Dispatchers.IO) {
            runCatching { Wearable.getMessageClient(application).addListener(messageListener) }
            loadInternal()
        }
    }

    override fun onCleared() {
        super.onCleared()
        val app = getApplication<Application>()
        runCatching { Wearable.getMessageClient(app).removeListener(messageListener) }
        app.unregisterReceiver(broadcastReceiver)
    }

    fun load() { viewModelScope.launch(Dispatchers.IO) { loadInternal() } }

    private suspend fun loadInternal() {
        // Transition to Refreshing if data is already visible — keeps the list on screen.
        _uiState.value = when (val s = _uiState.value) {
            is MyUiState.Success    -> MyUiState.Refreshing(s.data)
            is MyUiState.Refreshing -> s
            else                    -> MyUiState.Loading
        }
        val nodeId = runCatching { repository.findPhoneNodeId() }.getOrNull()
        if (nodeId == null) { _uiState.value = MyUiState.NoPhone; return }
        phoneNodeId = nodeId
        runCatching { repository.requestLoad(nodeId) }
            .onFailure { _uiState.value = MyUiState.Error("Failed to contact phone: ${it.message}") }
    }

    // Optimistic mutation — update local state immediately; phone response reconciles.
    fun completeItem(id: String) {
        removeOptimistically(id)
        viewModelScope.launch(Dispatchers.IO) {
            val nodeId = phoneNodeId ?: run { loadInternal(); return@launch }
            runCatching { repository.sendComplete(nodeId, id) }
                .onFailure { _uiState.value = MyUiState.Error("Complete failed: ${it.message}") }
        }
    }

    private fun removeOptimistically(id: String) {
        val data = when (val s = _uiState.value) {
            is MyUiState.Success    -> s.data
            is MyUiState.Refreshing -> s.data
            else                    -> return
        }
        _uiState.value = MyUiState.Success(data.withoutItem(id))
    }

    // StateFlow.value is thread-safe — can be set from IO dispatcher.
    private fun handleResponse(bytes: ByteArray) {
        repository.decodeResponse(bytes)
            .onSuccess { _uiState.value = MyUiState.Success(it) }
            .onFailure { _uiState.value = MyUiState.Error("Decode error: ${it.message}") }
    }
}
```

### Phone side — WearableListenerService

The phone app (or standalone Android app acting as data source) runs its own `WearableListenerService`:

```kotlin
class PhoneWearListenerService : WearableListenerService() {

    override fun onMessageReceived(event: MessageEvent) {
        when (event.path) {
            "/myfeature/load" -> {
                val json = runCatching { loadData() }.getOrElse { """{"error":"${it.message}"}""" }
                sendReply(event.sourceNodeId, "/myfeature/load/response", json.toByteArray())
            }
            "/myfeature/action/complete" -> {
                val id = event.data.decodeToString()
                runCatching { completeItem(id) }
                val json = runCatching { loadData() }.getOrElse { """{"error":"reload failed"}""" }
                sendReply(event.sourceNodeId, "/myfeature/load/response", json.toByteArray())
            }
        }
    }

    private fun sendReply(nodeId: String, path: String, data: ByteArray) {
        runBlocking {
            runCatching {
                Wearable.getMessageClient(this@PhoneWearListenerService)
                    .sendMessage(nodeId, path, data)
                    .await()
            }
        }
    }
}
```

### JSON serialization across the Wearable boundary

- Phone serializes responses as JSON (via Rust `serde_json` or Kotlin `kotlinx.serialization`).
- Watch deserializes: `Json { ignoreUnknownKeys = true }.decodeFromString<T>(bytes.decodeToString())`.
- Simple IDs and mutation payloads are sent as raw UTF-8 bytes — no JSON wrapper needed.

---

## Deployment

```sh
# Build
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
cd wear_os && ./gradlew :app:assembleDebug

# List devices — watch appears as a separate ADB device
adb devices -l

# Install to watch by transport ID (not device serial)
adb -t <transport-id> install app/build/outputs/apk/debug/app-debug.apk
```

The watch appears as a separate ADB entry when:
- Bluetooth debugging is enabled on the watch (Settings → Developer Options), or
- The watch is connected directly via USB.

`installDebug` without `-t` may attempt installation on the paired phone instead.

---

## Tiles

Tiles are surfaces displayed on the watch face carousel without opening the full app. They are implemented as `TileService` subclasses.

```kotlin
class MainTileService : TileService() {

    override fun onTileRequest(requestParams: RequestBuilders.TileRequest) =
        Futures.immediateFuture(
            Tile.Builder()
                .setResourcesVersion("1")
                .setTileTimeline(
                    Timeline.fromLayoutElement(
                        Text.Builder()
                            .setText(buildString { append("Hello from tile") })
                            .build()
                    )
                )
                .build()
        )

    override fun onResourcesRequest(requestParams: RequestBuilders.ResourcesRequest) =
        Futures.immediateFuture(Resources.Builder().setVersion("1").build())
}
```

Declare in `AndroidManifest.xml`:

```xml
<service
    android:name=".tile.MainTileService"
    android:exported="true"
    android:permission="com.google.android.wearable.permission.BIND_TILE_PROVIDER">
    <intent-filter>
        <action android:name="androidx.wear.tiles.action.BIND_TILE_PROVIDER" />
    </intent-filter>
    <meta-data
        android:name="androidx.wear.tiles.PREVIEW"
        android:resource="@drawable/tile_preview" />
</service>
```

Build dependencies for tiles:

```kotlin
implementation(libs.androidx.tiles)
implementation(libs.androidx.tiles.material)
debugImplementation(libs.androidx.tiles.tooling)
debugImplementation(libs.androidx.tiles.tooling.preview)
```

---

## Complications

Complications display small data items on watch faces. Implement `SuspendingComplicationDataSourceService`:

```kotlin
class MainComplicationService : SuspendingComplicationDataSourceService() {

    override fun getPreviewData(type: ComplicationType) =
        ShortTextComplicationData.Builder(
            text = PlainComplicationText.Builder("42").build(),
            contentDescription = PlainComplicationText.Builder("Count").build(),
        ).build()

    override suspend fun onComplicationRequest(request: ComplicationRequest) =
        when (request.complicationType) {
            ComplicationType.SHORT_TEXT ->
                ShortTextComplicationData.Builder(
                    text = PlainComplicationText.Builder(loadCurrentValue()).build(),
                    contentDescription = PlainComplicationText.Builder("My value").build(),
                ).build()
            else -> null
        }
}
```

Declare in `AndroidManifest.xml`:

```xml
<service
    android:name=".complication.MainComplicationService"
    android:exported="true"
    android:icon="@drawable/ic_complication"
    android:permission="com.google.android.wearable.permission.BIND_COMPLICATION_PROVIDER">
    <intent-filter>
        <action android:name="androidx.wear.watchface.complications.data.source.BIND_COMPLICATION_DATA_SOURCE" />
    </intent-filter>
    <meta-data
        android:name="androidx.wear.watchface.complications.data.source.SUPPORTED_TYPES"
        android:value="SHORT_TEXT,RANGED_VALUE" />
    <meta-data
        android:name="androidx.wear.watchface.complications.data.source.UPDATE_PERIOD_SECONDS"
        android:value="600" />
</service>
```
