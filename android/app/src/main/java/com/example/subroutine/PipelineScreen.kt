package com.example.subroutine

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.ArrowUpward
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FloatingActionButtonMenu
import androidx.compose.material3.FloatingActionButtonMenuItem
import androidx.compose.material3.FloatingToolbarDefaults
import androidx.compose.material3.FloatingToolbarDefaults.ScreenOffset
import androidx.compose.material3.FloatingToolbarExitDirection.Companion.Bottom
import androidx.compose.material3.HorizontalFloatingToolbar
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.ToggleFloatingActionButton
import androidx.compose.material3.ToggleFloatingActionButtonDefaults.animateIcon
import androidx.compose.material3.animateFloatingActionButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.isTraversalGroup
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.semantics.traversalIndex
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel

@Composable
fun PipelineScreen(
    pipelineViewModel: PipelineViewModel = viewModel(),
    actionsViewModel: ActionsViewModel = viewModel(),
) {
    val pipelineState by pipelineViewModel.uiState.collectAsState()
    val actionsState by actionsViewModel.uiState.collectAsState()

    var selectedTab by rememberSaveable { mutableIntStateOf(0) }
    var fabMenuExpanded by rememberSaveable { mutableStateOf(false) }
    var showInstantiateSheet by remember { mutableStateOf(false) }

    val listState = rememberLazyListState()
    val exitScrollBehavior = FloatingToolbarDefaults.exitAlwaysScrollBehavior(exitDirection = Bottom)
    val fabVisible by remember { derivedStateOf { listState.firstVisibleItemIndex == 0 } }

    Scaffold(
        modifier = Modifier.nestedScroll(exitScrollBehavior),
    ) { innerPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
        ) {
            Column(modifier = Modifier.fillMaxSize()) {
                PrimaryTabRow(selectedTabIndex = selectedTab) {
                    Tab(
                        selected = selectedTab == 0,
                        onClick = { selectedTab = 0 },
                        text = {
                            val count = (pipelineState as? PipelineUiState.Success)
                                ?.pipeline?.queue?.size ?: 0
                            Text("Queue${if (count > 0) " ($count)" else ""}")
                        },
                    )
                    Tab(
                        selected = selectedTab == 1,
                        onClick = { selectedTab = 1 },
                        text = {
                            val count = (pipelineState as? PipelineUiState.Success)
                                ?.pipeline?.backlog?.size ?: 0
                            Text("Backlog${if (count > 0) " ($count)" else ""}")
                        },
                    )
                }

                when (val state = pipelineState) {
                    is PipelineUiState.Loading -> {
                        Box(modifier = Modifier.fillMaxSize()) {
                            LoadingIndicator(modifier = Modifier.align(Alignment.Center))
                        }
                    }

                    is PipelineUiState.Error -> {
                        Box(modifier = Modifier.fillMaxSize()) {
                            Text(
                                text = "Error: ${state.message}",
                                color = MaterialTheme.colorScheme.error,
                                modifier = Modifier
                                    .align(Alignment.Center)
                                    .padding(16.dp)
                            )
                        }
                    }

                    is PipelineUiState.Success -> {
                        val entries = if (selectedTab == 0) {
                            state.pipeline.queue
                        } else {
                            state.pipeline.backlog
                        }

                        if (entries.isEmpty()) {
                            Box(modifier = Modifier.fillMaxSize()) {
                                Text(
                                    text = if (selectedTab == 0) {
                                        "Queue is empty.\nInstantiate a saved action to add one."
                                    } else {
                                        "Backlog is empty."
                                    },
                                    style = MaterialTheme.typography.bodyLarge,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    modifier = Modifier
                                        .align(Alignment.Center)
                                        .padding(16.dp)
                                )
                            }
                        } else {
                            LazyColumn(
                                state = listState,
                                modifier = Modifier.fillMaxSize(),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                                contentPadding = PaddingValues(
                                    start = 16.dp,
                                    end = 16.dp,
                                    top = 16.dp,
                                    bottom = 96.dp,
                                ),
                            ) {
                                items(entries, key = { it.id }) { entry ->
                                    PipelineEntryCard(
                                        entry = entry,
                                        inQueue = selectedTab == 0,
                                        onPromote = { pipelineViewModel.promote(entry.id) },
                                        onDemote = { pipelineViewModel.demote(entry.id) },
                                        onDelete = {
                                            entry.action?.id?.let { pipelineViewModel.deleteAction(it) }
                                        },
                                    )
                                }
                            }
                        }
                    }
                }
            }

            HorizontalFloatingToolbar(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .offset(y = -ScreenOffset),
                expanded = true,
                scrollBehavior = exitScrollBehavior,
                leadingContent = {},
                trailingContent = {},
                content = {
                    Text(
                        text = "Pipeline",
                        style = MaterialTheme.typography.titleMedium,
                        modifier = Modifier.padding(horizontal = 16.dp),
                    )
                },
            )

            FloatingActionButtonMenu(
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .padding(end = 16.dp, bottom = 24.dp),
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
                        val imageVector by remember {
                            derivedStateOf {
                                if (checkedProgress > 0.5f) Icons.Filled.Close else Icons.Filled.Add
                            }
                        }
                        Icon(
                            painter = rememberVectorPainter(imageVector),
                            contentDescription = if (fabMenuExpanded) "Close menu" else "Open menu",
                            modifier = Modifier.animateIcon({ checkedProgress }),
                        )
                    }
                },
            ) {
                FloatingActionButtonMenuItem(
                    modifier = Modifier.semantics {
                        isTraversalGroup = true
                        customActions = listOf(
                            CustomAccessibilityAction("Close menu") {
                                fabMenuExpanded = false; true
                            }
                        )
                    },
                    onClick = {
                        fabMenuExpanded = false
                        showInstantiateSheet = true
                    },
                    icon = { Icon(Icons.Filled.Add, contentDescription = null) },
                    text = { Text("Instantiate saved action") },
                )
            }
        }
    }

    if (showInstantiateSheet) {
        InstantiateActionSheet(
            actionsState = actionsState,
            onDismiss = { showInstantiateSheet = false },
            onInstantiate = { savedActionId ->
                pipelineViewModel.instantiateSavedAction(savedActionId)
                showInstantiateSheet = false
            },
        )
    }
}

