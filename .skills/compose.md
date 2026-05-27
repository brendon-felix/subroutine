# Android Compose Material 3 Expressive — Quick Reference

> Targets `androidx.compose.material3` via `compose-bom-alpha:2025.06.02` (latest). All content assumes Kotlin + Jetpack Compose + Material 3 Expressive.
> Expressive components require `@OptIn(ExperimentalMaterial3ExpressiveApi::class)` — set this globally via `freeCompilerArgs` rather than per-file.

## Dependencies

```kotlin
// build.gradle.kts (app module)
dependencies {
    implementation(platform("androidx.compose:compose-bom-alpha:2025.06.02"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended") // optional, for Icons.*

    // Navigation 3 (new API — see Navigation section)
    implementation("androidx.navigation3:navigation3-runtime:1.0.0-alpha05")
    implementation("androidx.navigation3:navigation3-ui:1.0.0-alpha05")
    implementation("androidx.compose.material3.adaptive:adaptive-navigation3:1.0.0-SNAPSHOT")
    implementation("androidx.lifecycle:lifecycle-viewmodel-navigation3:1.0.0-alpha03")

    debugImplementation("androidx.compose.ui:ui-tooling")
}
```

```toml
# gradle/libs.versions.toml
[versions]
composeBom = "2025.06.02"

[libraries]
# Use compose-bom-alpha, NOT compose-bom, for expressive components
androidx-compose-bom = { group = "androidx.compose", name = "compose-bom-alpha", version.ref = "composeBom" }
```

```kotlin
// build.gradle.kts — global opt-in for expressive APIs
kotlin {
    compilerOptions {
        freeCompilerArgs.addAll(
            "-Xopt-in=androidx.compose.material3.ExperimentalMaterial3ExpressiveApi",
            "-Xopt-in=androidx.compose.material3.ExperimentalMaterial3Api"
        )
    }
}
```

---

## Activity Setup

```kotlin
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()   // edge-to-edge is required for expressive layouts
        setContent {
            MyAppTheme {
                AppNavigation()
            }
        }
    }
}
```

- Always call `enableEdgeToEdge()` before `setContent` — expressive components like `HorizontalFloatingToolbar` are designed for edge-to-edge layouts.
- Wrap everything in your `MaterialTheme` — expressive shape morphing and motion depend on the theme's `shapes` and `motionScheme`.

---

## Navigation 3

Navigation 3 (`androidx.navigation3`) replaces the older `androidx.navigation:navigation-compose` for new projects. It uses a `NavBackStack` (a `SnapshotStateList<NavKey>`) instead of a `NavController`.

### Route keys

```kotlin
import androidx.navigation3.runtime.NavKey
import kotlinx.serialization.Serializable

@Serializable data object HomeRoute : NavKey
@Serializable data object DetailRoute : NavKey
@Serializable data class ItemRoute(val id: String) : NavKey  // route with args
```

- Routes implement `NavKey` (a marker interface).
- Annotate with `@Serializable` from `kotlinx.serialization` — required for state saving.

### NavDisplay setup

```kotlin
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.runtime.entry
import androidx.navigation3.ui.NavDisplay
import androidx.navigation3.ui.rememberSceneSetupNavEntryDecorator
import androidx.navigation3.runtime.rememberSavedStateNavEntryDecorator
import androidx.lifecycle.viewmodel.navigation3.rememberViewModelStoreNavEntryDecorator

@Composable
fun AppNavigation() {
    val backStack = rememberNavBackStack(HomeRoute)  // initial destination

    NavDisplay(
        backStack = backStack,
        onBack = { backStack.removeLastOrNull() },
        entryDecorators = listOf(
            rememberSceneSetupNavEntryDecorator(),
            rememberSavedStateNavEntryDecorator(),
            rememberViewModelStoreNavEntryDecorator(),
        ),
        transitionSpec = {
            slideInHorizontally(initialOffsetX = { it }) togetherWith
                slideOutHorizontally(targetOffsetX = { -it })
        },
        popTransitionSpec = {
            slideInHorizontally(initialOffsetX = { -it }) togetherWith
                slideOutHorizontally(targetOffsetX = { it })
        },
        predictivePopTransitionSpec = {
            slideInHorizontally(initialOffsetX = { -it }) togetherWith
                slideOutHorizontally(targetOffsetX = { it })
        },
        entryProvider = entryProvider {
            entry<HomeRoute> { HomeScreen(onNavigateToDetail = { backStack.add(DetailRoute) }) }
            entry<DetailRoute> { DetailScreen(onBack = { backStack.removeLastOrNull() }) }
            entry<ItemRoute> { key -> ItemScreen(id = key.id) }
        },
    )
}
```

- **Navigate forward**: `backStack.add(SomeRoute)`
- **Navigate back**: `backStack.removeLastOrNull()`
- **Replace current**: `backStack.removeLastOrNull(); backStack.add(NewRoute)`
- Decorators order matters: scene setup → saved state → viewmodel store.

---

## Scaffold

`Scaffold` handles insets, provides `innerPadding`, and slots for `topBar`, `bottomBar`, `floatingActionButton`, and `snackbarHost`.

