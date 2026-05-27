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
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.PlayArrow
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
import com.example.subroutine_simple.data.models.Action
import com.example.subroutine_simple.data.models.isBacklogged
import com.example.subroutine_simple.data.models.isQueued
import com.example.subroutine_simple.ui.ActionsUiState
import com.example.subroutine_simple.ui.MainViewModel

@Composable
fun EditActionScreen(
    actionId: String,
    viewModel: MainViewModel,
    onBack: () -> Unit,
) {
    val uiState by viewModel.uiState.collectAsStateWithLifecycle()
    val saving by viewModel.saving.collectAsStateWithLifecycle()

    // Resolve the action from the current loaded state. If data isn't ready yet, show nothing.
    val action: Action? = when (val s = uiState) {
        is ActionsUiState.Success -> {
            val fromQueue = s.queueItems
                .filterIsInstance<com.example.subroutine_simple.data.models.QueueItem.ActionItem>()
                .map { it.action }
                .firstOrNull { it.id == actionId }
            fromQueue ?: s.backlogged.firstOrNull { it.id == actionId }
        }
        else -> null
    }

    if (action == null && !saving) {
        // Still loading or action no longer exists — go back.
        onBack()
        return
    }
    if (action == null) return

    EditActionContent(
        action = action,
        saving = saving,
        onBack = onBack,
        onSave = { title, content -> viewModel.saveAction(actionId, title, content, onBack) },
        onQueue = { viewModel.queueEditingAction(actionId, onBack) },
        onBacklog = { viewModel.backlogEditingAction(actionId, onBack) },
    )
}

@Composable
private fun EditActionContent(
    action: Action,
    saving: Boolean,
    onBack: () -> Unit,
    onSave: (title: String, content: String?) -> Unit,
    onQueue: () -> Unit,
    onBacklog: () -> Unit,
) {
    var title by rememberSaveable(action.id) { mutableStateOf(action.title) }
    var content by rememberSaveable(action.id) { mutableStateOf(action.content ?: "") }

    val scrollBehavior = TopAppBarDefaults.pinnedScrollBehavior()

    Scaffold(
        modifier = Modifier
            .fillMaxSize()
            .nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            TopAppBar(
                title = { Text("Edit action") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
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

            // State-transition button
            if (action.isBacklogged) {
                Button(
                    onClick = onQueue,
                    enabled = !saving,
                    modifier = Modifier.fillMaxWidth(),
                    shapes = ButtonDefaults.shapes(),
                ) {
                    Icon(Icons.Filled.PlayArrow, contentDescription = null)
                    Text("Add to queue", modifier = Modifier.padding(start = 8.dp))
                }
            } else if (action.isQueued) {
                TextButton(
                    onClick = onBacklog,
                    enabled = !saving,
                    modifier = Modifier.fillMaxWidth(),
                    shapes = ButtonDefaults.shapes(),
                ) {
                    Icon(Icons.Filled.Inbox, contentDescription = null)
                    Text("Move to backlog", modifier = Modifier.padding(start = 8.dp))
                }
            }

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
}