@Composable
fun PipelineEntryCard(
    entry: PipelineEntry,
    inQueue: Boolean,
    onPromote: () -> Unit,
    onDemote: () -> Unit,
    onDelete: () -> Unit,
) {
    val isActionEntry = entry.entryType == "action"

    ElevatedCard(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize(),
        elevation = CardDefaults.elevatedCardElevation(
            defaultElevation = if (inQueue) 3.dp else 1.dp
        ),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 12.dp),
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = entry.title,
                        style = MaterialTheme.typography.titleMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    entry.action?.content?.let { content ->
                        Text(
                            text = content,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.padding(top = 2.dp),
                        )
                    }
                    if (entry.entryType != "action") {
                        Text(
                            text = entry.entryType,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.tertiary,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                    }
                }

                Spacer(modifier = Modifier.width(8.dp))

                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    if (inQueue) {
                        // Demote back to backlog
                        FilledTonalIconButton(
                            onClick = onDemote,
                            shapes = IconButtonDefaults.shapes(),
                        ) {
                            Icon(
                                Icons.Filled.ArrowDownward,
                                contentDescription = "Move to backlog",
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    } else {
                        // Promote to queue
                        FilledTonalIconButton(
                            onClick = onPromote,
                            shapes = IconButtonDefaults.shapes(),
                        ) {
                            Icon(
                                Icons.Filled.ArrowUpward,
                                contentDescription = "Move to queue",
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }

                    // Delete — only available for action entries (not routines/events)
                    if (isActionEntry) {
                        FilledIconButton(
                            onClick = onDelete,
                            shapes = IconButtonDefaults.shapes(),
                            colors = IconButtonDefaults.filledIconButtonColors(
                                containerColor = MaterialTheme.colorScheme.errorContainer,
                                contentColor = MaterialTheme.colorScheme.onErrorContainer,
                            ),
                        ) {
                            Icon(
                                Icons.Filled.Delete,
                                contentDescription = "Delete action",
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun InstantiateActionSheet(
    actionsState: ActionsUiState,
    onDismiss: () -> Unit,
    onInstantiate: (savedActionId: String) -> Unit,
) {
    androidx.compose.material3.AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Choose a saved action") },
        text = {
            when (actionsState) {
                is ActionsUiState.Loading -> {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 24.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        LoadingIndicator()
                    }
                }

                is ActionsUiState.Error -> {
                    Text(
                        text = "Could not load saved actions: ${actionsState.message}",
                        color = MaterialTheme.colorScheme.error,
                    )
                }

                is ActionsUiState.Success -> {
                    if (actionsState.actions.isEmpty()) {
                        Text(
                            text = "No saved actions yet. Create some in the Actions tab first.",
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    } else {
                        LazyColumn(
                            verticalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            items(actionsState.actions, key = { it.id }) { action ->
                                Card(
                                    onClick = { onInstantiate(action.id) },
                                    modifier = Modifier.fillMaxWidth(),
                                ) {
                                    Column(modifier = Modifier.padding(12.dp)) {
                                        Text(
                                            text = action.title,
                                            style = MaterialTheme.typography.titleSmall,
                                            maxLines = 1,
                                            overflow = TextOverflow.Ellipsis,
                                        )
                                        action.content?.let {
                                            Text(
                                                text = it,
                                                style = MaterialTheme.typography.bodySmall,
                                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                                maxLines = 1,
                                                overflow = TextOverflow.Ellipsis,
                                                modifier = Modifier.padding(top = 2.dp),
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                shapes = ButtonDefaults.shapes(),
            ) {
                Text("Cancel")
            }
        },
    )
}