```kotlin
Scaffold(
    topBar = {
        TopAppBar(title = { Text("My Screen") })
    },
    bottomBar = {
        // NavigationBar, BottomAppBar, etc.
    },
    floatingActionButton = {
        FloatingActionButton(onClick = { }) {
            Icon(Icons.Filled.Add, contentDescription = null)
        }
    },
    floatingActionButtonPosition = FabPosition.End,
) { innerPadding ->
    LazyColumn(modifier = Modifier.padding(innerPadding)) {
        // content
    }
}
```

- Always consume `innerPadding` via `Modifier.padding(innerPadding)` or pass as `contentPadding` to lazy lists — otherwise content is obscured by bars.
- `FabPosition.End`, `FabPosition.Center`, `FabPosition.EndOverlay` (overlaps the bottom bar) are available.

---

## Simple Tab Navigation (NavigationBar)

For apps with 2–3 top-level destinations, `NavigationBar` + state is simpler than Navigation 3.

```kotlin
var selectedTab by remember { mutableIntStateOf(0) }
val tabs = listOf("Queue", "Backlog")

Scaffold(
    topBar = {
        LargeTopAppBar(
            title = { Text(tabs[selectedTab]) },
            scrollBehavior = scrollBehavior,
        )
    },
    bottomBar = {
        NavigationBar {
            NavigationBarItem(
                selected = selectedTab == 0,
                onClick = { selectedTab = 0 },
                icon = {
                    Icon(
                        if (selectedTab == 0) Icons.Filled.FormatListBulleted
                        else Icons.Outlined.FormatListBulleted,
                        contentDescription = "Queue",
                    )
                },
                label = { Text("Queue") },
            )
            NavigationBarItem(
                selected = selectedTab == 1,
                onClick = { selectedTab = 1 },
                icon = {
                    Icon(
                        if (selectedTab == 1) Icons.Filled.Inbox
                        else Icons.Outlined.Inbox,
                        contentDescription = "Backlog",
                    )
                },
                label = { Text("Backlog") },
            )
        }
    },
    floatingActionButton = {
        FloatingActionButton(
            onClick = onAddClick,
            shape = FloatingActionButtonDefaults.shape,
        ) {
            Icon(Icons.Filled.Add, contentDescription = "Add")
        }
    },
) { innerPadding ->
    when (selectedTab) {
        0 -> QueueScreen(contentPadding = innerPadding)
        1 -> BacklogScreen(contentPadding = innerPadding)
    }
}
```

- Use filled icon for selected, outlined for unselected — standard M3 convention.
- Use `mutableIntStateOf` (not `mutableStateOf<Int>`) for integer-backed state — more efficient.
- The `LargeTopAppBar` title can reflect the selected tab name.
- For scroll-collapse to work, connect `scrollBehavior` with `Modifier.nestedScroll(scrollBehavior.nestedScrollConnection)` on the `Scaffold`.

---

## Buttons

All expressive button and icon button types accept a `shapes` parameter that enables shape morphing on press. Always pass this for the expressive interaction feel.

### Standard buttons

```kotlin
import androidx.compose.material3.ButtonDefaults

// Filled (default)
Button(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Action") }

// Elevated
ElevatedButton(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Elevated") }

// Tonal
FilledTonalButton(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Tonal") }

// Outlined
OutlinedButton(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Outlined") }

// Text
TextButton(onClick = {}, shapes = ButtonDefaults.shapes()) { Text("Text") }
```

### Toggle button

`ElevatedToggleButton` maintains elevation state and takes `checked`/`onCheckedChange` directly:

```kotlin
var checked by remember { mutableStateOf(false) }

ElevatedToggleButton(
    checked = checked,
    onCheckedChange = { checked = it },
) {
    Text(if (checked) "On" else "Off")
}

// Also available:
ToggleButton(checked = checked, onCheckedChange = { checked = it }, shapes = ButtonDefaults.shapes()) { Text("Toggle") }
FilledTonalToggleButton(checked = checked, onCheckedChange = { checked = it }) { Text("Tonal") }
OutlinedToggleButton(checked = checked, onCheckedChange = { checked = it }) { Text("Outlined") }
```

### Icon buttons

```kotlin
import androidx.compose.material3.IconButtonDefaults

FilledIconButton(onClick = {}, shapes = IconButtonDefaults.shapes()) {
    Icon(Icons.Filled.Add, contentDescription = null)
}

FilledTonalIconButton(onClick = {}, shapes = IconButtonDefaults.shapes()) {
    Icon(Icons.Filled.Edit, contentDescription = null)
}

OutlinedIconButton(onClick = {}, shapes = IconButtonDefaults.shapes()) {
    Icon(Icons.Filled.Search, contentDescription = null)
}

// Toggle variants
FilledIconToggleButton(
    checked = checked,
    onCheckedChange = { checked = it },
    shapes = IconButtonDefaults.toggleableShapes(),
) {
    if (checked) Icon(Icons.Filled.Favorite, contentDescription = null)
    else Icon(Icons.Outlined.FavoriteBorder, contentDescription = null)
}

FilledTonalIconToggleButton(
    checked = checked,
    onCheckedChange = { checked = it },
    shapes = IconButtonDefaults.toggleableShapes(),
) { /* content */ }
```

