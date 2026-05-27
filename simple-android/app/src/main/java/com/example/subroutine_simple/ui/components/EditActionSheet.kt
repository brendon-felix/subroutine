package com.example.subroutine_simple.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import com.example.subroutine_simple.data.models.Action
import com.example.subroutine_simple.data.models.isBacklogged
import com.example.subroutine_simple.data.models.isQueued

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EditActionSheet(
    action: Action,
    isSaving: Boolean,
    onDismiss: () -> Unit,
    onSave: (title: String, content: String?) -> Unit,
    onQueue: () -> Unit,
    onBacklog: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var title by remember(action.id) { mutableStateOf(action.title) }
    var content by remember(action.id) { mutableStateOf(action.content ?: "") }
    val focusRequester = remember { FocusRequester() }

    LaunchedEffect(action.id) { focusRequester.requestFocus() }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp)
                .navigationBarsPadding()
                .imePadding(),
        ) {
            Text(
                text = "Edit action",
                style = MaterialTheme.typography.titleLarge,
            )

            Spacer(modifier = Modifier.height(16.dp))

            OutlinedTextField(
                value = title,
                onValueChange = { title = it },
                label = { Text("Title") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Sentences,
                    imeAction = ImeAction.Next,
                ),
                modifier = Modifier
                    .fillMaxWidth()
                    .focusRequester(focusRequester),
                shape = MaterialTheme.shapes.large,
            )

            Spacer(modifier = Modifier.height(12.dp))

            OutlinedTextField(
                value = content,
                onValueChange = { content = it },
                label = { Text("Notes (optional)") },
                minLines = 3,
                maxLines = 6,
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Sentences,
                    imeAction = ImeAction.Default,
                ),
                modifier = Modifier.fillMaxWidth(),
                shape = MaterialTheme.shapes.large,
            )

            Spacer(modifier = Modifier.height(16.dp))

            // State-transition row: queue ↔ backlog
            if (action.isBacklogged) {
                Button(
                    onClick = onQueue,
                    enabled = !isSaving,
                    modifier = Modifier.fillMaxWidth(),
                    shapes = ButtonDefaults.shapes(),
                ) {
                    Icon(Icons.Filled.PlayArrow, contentDescription = null)
                    Text("Add to queue", modifier = Modifier.padding(start = 8.dp))
                }
            } else if (action.isQueued) {
                TextButton(
                    onClick = onBacklog,
                    enabled = !isSaving,
                    modifier = Modifier.fillMaxWidth(),
                    shapes = ButtonDefaults.shapes(),
                ) {
                    Icon(Icons.Filled.Inbox, contentDescription = null)
                    Text("Move to backlog", modifier = Modifier.padding(start = 8.dp))
                }
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))

            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp, alignment = Alignment.End),
                modifier = Modifier.fillMaxWidth(),
            ) {
                TextButton(onClick = onDismiss, enabled = !isSaving) {
                    Text("Cancel")
                }
                Button(
                    onClick = { onSave(title, content.ifBlank { null }) },
                    enabled = title.isNotBlank() && !isSaving,
                    shapes = ButtonDefaults.shapes(),
                ) {
                    Text("Save")
                }
            }

            Spacer(modifier = Modifier.height(8.dp))
        }
    }
}
