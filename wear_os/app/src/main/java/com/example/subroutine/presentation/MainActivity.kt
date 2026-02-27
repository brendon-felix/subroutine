package com.example.subroutine.presentation

import android.os.Bundle
import android.os.Trace
import android.util.Log
import android.view.Choreographer
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.util.concurrent.atomic.AtomicLong
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.wear.compose.foundation.lazy.TransformingLazyColumn
import androidx.wear.compose.foundation.lazy.TransformingLazyColumnItemScope
import androidx.wear.compose.foundation.lazy.TransformingLazyColumnScope
import androidx.wear.compose.foundation.lazy.items
import androidx.wear.compose.foundation.lazy.rememberTransformingLazyColumnState
import androidx.wear.compose.material3.AppScaffold
import androidx.compose.foundation.clickable
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.ButtonDefaults
import androidx.wear.compose.material3.Card
import androidx.wear.compose.material3.CircularProgressIndicator
import androidx.wear.compose.material3.ListHeader
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.ScreenScaffold
import androidx.wear.compose.material3.SurfaceTransformation
import androidx.wear.compose.material3.Text
import androidx.wear.compose.material3.TimeText
import androidx.wear.compose.material3.lazy.rememberTransformationSpec
import androidx.wear.compose.material3.lazy.TransformationSpec
import androidx.wear.compose.material3.lazy.transformedHeight
import androidx.wear.compose.navigation.SwipeDismissableNavHost
import androidx.wear.compose.navigation.composable
import androidx.wear.compose.navigation.rememberSwipeDismissableNavController
import com.example.subroutine.data.PipelineEntry
import com.example.subroutine.presentation.theme.SubroutineTheme
import com.google.android.horologist.compose.layout.ColumnItemType
import com.google.android.horologist.compose.layout.rememberResponsiveColumnPadding

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        installSplashScreen()
        super.onCreate(savedInstanceState)
        setTheme(android.R.style.Theme_DeviceDefault)
        setContent {
            SubroutineTheme {
                WearApp()
            }
        }
    }
}

private object Routes {
    const val LIST = "list"
    const val DETAIL = "detail/{entryId}"
    fun detail(entryId: String) = "detail/$entryId"
}

private const val FRAME_TAG = "FrameTiming"
private const val RECOMPOSE_TAG = "RecomposeTiming"
private const val JANK_THRESHOLD_MS = 32L

// Shared counters so FrameTimingLogger can report recompose activity that occurred
// during the gap. Incremented on the main thread by RecomposeLogger.
private val listItemRecomposeCount = AtomicLong(0L)
private val screenRecomposeCount = AtomicLong(0L)

// Timestamp (System.nanoTime) of the last recompose of any list item, so the frame gap
// logger can report "last recompose was Xms before the gap" for correlation.
private val lastListItemRecomposeNanos = AtomicLong(0L)

@Composable
private fun FrameTimingLogger(tag: String) {
    val tagState = rememberUpdatedState(tag)
    DisposableEffect(Unit) {
        var lastFrameNanos = 0L
        val choreographer = Choreographer.getInstance()
        val callback = object : Choreographer.FrameCallback {
            override fun doFrame(frameTimeNanos: Long) {
                if (lastFrameNanos != 0L) {
                    val gapMs = (frameTimeNanos - lastFrameNanos) / 1_000_000L
                    if (gapMs > JANK_THRESHOLD_MS) {
                        val recomposes = listItemRecomposeCount.getAndSet(0L)
                        val screenRecomposes = screenRecomposeCount.getAndSet(0L)
                        val lastItemNanos = lastListItemRecomposeNanos.get()
                        val msSinceLastItemRecompose =
                            if (lastItemNanos > 0L) (frameTimeNanos - lastItemNanos) / 1_000_000L else -1L

                        // Categorise the gap to help narrow down the cause:
                        //   RECOMPOSE  — items recomposed during the gap window (Compose work)
                        //   GC_SUSPECT — gap >80ms with no recompose activity (matches typical
                        //                ART concurrent mark-compact pause durations)
                        //   VSYNC_MISS — moderate gap with no obvious cause
                        val category = when {
                            recomposes > 0 || screenRecomposes > 0 -> "RECOMPOSE"
                            gapMs > 80 -> "GC_SUSPECT"
                            else -> "VSYNC_MISS"
                        }

                        Log.w(
                            FRAME_TAG,
                            "${tagState.value}: [$category] gap=${gapMs}ms " +
                                    "listItemRecomposes=$recomposes " +
                                    "screenRecomposes=$screenRecomposes " +
                                    "msSinceLastItemRecompose=$msSinceLastItemRecompose",
                        )

                        // Emit a named Trace section so the gap is visible in a systrace /
                        // Perfetto capture as a slice on the main thread.
                        Trace.beginSection("JankGap_${category}_${gapMs}ms")
                        Trace.endSection()
                    }
                }
                lastFrameNanos = frameTimeNanos
                choreographer.postFrameCallback(this)
            }
        }
        choreographer.postFrameCallback(callback)
        onDispose { choreographer.removeFrameCallback(callback) }
    }
}

