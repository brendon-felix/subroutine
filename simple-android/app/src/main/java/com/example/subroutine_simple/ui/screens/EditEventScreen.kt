package com.example.subroutine_simple.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.subroutine_simple.data.models.Event
import com.example.subroutine_simple.data.models.QueueItem
import com.example.subroutine_simple.data.models.durationSecs
import com.example.subroutine_simple.ui.ActionsUiState
import com.example.subroutine_simple.ui.MainViewModel
import com.example.subroutine_simple.ui.components.formatIsoTime

@Composable
fun EditEventScreen(
    eventId: String,
    viewModel: MainViewModel,
    onBack: () -> Unit,
) {
    val uiState by viewModel.uiState.collectAsStateWithLifecycle()
    val saving by viewModel.saving.collectAsStateWithLifecycle()

    val event: Event? = when (val s = uiState) {
        is ActionsUiState.Success -> s.queueItems
            .filterIsInstance<QueueItem.EventItem>()
            .map { it.event }
            .firstOrNull { it.id == eventId }
        else -> null
    }

    // Only navigate away when data is fully loaded and the event genuinely
    // no longer exists (e.g. deleted by another client). Transient Loading
    // states from SSE-triggered reloads should not interrupt an active edit.
    if (uiState is ActionsUiState.Success && event == null && !saving) {
        onBack()
        return
    }
    if (event == null) return

    EditEventContent(
        event = event,
        saving = saving,
        onBack = onBack,
        onSave = { title, content -> viewModel.saveEvent(eventId, title, content, onBack) },
        onDelete = { viewModel.deleteEvent(eventId, onBack) },
    )
}

@Composable
private fun EditEventContent(
    event: Event,
    saving: Boolean,
    onBack: () -> Unit,
    onSave: (title: String, content: String?) -> Unit,
    onDelete: () -> Unit,
) {
    var title by rememberSaveable(event.id) { mutableStateOf(event.title) }
    var content by rememberSaveable(event.id) { mutableStateOf(event.content ?: "") }
    var showDeleteDialog by rememberSaveable { mutableStateOf(false) }

    val scrollBehavior = TopAppBarDefaults.pinnedScrollBehavior()

    // Time summary shown read-only (editing time not yet supported)
    val durationMins = event.durationSecs / 60
    val timeLabel = buildString {
        append(formatIsoTime(event.time))
        if (durationMins > 0) append(" · ${durationMins}min")
    }

    Scaffold(
        modifier = Modifier
            .fillMaxSize()
            .nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            TopAppBar(
                title = { Text("Edit event") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    IconButton(onClick = { showDeleteDialog = true }, enabled = !saving) {
                        Icon(
                            Icons.Filled.Delete,
                            contentDescription = "Delete event",
                            tint = MaterialTheme.colorScheme.error,
                        )
                    }
                },
                scrollBehavior = scrollBehavior,
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(innerPadding)
                .padding(horizontal = 24.dp, vertical = 16.dp)
                .imePadding(),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            // Read-only time chip
            Text(
                text = timeLabel,
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.secondary,
            )

            OutlinedTextField(
                value = title,
                onValueChange = { title = it },
                label = { Text("Title") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Sentences,
                    imeAction = ImeAction.Next,
                ),
                modifier = Modifier.fillMaxWidth(),
                shape = MaterialTheme.shapes.large,
            )

            OutlinedTextField(
                value = content,
                onValueChange = { content = it },
                label = { Text("Notes (optional)") },
                minLines = 4,
                maxLines = 8,
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Sentences,
                    imeAction = ImeAction.Default,
                ),
                modifier = Modifier.fillMaxWidth(),
                shape = MaterialTheme.shapes.large,
            )

            Spacer(modifier = Modifier.height(4.dp))
            HorizontalDivider()
            Spacer(modifier = Modifier.height(4.dp))

            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp, alignment = Alignment.End),
                modifier = Modifier.fillMaxWidth(),
            ) {
                TextButton(onClick = onBack, enabled = !saving) {
                    Text("Discard")
                }
                if (saving) {
                    CircularProgressIndicator(modifier = Modifier.align(Alignment.CenterVertically))
                } else {
                    Button(
                        onClick = { onSave(title, content.ifBlank { null }) },
                        enabled = title.isNotBlank(),
                        shapes = ButtonDefaults.shapes(),
                    ) {
                        Text("Save")
                    }
                }
            }
        }
    }

    if (showDeleteDialog) {
        AlertDialog(
            onDismissRequest = { showDeleteDialog = false },
            title = { Text("Delete event?") },
            text = { Text("\"${event.title}\" will be permanently removed.") },
            confirmButton = {
                TextButton(
                    onClick = {
                        showDeleteDialog = false
                        onDelete()
                    },
                    colors = androidx.compose.material3.ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error,
                    ),
                ) {
                    Text("Delete")
                }
            },
            dismissButton = {
                TextButton(onClick = { showDeleteDialog = false }) {
                    Text("Cancel")
                }
            },
        )
    }
}
