package com.example.subroutine_simple.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.RadioButtonUnchecked
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.example.subroutine_simple.data.models.Action
import com.example.subroutine_simple.data.models.isQueued
import com.example.subroutine_simple.data.models.scheduledTimeIso

@Composable
fun ActionListItem(
    action: Action,
    isCompleting: Boolean,
    onComplete: () -> Unit,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val timeText = if (action.isQueued) {
        action.scheduledTimeIso
            ?.let { formatIsoTime(it) }
            ?: "Scheduled"
    } else {
        action.content?.take(80)
    }

    ListItem(
        headlineContent = {
            Text(
                text = action.title,
                style = MaterialTheme.typography.bodyLarge,
            )
        },
        supportingContent = timeText?.let {
            { Text(it, style = MaterialTheme.typography.bodySmall) }
        },
        leadingContent = {
            Icon(
                imageVector = Icons.Filled.RadioButtonUnchecked,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
            )
        },
        trailingContent = {
            if (isCompleting) {
                CircularProgressIndicator(modifier = Modifier.size(24.dp))
            } else {
                Checkbox(
                    checked = false,
                    onCheckedChange = { if (it) onComplete() },
                )
            }
        },
        modifier = modifier.clickable(onClick = onClick),
    )
    HorizontalDivider()
}