---

## SplitButtonLayout

A two-part button: the leading part triggers a primary action, the trailing part (usually a chevron) toggles an expanded state (e.g., opens a `DropdownMenu`).

```kotlin
import androidx.compose.material3.SplitButtonLayout
import androidx.compose.material3.SplitButtonDefaults
import androidx.compose.material3.ButtonDefaults

var expanded by remember { mutableStateOf(false) }

Box {
    SplitButtonLayout(
        leadingButton = {
            SplitButtonDefaults.LeadingButton(onClick = { /* primary action */ }) {
                Icon(
                    Icons.Filled.Edit,
                    modifier = Modifier.size(SplitButtonDefaults.LeadingIconSize),
                    contentDescription = null,
                )
                Spacer(Modifier.size(ButtonDefaults.IconSpacing))
                Text("Edit")
            }
        },
        trailingButton = {
            SplitButtonDefaults.TrailingButton(
                checked = expanded,
                onCheckedChange = { expanded = it },
                modifier = Modifier.semantics {
                    stateDescription = if (expanded) "Expanded" else "Collapsed"
                },
            ) {
                val rotation by animateFloatAsState(
                    targetValue = if (expanded) 180f else 0f,
                    label = "chevron_rotation",
                )
                Icon(
                    Icons.Filled.KeyboardArrowDown,
                    modifier = Modifier
                        .size(SplitButtonDefaults.TrailingIconSize)
                        .graphicsLayer { rotationZ = rotation },
                    contentDescription = null,
                )
            }
        },
    )

    DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
        DropdownMenuItem(text = { Text("Option 1") }, onClick = { expanded = false })
        DropdownMenuItem(text = { Text("Option 2") }, onClick = { expanded = false })
    }
}
```

- Accessibility: set `stateDescription` on the trailing button to announce "Expanded"/"Collapsed".
- `SplitButtonDefaults.LeadingIconSize` and `TrailingIconSize` provide the correct icon sizes.

---

## ButtonGroup

A row of buttons that hides overflow items into a menu when they don't fit.

```kotlin
import androidx.compose.material3.ButtonGroup

// Basic: overflow with icon button
ButtonGroup(
    overflowIndicator = { menuState ->
        FilledIconButton(
            onClick = {
                if (menuState.isExpanded) menuState.dismiss() else menuState.show()
            },
        ) {
            Icon(Icons.Filled.MoreVert, contentDescription = null)
        }
    },
) {
    clickableItem(onClick = { }, label = "First")
    clickableItem(onClick = { }, label = "Second")
    clickableItem(onClick = { }, label = "Third")
}
```

### Connected ButtonGroup (radio-style)

A visually connected group of toggle buttons acting as a single-select control. Uses `ButtonGroupDefaults.connectedLeadingButtonShapes()`, `connectedMiddleButtonShapes()`, and `connectedTrailingButtonShapes()` to produce rounded corners only at the group ends:

```kotlin
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ToggleButton

var selectedIndex by remember { mutableIntStateOf(0) }
val options = listOf("Day", "Week", "Month")

Row(
    horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween),
) {
    options.forEachIndexed { index, label ->
        ToggleButton(
            checked = selectedIndex == index,
            onCheckedChange = { selectedIndex = index },
            modifier = Modifier
                .weight(1f)
                .semantics { role = Role.RadioButton },
            shapes = when (index) {
                0                   -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                options.lastIndex   -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                else                -> ButtonGroupDefaults.connectedMiddleButtonShapes()
            },
        ) {
            Text(label)
        }
    }
}
```

---

## FloatingActionButton Menu

An expandable FAB that morphs between Add and Close icons as `checkedProgress` animates 0→1. Use when you need multiple quick actions from one FAB.