// Increments a shared counter and records a timestamp every time it recomposes.
// Place this inside any composable you want to track (list item, screen root, etc.).
// The counters are read and cleared by FrameTimingLogger on the next jank gap.
@Composable
private fun RecomposeLogger(tag: String, isListItem: Boolean = false) {
    val countState = remember { mutableIntStateOf(0) }
    SideEffect {
        countState.intValue++
        val total = if (isListItem) {
            lastListItemRecomposeNanos.set(System.nanoTime())
            listItemRecomposeCount.incrementAndGet()
        } else {
            screenRecomposeCount.incrementAndGet()
        }
        Log.d(RECOMPOSE_TAG, "$tag recomposed (this=${countState.intValue} total=$total)")
        Trace.beginSection("Recompose_$tag")
        Trace.endSection()
    }
}

@Composable
fun WearApp(viewModel: PipelineViewModel = viewModel()) {
    val navController = rememberSwipeDismissableNavController()

    AppScaffold(timeText = { TimeText() }) {
        SwipeDismissableNavHost(
            navController = navController,
            startDestination = Routes.LIST,
        ) {
            composable(Routes.LIST) {
                val uiState by viewModel.uiState.collectAsState()
                val onNavigateToDetail = remember(navController) {
                    { entryId: String -> navController.navigate(Routes.detail(entryId)) }
                }
                val onRefresh = remember(viewModel) { { viewModel.loadPipeline() } }
                QueueListScreen(
                    uiState = uiState,
                    onNavigateToDetail = onNavigateToDetail,
                    onRefresh = onRefresh,
                )
            }
            composable(Routes.DETAIL) { backStackEntry ->
                val entryId = backStackEntry.arguments?.getString("entryId") ?: return@composable
                val uiState by viewModel.uiState.collectAsState()
                val entry = remember(uiState, entryId) {
                    (uiState as? PipelineUiState.Success)
                        ?.pipeline?.queue
                        ?.firstOrNull { it.id == entryId }
                }
                val onComplete = remember(viewModel, entryId) {
                    {
                        viewModel.completeAction(entryId)
                        navController.popBackStack()
                        Unit
                    }
                }
                val onDemote = remember(viewModel, entryId) {
                    {
                        viewModel.demoteAction(entryId)
                        navController.popBackStack()
                        Unit
                    }
                }
                EntryDetailScreen(
                    entry = entry,
                    onComplete = onComplete,
                    onDemote = onDemote,
                )
            }
        }
    }
}

