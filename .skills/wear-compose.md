# Wear Compose Material 3 — Quick Reference

> Targets `androidx.wear.compose:compose-*:1.6.0-beta01` (latest). All content assumes Kotlin + Jetpack Compose + Material 3 Expressive.
> Do NOT use the legacy `compose-material` library. Use `compose-material3` exclusively.

## Dependencies

```kotlin
dependencies {
    implementation("androidx.wear.compose:compose-material3:1.6.0-beta01")
    implementation("androidx.wear.compose:compose-foundation:1.6.0-beta01")
    implementation("androidx.wear.compose:compose-navigation:1.6.0-beta01")
    implementation("androidx.wear.compose:compose-ui-tooling:1.6.0-beta01")

    // Do NOT add androidx.compose.material:material — compose-material3 is its replacement.
}
```

---

## Screen Structure — AppScaffold & ScreenScaffold

Every screen uses exactly one `AppScaffold` at the Activity level and one `ScreenScaffold` per screen. They automatically coordinate `TimeText`, `ScrollIndicator`, and `PageIndicator` animations.

```kotlin
AppScaffold(timeText = { TimeText() }) {
    ScreenScaffold(
        scrollState = listState,
        contentPadding = contentPadding,
    ) { contentPadding ->
        TransformingLazyColumn(
            state = listState,
            contentPadding = contentPadding,
        ) { /* items */ }
    }
}
```

- `AppScaffold` holds `TimeText` at the app level so it persists across navigation transitions.
- `ScreenScaffold` wires `ScrollIndicator` and `TimeText` scroll-away behavior automatically.
- Always pass `contentPadding` from `rememberResponsiveColumnPadding` (Horologist) into both `ScreenScaffold` and `TransformingLazyColumn`.
- `ScreenScaffold` takes a content lambda `{ contentPadding -> ... }` — pass that lambda parameter down to `TransformingLazyColumn`, not the outer `contentPadding` value.

### ResponsiveTransformingLazyColumn (1.6+)

A Material 3 wrapper around `TransformingLazyColumn` that automatically calculates and applies responsive vertical padding based on item content types. Prefer this over manual `rememberResponsiveColumnPadding` + `TransformingLazyColumn` when content types map cleanly to the standard set:

```kotlin
ResponsiveTransformingLazyColumn(state = listState) {
    item(contentType = "header") {
        ListHeader(
            modifier = Modifier.fillMaxWidth().transformedHeight(this, transformationSpec),
            transformation = SurfaceTransformation(transformationSpec),
        ) { Text("Header") }
    }
    items(myList, key = { it.id }, contentType = { "card" }) { item ->
        MyCard(item = item, transformationSpec = transformationSpec)
    }
}
```

### Minimum vertical content padding (1.6+)

`Modifier.minimumVerticalContentPadding` defines a preferred content padding for items when they are at the top or bottom edges of the list. Recommended defaults are in component-specific defaults objects such as `ButtonDefaults`, `CardDefaults`, etc.:

```kotlin
Button(
    modifier = Modifier
        .fillMaxWidth()
        .transformedHeight(this, transformationSpec)
        .minimumVerticalContentPadding(ButtonDefaults.minimumVerticalListContentPadding),
    // ...
)
```

---

## TransformingLazyColumn

The standard scrolling list for Wear OS. Applies scaling and morphing animations as items approach screen edges. Without the transformation modifiers the list falls back to an unoptimized, janky path.

### Three required pieces for every item

1. `Modifier.transformedHeight(this, transformationSpec)` — pre-calculates height changes per frame.
2. `transformation = SurfaceTransformation(transformationSpec)` — applies the visual scale/morph (on components that accept this: `Card`, `Button`, `ListHeader`, etc.).
3. `rememberTransformationSpec()` — shared across all items on a screen.

`rememberResponsiveColumnPadding` is from **Horologist** (`com.google.android.horologist.compose.layout`), not from `compose-material3`.

### Canonical setup