```kotlin
import androidx.compose.material3.FloatingActionButtonMenu
import androidx.compose.material3.FloatingActionButtonMenuItem
import androidx.compose.material3.ToggleFloatingActionButton
import androidx.compose.material3.ToggleFloatingActionButtonDefaults.animateIcon
import androidx.compose.material3.animateFloatingActionButton

var fabMenuExpanded by rememberSaveable { mutableStateOf(false) }
val fabVisible by remember { derivedStateOf { listState.firstVisibleItemIndex == 0 } }

BackHandler(fabMenuExpanded) { fabMenuExpanded = false }

Box(modifier = Modifier.fillMaxSize()) {
    // ... scrollable content ...

    FloatingActionButtonMenu(
        modifier = Modifier.align(Alignment.BottomEnd),
        expanded = fabMenuExpanded,
        button = {
            ToggleFloatingActionButton(
                modifier = Modifier
                    .semantics {
                        traversalIndex = -1f
                        stateDescription = if (fabMenuExpanded) "Expanded" else "Collapsed"
                    }
                    .animateFloatingActionButton(
                        visible = fabVisible || fabMenuExpanded,
                        alignment = Alignment.BottomEnd,
                    ),
                checked = fabMenuExpanded,
                onCheckedChange = { fabMenuExpanded = !fabMenuExpanded },
            ) {
                // checkedProgress is a scope property (0f → 1f), not a parameter
                val imageVector by remember {
                    derivedStateOf {
                        if (checkedProgress > 0.5f) Icons.Filled.Close else Icons.Filled.Add
                    }
                }
                Icon(
                    painter = rememberVectorPainter(imageVector),
                    contentDescription = null,
                    modifier = Modifier.animateIcon({ checkedProgress }),
                )
            }
        },
    ) {
        val items = listOf(
            Icons.Filled.Archive to "Archive",
            Icons.Filled.People  to "Share",
            Icons.AutoMirrored.Filled.Message to "Reply",
        )
        items.forEachIndexed { i, (icon, label) ->
            FloatingActionButtonMenuItem(
                modifier = Modifier.semantics {
                    isTraversalGroup = true
                    if (i == items.lastIndex) {
                        customActions = listOf(
                            CustomAccessibilityAction("Close menu") {
                                fabMenuExpanded = false; true
                            }
                        )
                    }
                },
                onClick = { fabMenuExpanded = false },
                icon = { Icon(icon, contentDescription = null) },
                text = { Text(label) },
            )
        }
    }
}
```

Key rules:
- `checkedProgress` is a scope property inside `ToggleFloatingActionButton`'s content lambda — access it directly, it is NOT a parameter.
- `animateIcon` and `animateFloatingActionButton` are extension functions from `androidx.compose.material3`.
- Register `BackHandler` to close the menu on back press.
- Set `traversalIndex = -1f` so the toggle button comes before menu items in accessibility traversal.
- Add `customActions` on the last menu item so screen readers can close the menu.

### LargeExtendedFloatingActionButton

Collapses to icon-only when scrolled past the first item:

```kotlin
val listState = rememberLazyListState()
val expandedFab by remember { derivedStateOf { listState.firstVisibleItemIndex == 0 } }

Scaffold(
    floatingActionButton = {
        LargeExtendedFloatingActionButton(
            onClick = { },
            expanded = expandedFab,
            icon = {
                Icon(
                    Icons.Filled.Add,
                    contentDescription = null,
                    modifier = Modifier.size(FloatingActionButtonDefaults.LargeIconSize),
                )
            },
            text = { Text("Create New") },
        )
    },
    floatingActionButtonPosition = FabPosition.End,
) { innerPadding ->
    LazyColumn(state = listState, modifier = Modifier.padding(innerPadding)) { /* ... */ }
}
```

---

## HorizontalFloatingToolbar

Floats above content and optionally hides on scroll down, returns on scroll up. Replaces a traditional `TopAppBar` for an immersive layout.

```kotlin
import androidx.compose.material3.HorizontalFloatingToolbar
import androidx.compose.material3.FloatingToolbarDefaults
import androidx.compose.material3.FloatingToolbarDefaults.ScreenOffset
import androidx.compose.material3.FloatingToolbarExitDirection.Companion.Bottom
import androidx.compose.material3.AppBarRow

// Static (always visible)
Scaffold { innerPadding ->
    Box(Modifier.padding(innerPadding)) {
        LazyColumn(contentPadding = PaddingValues(bottom = 96.dp)) { /* ... */ }

        HorizontalFloatingToolbar(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .offset(y = -ScreenOffset),  // ScreenOffset = FloatingToolbarDefaults.ScreenOffset
            expanded = true,
            leadingContent = { /* optional leading icons */ },
            trailingContent = {
                AppBarRow(
                    overflowIndicator = { menuState ->
                        IconButton(onClick = {
                            if (menuState.isExpanded) menuState.dismiss() else menuState.show()
                        }) {
                            Icon(Icons.Filled.MoreVert, contentDescription = null)
                        }
                    },
                ) {
                    clickableItem(onClick = { }, icon = { Icon(Icons.Filled.Download, null) }, label = "Download")
                    clickableItem(onClick = { }, icon = { Icon(Icons.Filled.Favorite, null) }, label = "Favorite")
                    clickableItem(onClick = { }, icon = { Icon(Icons.Filled.Add, null) }, label = "Add")
                }
            },
            content = {
                FilledIconButton(modifier = Modifier.width(64.dp), onClick = { }) {
                    Icon(Icons.Filled.Add, contentDescription = null)
                }
            },
        )
    }
}

// Scroll-away: hides when scrolling down, returns when scrolling up
val exitScrollBehavior = FloatingToolbarDefaults.exitAlwaysScrollBehavior(exitDirection = Bottom)

Scaffold(modifier = Modifier.nestedScroll(exitScrollBehavior)) { innerPadding ->
    Box(Modifier.padding(innerPadding)) {
        LazyColumn(contentPadding = PaddingValues(bottom = 96.dp)) { /* ... */ }

        HorizontalFloatingToolbar(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .offset(y = -ScreenOffset),
            expanded = true,
            scrollBehavior = exitScrollBehavior,
            content = { /* primary action */ },
        )
    }
}
```