@Composable
private fun QueueListScreen(
    uiState: PipelineUiState,
    onNavigateToDetail: (entryId: String) -> Unit,
    onRefresh: () -> Unit,
) {
    val columnState = rememberTransformingLazyColumnState()
    val transformationSpec = rememberTransformationSpec()

    FrameTimingLogger(tag = "QueueListScreen")
    RecomposeLogger(tag = "QueueListScreen", isListItem = false)

    val hasItems = uiState is PipelineUiState.Success && uiState.pipeline.queue.isNotEmpty()
    val contentPadding = rememberResponsiveColumnPadding(
        first = if (hasItems) ColumnItemType.ListHeader else ColumnItemType.BodyText,
        last = ColumnItemType.Button,
    )

    ScreenScaffold(
        scrollState = columnState,
        contentPadding = contentPadding,
    ) { contentPadding ->
        TransformingLazyColumn(
            state = columnState,
            contentPadding = contentPadding,
        ) {
            when (uiState) {
                is PipelineUiState.Loading -> {
                    item {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .transformedHeight(this, transformationSpec),
                            contentAlignment = Alignment.Center,
                        ) {
                            CircularProgressIndicator()
                        }
                    }
                }

                is PipelineUiState.NoPhone -> {
                    messageItems(
                        text = "Phone not found.\nOpen Subroutine on your phone.",
                        buttonLabel = "Retry",
                        onButton = onRefresh,
                        transformationSpec = transformationSpec,
                    )
                }

                is PipelineUiState.Error -> {
                    messageItems(
                        text = uiState.message,
                        buttonLabel = "Retry",
                        onButton = onRefresh,
                        transformationSpec = transformationSpec,
                    )
                }

                is PipelineUiState.Success -> {
                    val queue = uiState.pipeline.queue
                    if (queue.isEmpty()) {
                        messageItems(
                            text = "Queue is empty.",
                            buttonLabel = "Refresh",
                            onButton = onRefresh,
                            transformationSpec = transformationSpec,
                        )
                    } else {
                        item {
                            ListHeader(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .transformedHeight(this, transformationSpec),
                                transformation = SurfaceTransformation(transformationSpec),
                            ) {
                                Text(text = "Queue (${queue.size})")
                            }
                        }

                        items(queue, key = { it.id }) { entry ->
                            QueueListItem(
                                entry = entry,
                                transformationSpec = transformationSpec,
                                onClick = { onNavigateToDetail(entry.id) },
                            )
                        }

                        item {
                            Button(
                                onClick = onRefresh,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .transformedHeight(this, transformationSpec),
                                transformation = SurfaceTransformation(transformationSpec),
                                colors = ButtonDefaults.filledTonalButtonColors(),
                            ) {
                                Text(text = "Refresh")
                            }
                        }
                    }
                }

                // Keep the existing list visible during refresh — only show a spinner above it
                // rather than collapsing the whole list, which would flash mid-interaction.
                is PipelineUiState.Refreshing -> {
                    item {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .transformedHeight(this, transformationSpec),
                            contentAlignment = Alignment.Center,
                        ) {
                            CircularProgressIndicator()
                        }
                    }
                    val queue = uiState.pipeline.queue
                    if (queue.isNotEmpty()) {
                        items(queue, key = { it.id }) { entry ->
                            QueueListItem(
                                entry = entry,
                                transformationSpec = transformationSpec,
                                onClick = { onNavigateToDetail(entry.id) },
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun TransformingLazyColumnItemScope.QueueListItem(
    entry: PipelineEntry,
    transformationSpec: TransformationSpec,
    onClick: () -> Unit,
) {
    RecomposeLogger(tag = "QueueListItem[${entry.id.take(8)}]", isListItem = true)
    // Non-clickable Card avoids allocating MutableInteractionSource + ripple per item,
    // which was costing ~150ms per item composition on this hardware. A plain Modifier.clickable
    // on the outer modifier is sufficient for tap handling with much lower overhead.
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .transformedHeight(this, transformationSpec)
            .clickable(onClick = onClick),
        transformation = SurfaceTransformation(transformationSpec),
    ) {
        Text(
            text = entry.title,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun EntryDetailScreen(
    entry: PipelineEntry?,
    onComplete: () -> Unit,
    onDemote: () -> Unit,
) {
    val columnState = rememberTransformingLazyColumnState()
    val transformationSpec = rememberTransformationSpec()

    FrameTimingLogger(tag = "EntryDetailScreen")
    RecomposeLogger(tag = "EntryDetailScreen", isListItem = false)

    val contentPadding = rememberResponsiveColumnPadding(
        first = ColumnItemType.BodyText,
        last = ColumnItemType.Button,
    )

    ScreenScaffold(
        scrollState = columnState,
        contentPadding = contentPadding,
    ) { contentPadding ->
        TransformingLazyColumn(
            state = columnState,
            contentPadding = contentPadding,
        ) {
            if (entry == null) {
                item {
                    Text(
                        text = "Item not found.",
                        style = MaterialTheme.typography.bodyMedium,
                        textAlign = TextAlign.Center,
                        modifier = Modifier
                            .fillMaxWidth()
                            .transformedHeight(this, transformationSpec),
                    )
                }
            } else {
                item {
                    Text(
                        text = entry.title,
                        style = MaterialTheme.typography.titleMedium,
                        textAlign = TextAlign.Center,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 8.dp)
                            .transformedHeight(this, transformationSpec),
                    )
                }

                entry.action?.content?.let { content ->
                    if (content.isNotBlank()) {
                        item {
                            Text(
                                text = content,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                textAlign = TextAlign.Center,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 8.dp)
                                    .transformedHeight(this, transformationSpec),
                            )
                        }
                    }
                }

                item {
                    Button(
                        onClick = onComplete,
                        modifier = Modifier
                            .fillMaxWidth()
                            .transformedHeight(this, transformationSpec),
                        transformation = SurfaceTransformation(transformationSpec),
                        colors = ButtonDefaults.buttonColors(),
                    ) {
                        Text(text = "Done")
                    }
                }

                item {
                    Button(
                        onClick = onDemote,
                        modifier = Modifier
                            .fillMaxWidth()
                            .transformedHeight(this, transformationSpec),
                        transformation = SurfaceTransformation(transformationSpec),
                        colors = ButtonDefaults.filledTonalButtonColors(),
                    ) {
                        Text(text = "Later")
                    }
                }
            }
        }
    }
}

private fun TransformingLazyColumnScope.messageItems(
    text: String,
    buttonLabel: String,
    onButton: () -> Unit,
    transformationSpec: TransformationSpec,
) {
    item {
        Text(
            text = text,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            textAlign = TextAlign.Center,
            modifier = Modifier
                .fillMaxWidth()
                .transformedHeight(this, transformationSpec),
        )
    }
    item {
        Button(
            onClick = onButton,
            modifier = Modifier
                .fillMaxWidth()
                .transformedHeight(this, transformationSpec),
            transformation = SurfaceTransformation(transformationSpec),
            colors = ButtonDefaults.filledTonalButtonColors(),
        ) {
            Text(text = buttonLabel)
        }
    }
}