```kotlin
import com.google.android.horologist.compose.layout.ColumnItemType
import com.google.android.horologist.compose.layout.rememberResponsiveColumnPadding
import androidx.wear.compose.material3.lazy.rememberTransformationSpec
import androidx.wear.compose.material3.lazy.transformedHeight
import androidx.wear.compose.material3.SurfaceTransformation

@Composable
fun MyScreen() {
    val listState = rememberTransformingLazyColumnState()
    val transformationSpec = rememberTransformationSpec()
    val contentPadding = rememberResponsiveColumnPadding(
        first = ColumnItemType.ListHeader,
        last = ColumnItemType.Button,
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

### Item composables inside the list

Card items should be `TransformingLazyColumnItemScope` receivers so `transformedHeight` can reference the scope:

```kotlin
@Composable
fun TransformingLazyColumnItemScope.MyCard(
    item: MyItem,
    transformationSpec: TransformationSpec,
) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .transformedHeight(this, transformationSpec),
        transformation = SurfaceTransformation(transformationSpec),
    ) { /* content */ }
}
```

`Text` and other elements that lack a `transformation` parameter still need `transformedHeight`:

```kotlin
Text(
    text = "...",
    modifier = Modifier.fillMaxWidth().transformedHeight(this, transformationSpec),
)
```

### Non-clickable Card (1.6+)

In 1.6.0-alpha01+, `Card` has non-clickable overloads. Use these for presentational-only items — they avoid allocating interaction state per frame, which is a meaningful scroll performance win:

```kotlin
// Non-clickable (no onClick):
Card(
    modifier = Modifier.fillMaxWidth().transformedHeight(this, transformationSpec),
    transformation = SurfaceTransformation(transformationSpec),
) { /* content */ }

// Clickable (only when the card itself needs to be tapped):
Card(
    onClick = { },
    modifier = Modifier.fillMaxWidth().transformedHeight(this, transformationSpec),
    transformation = SurfaceTransformation(transformationSpec),
) { /* content */ }
```

### Item animations

Use `Modifier.animateItem()` on items to animate insertions, removals, and moves:

```kotlin
items(myList, key = { it.id }) { item ->
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .animateItem()
            .transformedHeight(this, transformationSpec),
        // ...
    )
}
```

### Snapping

Configure snapping for both touch and rotary for a consistent experience:

```kotlin
TransformingLazyColumn(
    flingBehavior = TransformingLazyColumnDefaults.snapFlingBehavior(state),
    rotaryScrollableBehavior = RotaryScrollableDefaults.snapBehavior(state),
)
```

### Controlling scroll position

```kotlin
val listState = rememberTransformingLazyColumnState()

// Scroll immediately
listState.scrollToItem(index = 2)

// Animated scroll
listState.animateScrollToItem(index = 2)
```

### Scroll performance — key rules

1. **Missing `transformedHeight`** — the single biggest cause of jank. Apply to every item.
2. **Lambda allocations per recompose** — `onClick = { viewModel.action(item.id) }` creates a new lambda on every recomposition. Fix with `remember(item.id) { { viewModel.action(item.id) } }`.
3. **`Loading` state on actions** — setting `uiState = Loading` in response to a button tap makes the list flash away. Use optimistic UI instead.
4. **Wide recomposition scope** — `collectAsState()` at the top of a composable containing the full list causes the whole tree to recompose. Extract the list into a child composable.
5. **Non-clickable card before 1.6** — even `Card(onClick = {})` with an empty lambda allocates interaction state. Upgrade to 1.6+ or use a plain container.
6. **Always test on a release build** on physical hardware. Debug builds disable baseline profiles and have significant overhead.

---

## Navigation

### SwipeDismissableNavHost (compose-navigation)

Standard nav with swipe-to-dismiss. Uses `PredictiveBackHandler` on API 36+.

```kotlin
val navController = rememberSwipeDismissableNavController()
SwipeDismissableNavHost(navController = navController, startDestination = "home") {
    composable("home") { HomeScreen(navController) }
    composable("detail") { DetailScreen() }
}
```

Use `Modifier.edgeSwipeToDismiss(swipeToDismissBoxState)` on horizontally-scrollable content inside a `SwipeDismissableNavHost` to restore swipe-to-dismiss from the left edge.

### Navigation3 (compose-navigation3, 1.6+)

Integrates with the Navigation 3 library via `SwipeDismissableSceneStrategy`:

```kotlin
val strategyState = rememberSwipeDismissableSceneStrategyState()