- The `LazyColumn` (or any scrollable content) must have enough bottom `contentPadding` (≥ 96.dp) so content isn't hidden behind the toolbar.
- Connect scroll to toolbar via `Modifier.nestedScroll(exitScrollBehavior)` on the `Scaffold` itself.
- `ScreenOffset` is `FloatingToolbarDefaults.ScreenOffset` — the standard distance from the screen edge.
- `exitDirection = Bottom` makes it exit downward when scrolling down; `Top` exits upward.

### VerticalFloatingToolbar

Same concept, oriented vertically, anchored to the side edge:

```kotlin
import androidx.compose.material3.VerticalFloatingToolbar
import androidx.compose.material3.AppBarColumn

VerticalFloatingToolbar(
    modifier = Modifier
        .align(Alignment.CenterEnd)
        .offset(x = -ScreenOffset),
    expanded = true,
    trailingContent = {
        AppBarColumn(  // AppBarColumn instead of AppBarRow
            overflowIndicator = { menuState ->
                IconButton(onClick = {
                    if (menuState.isExpanded) menuState.dismiss() else menuState.show()
                }) {
                    Icon(Icons.Filled.MoreVert, contentDescription = null)
                }
            },
        ) {
            clickableItem(onClick = { }, icon = { Icon(Icons.Filled.Download, null) }, label = "Download")
            clickableItem(onClick = { }, icon = { Icon(Icons.Filled.Favorite, null) }, label = "Favorite")
        }
    },
    content = {
        FilledIconButton(modifier = Modifier.height(64.dp), onClick = { }) {
            Icon(Icons.Filled.Add, contentDescription = null)
        }
    },
)
```

- Use `AppBarColumn` (not `AppBarRow`) for the overflow items inside a `VerticalFloatingToolbar`.
- Anchor with `Alignment.CenterEnd` and `offset(x = -ScreenOffset)` for the standard right-side position.

---

## FlexibleBottomAppBar

An expressive replacement for `BottomAppBar` with support for `AppBarRow` overflow:

```kotlin
import androidx.compose.material3.FlexibleBottomAppBar
import androidx.compose.material3.BottomAppBarDefaults

FlexibleBottomAppBar(
    contentPadding = PaddingValues(horizontal = 16.dp),
    horizontalArrangement = BottomAppBarDefaults.FlexibleFixedHorizontalArrangement,
) {
    AppBarRow(
        overflowIndicator = { menuState ->
            IconButton(onClick = {
                if (menuState.isExpanded) menuState.dismiss() else menuState.show()
            }) {
                Icon(Icons.Filled.MoreVert, contentDescription = null)
            }
        },
    ) {
        clickableItem(onClick = { }, icon = { Icon(Icons.Filled.Home, null) }, label = "Home")
        clickableItem(onClick = { }, icon = { Icon(Icons.Filled.Search, null) }, label = "Search")
        clickableItem(onClick = { }, icon = { Icon(Icons.Filled.Add, null) }, label = "Add")
        clickableItem(onClick = { }, icon = { Icon(Icons.Filled.Favorite, null) }, label = "Favorite")
    }
}
```

### Standard BottomAppBar with animated hide-on-scroll

When not using `FlexibleBottomAppBar`, the standard pattern hides the bar on scroll down:

```kotlin
val scrollState = rememberLazyListState()
val isScrollingUp = /* see helper below */
val bottomBarVisible = !scrollState.isScrollInProgress || isScrollingUp

Scaffold(
    bottomBar = {
        AnimatedVisibility(
            visible = bottomBarVisible,
            enter = slideInVertically(initialOffsetY = { it }),
            exit = slideOutVertically(targetOffsetY = { it }),
        ) {
            BottomAppBar(
                actions = {
                    IconButton(onClick = { }) { Icon(Icons.Filled.Home, "Home") }
                    IconButton(onClick = { }) { Icon(Icons.Filled.Search, "Search") }
                },
                floatingActionButton = {
                    FloatingActionButton(onClick = { }) {
                        Icon(Icons.Filled.Add, contentDescription = null)
                    }
                },
            )
        }
    },
) { innerPadding ->
    LazyColumn(state = scrollState, modifier = Modifier.padding(innerPadding)) { /* ... */ }
}

// Helper extension
@Composable
fun LazyListState.isScrollingUp(): Boolean {
    var previousIndex by remember(this) { mutableStateOf(firstVisibleItemIndex) }
    var previousOffset by remember(this) { mutableStateOf(firstVisibleItemScrollOffset) }
    return remember(this) {
        derivedStateOf {
            val scrollingUp = if (previousIndex != firstVisibleItemIndex) {
                firstVisibleItemIndex < previousIndex
            } else {
                firstVisibleItemScrollOffset < previousOffset
            }
            previousIndex = firstVisibleItemIndex
            previousOffset = firstVisibleItemScrollOffset
            scrollingUp
        }
    }.value
}
```

---

## WideNavigationRail

An expressive side navigation component that expands/collapses between icon-only and icon+label modes. Use on medium/expanded window size classes.

```kotlin
import androidx.compose.material3.WideNavigationRail
import androidx.compose.material3.WideNavigationRailItem
import androidx.compose.material3.WideNavigationRailValue
import androidx.compose.material3.rememberWideNavigationRailState

var selectedItem by remember { mutableIntStateOf(0) }
val items = listOf("Home", "Search", "Settings")
val selectedIcons = listOf(Icons.Filled.Home, Icons.Filled.Favorite, Icons.Filled.Star)
val unselectedIcons = listOf(Icons.Outlined.Home, Icons.Outlined.FavoriteBorder, Icons.Outlined.StarBorder)
val state = rememberWideNavigationRailState()
val scope = rememberCoroutineScope()

Row {
    WideNavigationRail(
        state = state,
        header = {
            IconButton(
                modifier = Modifier
                    .padding(start = 24.dp)
                    .semantics {
                        stateDescription = if (state.currentValue == WideNavigationRailValue.Expanded)
                            "Expanded" else "Collapsed"
                    },
                onClick = {
                    scope.launch {
                        if (state.targetValue == WideNavigationRailValue.Expanded)
                            state.collapse()
                        else
                            state.expand()
                    }
                },
            ) {
                if (state.targetValue == WideNavigationRailValue.Expanded)
                    Icon(Icons.AutoMirrored.Filled.MenuOpen, "Collapse")
                else
                    Icon(Icons.Filled.Menu, "Expand")
            }
        },
    ) {
        items.forEachIndexed { index, label ->
            WideNavigationRailItem(
                railExpanded = state.targetValue == WideNavigationRailValue.Expanded,
                selected = selectedItem == index,
                onClick = { selectedItem = index },
                icon = {
                    Icon(
                        if (selectedItem == index) selectedIcons[index] else unselectedIcons[index],
                        contentDescription = label,
                    )
                },
                label = { Text(label) },
            )
        }
    }

    // Main content area
    Box(Modifier.weight(1f)) { /* screen content */ }
}
```

### ModalWideNavigationRail

Same API as `WideNavigationRail` but renders as a modal drawer overlay when expanded:

```kotlin
ModalWideNavigationRail(
    state = state,
    expandedHeaderTopPadding = 64.dp,  // align with app bar
    header = { /* toggle button */ },
) {
    // WideNavigationRailItem entries
}
```

- Use `ModalWideNavigationRail` when the rail should overlay content rather than push it aside.
- `expandedHeaderTopPadding` controls the top padding of the header in expanded mode — set it to match your `TopAppBar` height for visual alignment.
- Accessibility: always set `stateDescription` on the toggle button to announce "Expanded"/"Collapsed".

---

## Progress Indicators

### Wavy progress indicators (Expressive)

```kotlin
import androidx.compose.material3.CircularWavyProgressIndicator
import androidx.compose.material3.LinearWavyProgressIndicator

// Indeterminate
CircularWavyProgressIndicator()
LinearWavyProgressIndicator()

// Determinate
val animatedProgress by animateFloatAsState(
    targetValue = progress,
    animationSpec = ProgressIndicatorDefaults.ProgressAnimationSpec,
)
CircularWavyProgressIndicator(progress = { animatedProgress })
LinearWavyProgressIndicator(progress = { animatedProgress })

// Custom stroke width
val thickStroke = remember {
    Stroke(width = with(density) { 8.dp.toPx() }, cap = StrokeCap.Round)
}
CircularWavyProgressIndicator(
    progress = { animatedProgress },
    modifier = Modifier.size(52.dp),
    stroke = thickStroke,
    trackStroke = thickStroke,
)
```

### Loading indicators (Expressive)

Organic animated dot indicators that replace `CircularProgressIndicator` for loading states:

```kotlin
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.ContainedLoadingIndicator

LoadingIndicator()           // floating dots, indeterminate

ContainedLoadingIndicator()  // contained, indeterminate

// Determinate contained loading
val animatedProgress by animateFloatAsState(
    targetValue = progress,
    animationSpec = spring(
        dampingRatio = Spring.DampingRatioNoBouncy,
        stiffness = Spring.StiffnessVeryLow,
        visibilityThreshold = 1 / 1000f,
    ),
)
ContainedLoadingIndicator(progress = { animatedProgress })
```

### Pull to refresh

`PullToRefreshBox` is the simpler wrapper API (preferred over the Modifier approach):

```kotlin
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState

val pullState = rememberPullToRefreshState()

PullToRefreshBox(
    isRefreshing = uiState is UiState.Loading,
    onRefresh = viewModel::loadData,
    state = pullState,
    modifier = Modifier.fillMaxSize(),
) {
    LazyColumn(contentPadding = innerPadding, modifier = Modifier.fillMaxSize()) {
        items(items, key = { it.id }) { item ->
            MyListItem(item)
        }
    }
}
```

- `isRefreshing` drives the spinner — tie it to your loading state.
- `onRefresh` is called when the user pulls far enough — call your ViewModel's load function.
- The `PullToRefreshBox` replaces the outer content container; put the `LazyColumn` inside it.
- `rememberPullToRefreshState()` manages the pull distance animation.

---

## TopAppBar