NavDisplay(
    backStack = backStack,
    sceneStrategy = SwipeDismissableSceneStrategy(strategyState),
) { /* content */ }
```

---

## Buttons

### Standard buttons with shape morphing

Always pass `shapes` for expressive morphing animations on press:

```kotlin
Button(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Primary") }
FilledTonalButton(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Tonal") }
OutlinedButton(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Outlined") }
TextButton(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Text") }

// Icon buttons
FilledIconButton(onClick = {}, shapes = IconButtonDefaults.shapes()) { Icon(...) }
FilledTonalIconButton(onClick = {}, shapes = IconButtonDefaults.shapes()) { Icon(...) }

// Toggle icon button
FilledIconToggleButton(
    checked = checked,
    onCheckedChange = { checked = it },
    shapes = IconButtonDefaults.toggleableShapes(),
) { Icon(...) }
```

### TextButton / IconButton toggle variants

```kotlin
TextToggleButton(
    checked = checked,
    onCheckedChange = { checked = it },
    shapes = TextToggleButtonDefaults.shapes(),
) { Text(if (checked) "On" else "Off") }
```

### EdgeButton

A special button for the bottom of the screen, hugging the display edge. Used inside `ScreenScaffold`:

```kotlin
ScreenScaffold(
    scrollState = listState,
    edgeButton = { EdgeButton(onClick = { }) { Text("Confirm") } }
) { /* content */ }
```

### ButtonGroup

An expressive row of buttons that shape-morph when touched:

```kotlin
ButtonGroup {
    item { Button(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("A") } }
    item { Button(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("B") } }
}
```

---

## Toggle Controls

```kotlin
// Checkbox row
CheckboxButton(
    checked = checked,
    onCheckedChange = { checked = it },
    label = { Text("Option") },
)

// Switch row
SwitchButton(
    checked = checked,
    onCheckedChange = { checked = it },
    label = { Text("Enable") },
)

// Radio selection row
RadioButton(
    selected = selected,
    onSelect = { selected = true },
    label = { Text("Choice") },
)

// Split variants (body click separate from toggle click)
SplitCheckboxButton(checked = checked, onCheckedChange = { }, onContainerClick = { }, label = { Text("...") })
SplitSwitchButton(checked = checked, onCheckedChange = { }, onContainerClick = { }, label = { Text("...") })
SplitRadioButton(selected = selected, onSelect = { }, onContainerClick = { }, label = { Text("...") })
```

---

## Dialogs

### AlertDialog

```kotlin
// With confirm/dismiss buttons
AlertDialog(
    show = showDialog,
    onDismissRequest = { showDialog = false },
    title = { Text("Title") },
    confirmButton = { Button(onClick = { showDialog = false }) { Text("OK") } },
    dismissButton = { Button(onClick = { showDialog = false }) { Text("Cancel") } },
)

// With EdgeButton
AlertDialog(
    show = showDialog,
    onDismissRequest = { showDialog = false },
    title = { Text("Title") },
    edgeButton = { EdgeButton(onClick = { showDialog = false }) { Text("OK") } },
)
```

### ConfirmationDialog

Self-dismissing dialog with animated icon for success, failure, or open-on-phone:

```kotlin
SuccessConfirmationDialog(
    show = showConfirmation,
    onDismissRequest = { showConfirmation = false },
    text = { Text("Done!") },
)

FailureConfirmationDialog(
    show = showFailure,
    onDismissRequest = { showFailure = false },
    text = { Text("Failed") },
)

OpenOnPhoneDialog(
    show = showOpenOnPhone,
    onDismissRequest = { showOpenOnPhone = false },
)
```

---

## Pickers

### TimePicker

Fully driven by the user's locale — column ordering, separators, and 12/24h format are all locale-aware.

```kotlin
var selectedTime by remember { mutableStateOf(LocalTime.now()) }
TimePicker(
    onTimePicked = { selectedTime = it },
    time = selectedTime,
)

// Minutes and seconds only (1.6+)
TimePicker(
    onTimePicked = { },
    time = selectedTime,
    timePickerType = TimePickerType.MinutesSeconds,
)

// Specify initially focused column (1.6+)
TimePicker(
    onTimePicked = { },
    time = selectedTime,
    initialSelection = TimePickerType.Hours,
)
```

To render over a custom background (gradient/image), pass `Color.Unspecified` as the `Picker`'s `gradientColor`. For `TimePicker`/`DatePicker`, locally override `MaterialTheme.colorScheme.background` to `Color.Unspecified`.

### DatePicker

```kotlin
DatePicker(
    onDatePicked = { selectedDate = it },
    date = selectedDate,
)
```

### Custom Picker / PickerGroup

```kotlin
val pickerState = rememberPickerState(initialNumberOfOptions = 60, initiallySelectedOption = 0)
Picker(
    state = pickerState,
    contentDescription = "Select value",
) { option ->
    Text("$option")
}
```

---

## Progress Indicators

```kotlin
// Determinate circular
CircularProgressIndicator(progress = { 0.6f })

// Indeterminate circular
CircularProgressIndicator()

// Segmented
CircularProgressIndicator(
    progress = { 0.6f },
    startAngle = 300f,
    endAngle = 240f,
    strokeWidth = ProgressIndicatorDefaults.FullScreenStrokeWidth,
)

// Linear
LinearProgressIndicator(progress = { 0.4f })
```

---

## Cards

```kotlin
// Basic card
Card(onClick = { }) {
    Text("Content")
}

// Title card with metadata slots
TitleCard(
    onClick = { },
    title = { Text("Title") },
    subtitle = { Text("Subtitle") },
    time = { Text("Now") },
) {
    Text("Body content")
}

// App card with icon
AppCard(
    onClick = { },
    appName = { Text("App") },
    appImage = { Icon(Icons.Default.Star, contentDescription = null) },
    title = { Text("Title") },
    time = { Text("12:00") },
) {
    Text("Content")
}
```

---

## Pagers

```kotlin
val pagerState = rememberPagerState { pageCount }

HorizontalPagerScaffold(pagerState = pagerState) {
    HorizontalPager(state = pagerState) { page ->
        // page content
    }
}

VerticalPagerScaffold(pagerState = pagerState) {
    VerticalPager(state = pagerState) { page ->
        // page content
    }
}
```

- Page indicators are positioned automatically by the scaffold. When not using a scaffold: align `HorizontalPageIndicator` to `Alignment.BottomCenter` and `VerticalPageIndicator` to `Alignment.CenterEnd`.
- Add rotary support via `Modifier.rotaryScrollable(RotaryScrollableDefaults.snapBehavior(pagerState))`.

---

## SwipeToReveal

Adds hidden actions revealed on right-to-left swipe. Import from `androidx.wear.compose.material3`.

```kotlin
val revealState = rememberRevealState()

SwipeToReveal(
    state = revealState,
    primaryAction = {
        PrimaryActionButton(
            onClick = { /* delete */ },
            icon = { Icon(Icons.Default.Delete, contentDescription = "Delete") },
            label = { Text("Delete") },
        )
    },
    undoPrimaryAction = {
        UndoActionButton(
            onClick = { /* undo */ },
            label = { Text("Undo") },
        )
    },
) {
    Card(onClick = { }) { Text("Swipe me") }
}
```

- Default swipe direction is right-to-left. Bidirectional is supported but strongly discouraged (conflicts with swipe-to-dismiss).
- Clicks are ignored within a 20 dp threshold at top/bottom edges of `TransformingLazyColumn` items (1.6+).

---

## Sliders & Steppers

```kotlin
// Slider
var sliderValue by remember { mutableStateOf(3f) }
InlineSlider(
    value = sliderValue,
    onValueChange = { sliderValue = it },
    steps = 4,
    valueRange = 1f..5f,
)

// Stepper (full screen)
Stepper(
    value = stepperValue,
    onValueChange = { stepperValue = it },
    steps = 4,
    valueRange = 1f..5f,
) {
    Text("$stepperValue")
}
```

---

## Ripple

Control ripple appearance globally or locally:

```kotlin
// Globally (wrap content)
CompositionLocalProvider(
    LocalRippleConfiguration provides RippleConfiguration(color = Color.White, rippleAlpha = null)
) {
    // content with custom ripple
}

// Disable ripple
CompositionLocalProvider(LocalRippleConfiguration provides null) {
    Button(onClick = { }) { Text("No ripple") }
}
```

---

## Theming

```kotlin
MaterialTheme(
    colorScheme = dynamicColorScheme(context) ?: defaultColorScheme(),
) {
    // content
}
```

- `MaterialTheme.colorScheme` provides all color tokens.
- Dynamic Color Theming automatically generates a color scheme matching the watch face colors.

---

## Ambient Mode (1.6+)

```kotlin
val ambientModeManager = LocalAmbientModeManager.current

// Check current mode (non-exhaustive sealed class)
when (ambientModeManager.ambientMode) {
    is AmbientMode.Interactive -> { /* full color, animations */ }
    is AmbientMode.Ambient -> { /* reduced colors, no animations */ }
    else -> { }
}
```

---

## Rotary Input

`TransformingLazyColumn`, `ScalingLazyColumn`, and `Picker` support rotary by default. For other containers use `Modifier.rotaryScrollable`:

```kotlin
// Continuous scroll
Modifier.rotaryScrollable(
    behavior = RotaryScrollableDefaults.behavior(scrollableState),
    focusRequester = focusRequester,
)

// Snap behavior (use together with touch snap flingBehavior)
Modifier.rotaryScrollable(
    behavior = RotaryScrollableDefaults.snapBehavior(scrollableState),
    focusRequester = focusRequester,
)
```

---

## CurvedText / TimeText

```kotlin
// TimeText with custom content
TimeText {
    timeTextCurvedText("12:00")
    timeTextSeparator()
    timeTextCurvedText("Notification")
}

// Standalone curved text
CurvedLayout {
    curvedText(
        text = "Hello",
        style = CurvedTextStyle(fontSize = 14.sp),
    )
}
```

- Curved text now warps by default (API 34+) for improved cursive font rendering, via `CurvedTextStyle(warpOffset = ...)`.

---

## Placeholders (skeleton loading)

```kotlin
val placeHolderState = rememberPlaceholderState { contentIsReady }

Card(
    modifier = Modifier.placeholder(placeHolderState),
    onClick = { },
) {
    Text(
        text = if (contentIsReady) actualText else "",
        modifier = Modifier.placeholderShimmer(placeHolderState),
    )
}

if (!placeHolderState.isShowContent) {
    LaunchedEffect(placeHolderState) {
        placeHolderState.startPlaceholderAnimation()
    }
}
```

---

## Hierarchical Focus

Used to track the active composable and coordinate rotary/focus in multi-screen layouts:

```kotlin
// Mark a composable as a focus group
Modifier.hierarchicalFocusGroup()

// Attach a focus requester that activates when the group becomes active
Modifier.hierarchicalFocusRequester(focusRequester)

// Check if the current screen is active
val isActive = LocalScreenIsActive.current
```

---

## Accessibility notes

- `TransformingLazyColumn` ignores clicks beyond a 20 dp threshold at the top and bottom edges (prevents accidental taps on partially visible items).
- `Picker` announces with role `ValuePicker`.
- `CheckboxButton`/`SwitchButton` icons are rounded.
- `PageIndicators` are no longer full-screen — use scaffold or explicit alignment.
- `ConfirmationDialog`/`OpenOnPhoneDialog` set `FLAG_KEEP_SCREEN_ON` so animations complete before self-dismissing.
- `SuccessConfirmationDialog` check mark ignores RTL layout direction.