```kotlin
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.MediumTopAppBar
import androidx.compose.material3.TopAppBarDefaults

// Basic
TopAppBar(
    title = { Text("Title") },
    navigationIcon = {
        IconButton(onClick = onBack) {
            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
        }
    },
    actions = {
        IconButton(onClick = { }) { Icon(Icons.Filled.MoreVert, contentDescription = null) }
    },
)

// Collapsing (scrolls away with content)
val scrollBehavior = TopAppBarDefaults.enterAlwaysScrollBehavior()

Scaffold(
    modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
    topBar = {
        LargeTopAppBar(
            title = { Text("Large Title") },
            scrollBehavior = scrollBehavior,
        )
    },
) { innerPadding ->
    LazyColumn(modifier = Modifier.padding(innerPadding)) { /* ... */ }
}
```

- `enterAlwaysScrollBehavior()` — hides on scroll down, returns on scroll up.
- `exitUntilCollapsedScrollBehavior()` — collapses the large title to small on scroll.
- `pinnedScrollBehavior()` — always visible, no animation.

---

## ModalBottomSheet

Use for contextual actions, creation forms, and detail views that slide up from the bottom.

```kotlin
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CreateActionSheet(
    onDismiss: () -> Unit,
    onConfirm: (String) -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartialExpansion = true)
    var text by remember { mutableStateOf("") }
    val focusRequester = remember { FocusRequester() }

    // Auto-focus the text field when the sheet opens
    LaunchedEffect(Unit) { focusRequester.requestFocus() }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp)
                .navigationBarsPadding()   // avoid navigation bar overlap
                .imePadding(),             // push content above keyboard
        ) {
            Text("New action", style = MaterialTheme.typography.titleLarge)
            Spacer(Modifier.height(16.dp))

            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                placeholder = { Text("What do you want to do?") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Sentences,
                    imeAction = ImeAction.Done,
                ),
                keyboardActions = KeyboardActions(
                    onDone = { if (text.isNotBlank()) onConfirm(text) },
                ),
                modifier = Modifier
                    .fillMaxWidth()
                    .focusRequester(focusRequester),
                shape = MaterialTheme.shapes.large,
            )

            Spacer(Modifier.height(16.dp))
            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                modifier = Modifier.fillMaxWidth(),
            ) {
                TextButton(onClick = onDismiss) { Text("Cancel") }
                Button(
                    onClick = { onConfirm(text) },
                    enabled = text.isNotBlank(),
                    shapes = ButtonDefaults.shapes(),
                ) { Text("Add to backlog") }
            }
            Spacer(Modifier.height(8.dp))
        }
    }
}
```

Show/hide from the parent composable using a `StateFlow<Boolean>` in the ViewModel:

```kotlin
// In Scaffold content:
val showSheet by viewModel.showCreateSheet.collectAsStateWithLifecycle()

if (showSheet) {
    CreateActionSheet(
        onDismiss = viewModel::closeCreateSheet,
        onConfirm = viewModel::createAction,
    )
}
```

Key rules:
- `skipPartialExpansion = true` — sheet opens fully expanded, skipping the half-height stop.
- `navigationBarsPadding()` + `imePadding()` — essential for edge-to-edge; prevents content going under the navigation bar or keyboard.
- `LaunchedEffect(Unit) { focusRequester.requestFocus() }` — auto-opens the keyboard when the sheet appears.
- Both `onDone` keyboard action and the button should call `onConfirm` — users expect both to work.
- `shapes = ButtonDefaults.shapes()` on the confirm `Button` enables Expressive shape morphing.

---

## LazyColumn / LazyRow

Standard Compose lazy lists. Unlike Wear OS there is no required transformation modifier — use them directly.

```kotlin
val listState = rememberLazyListState()

LazyColumn(
    state = listState,
    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
    verticalArrangement = Arrangement.spacedBy(8.dp),
    modifier = Modifier.fillMaxSize(),
) {
    item { HeaderComposable() }

    items(
        items = myList,
        key = { it.id },               // stable keys improve performance
        contentType = { "card" },      // group similar items for reuse
    ) { item ->
        MyCard(item = item)
    }

    item { FooterComposable() }
}
```

- Always provide `key = { it.id }` on `items` — this prevents full re-layouts when list items reorder.
- `contentType` groups items of the same type to improve view recycling.
- Prefer `derivedStateOf { }` when deriving state from `listState` to avoid recomposition on every scroll pixel.

---

## ListItem

M3's standard list row composable. Use it for any content list — it handles the M3 spacing, typography, and icon sizing automatically.

```kotlin
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.HorizontalDivider

ListItem(
    headlineContent = {
        Text(item.title, style = MaterialTheme.typography.bodyLarge)
    },
    supportingContent = {
        Text(item.subtitle, style = MaterialTheme.typography.bodySmall)
    },
    leadingContent = {
        Icon(
            imageVector = Icons.Filled.RadioButtonUnchecked,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.primary,
        )
    },
    trailingContent = {
        Checkbox(checked = false, onCheckedChange = { /* ... */ })
    },
    // Optional: tint the background for visual grouping
    colors = ListItemDefaults.colors(
        containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
    ),
)
HorizontalDivider()
```

- `headlineContent` — required. The primary label.
- `supportingContent` — optional second line (subtitle, metadata).
- `leadingContent` — optional icon/avatar on the left.
- `trailingContent` — optional widget on the right (checkbox, icon button, text).
- `colors = ListItemDefaults.colors(containerColor = ...)` — tint the background without wrapping in a `Card`.
- `HorizontalDivider()` after the item for separation (don't add for the last item, or always add and let the list clip it).

### Mixed-type lists with sealed classes

When a list contains multiple item types (e.g. queued actions + calendar events), use a sealed class and `contentType` for recycling efficiency:

```kotlin
sealed class QueueItem {
    abstract val id: String
    abstract val sortKey: String  // ISO-8601 for chronological sort

    data class ActionItem(val action: Action) : QueueItem() { ... }
    data class EventItem(val event: Event)   : QueueItem() { ... }
}

LazyColumn {
    items(
        items = queueItems,
        key = { it.id },
        contentType = { it::class.simpleName },   // enables ABI-efficient recycling
    ) { item ->
        when (item) {
            is QueueItem.ActionItem -> ActionRow(item)
            is QueueItem.EventItem  -> EventRow(item)
        }
    }
}
```

Sort mixed lists on the ViewModel side using a shared `sortKey` before passing to the composable.

---

## Theming

### MaterialTheme structure

```kotlin
@Composable
fun MyAppTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (isSystemInDarkTheme()) darkColorScheme else lightColorScheme,
        typography = MyTypography,
        shapes = MaterialTheme.shapes,   // shapes provides expressive shape morphing
        content = content,
    )
}
```

### Accessing theme tokens

```kotlin
// Colors
MaterialTheme.colorScheme.primary
MaterialTheme.colorScheme.onPrimary
MaterialTheme.colorScheme.primaryContainer
MaterialTheme.colorScheme.secondary
MaterialTheme.colorScheme.tertiary
MaterialTheme.colorScheme.background
MaterialTheme.colorScheme.surface
MaterialTheme.colorScheme.surfaceVariant
MaterialTheme.colorScheme.error
MaterialTheme.colorScheme.outline

// Typography
MaterialTheme.typography.displayLarge
MaterialTheme.typography.headlineMedium
MaterialTheme.typography.titleLarge
MaterialTheme.typography.bodyLarge
MaterialTheme.typography.bodyMedium
MaterialTheme.typography.labelSmall

// Shapes
MaterialTheme.shapes.small    // 4.dp corner
MaterialTheme.shapes.medium   // 12.dp corner
MaterialTheme.shapes.large    // 16.dp corner
MaterialTheme.shapes.extraLarge  // 28.dp corner
```

```kotlin
// Surface container variants (use for subtle layering without elevation)
MaterialTheme.colorScheme.surfaceContainerLowest  // most transparent
MaterialTheme.colorScheme.surfaceContainerLow
MaterialTheme.colorScheme.surfaceContainer
MaterialTheme.colorScheme.surfaceContainerHigh
MaterialTheme.colorScheme.surfaceContainerHighest
```

Use `surfaceContainerLow` for cards or list items that need subtle differentiation from the background without a drop shadow. Use `surfaceContainerHigh` for modal elements like bottom sheets and dialogs.

### Dynamic color (Material You)

```kotlin
val colorScheme = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
    val context = LocalContext.current
    if (isSystemInDarkTheme()) dynamicDarkColorScheme(context)
    else dynamicLightColorScheme(context)
} else {
    if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()
}
```

---

## Accessibility

- **`stateDescription`** — always set on toggle controls (`ToggleFloatingActionButton`, `SplitButtonDefaults.TrailingButton`, `WideNavigationRail` toggle) to announce "Expanded"/"Collapsed".
- **`traversalIndex`** — set to `-1f` on the `ToggleFloatingActionButton` within a `FloatingActionButtonMenu` so screen readers navigate to it before the menu items.
- **`isTraversalGroup = true`** — set on each `FloatingActionButtonMenuItem` to group its icon and label as a single traversal unit.
- **`customActions`** — add a "Close menu" action on the last `FloatingActionButtonMenuItem` so screen readers can close the menu without backtracking.
- **`Role.RadioButton`** — set via `semantics { role = Role.RadioButton }` on connected `ToggleButton` items in a single-select `ButtonGroup`.
- **`contentDescription`** on every `Icon` — use `null` only when the icon is purely decorative and the parent element has its own label.

---

## Gradle / Build config notes

- Use `kotlin { compilerOptions { } }` (not the deprecated `kotlinOptions { }`) when using only `alias(libs.plugins.kotlin.compose)` without `kotlin.android`.
- Use `jvmTarget.set(JvmTarget.JVM_11)` (typed enum), not the string `"11"`.
- Use `freeCompilerArgs.addAll(...)`, not `freeCompilerArgs += listOf(...)`.
- `compose-bom-alpha` is required for expressive APIs. The stable `compose-bom` does not include them.
- `compileSdk = 36` and `targetSdk = 36` are required for the latest expressive components.
- `minSdk = 31` is a practical minimum for expressive features (dynamic color requires API 31).