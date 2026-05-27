package com.example.subroutine_simple.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.RadioButtonUnchecked
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.example.subroutine_simple.data.models.QueueItem
import com.example.subroutine_simple.data.models.durationSecs
import com.example.subroutine_simple.data.models.scheduledTimeIso

@Composable
fun QueueItemRow(
    item: QueueItem,
    isCompleting: Boolean,
    onComplete: () -> Unit,
    onClickAction: (QueueItem.ActionItem) -> Unit,
    onClickEvent: (QueueItem.EventItem) -> Unit,
    modifier: Modifier = Modifier,
) {
    when (item) {
        is QueueItem.ActionItem -> ActionQueueRow(
            item = item,
            isCompleting = isCompleting,
            onComplete = onComplete,
            onClick = { onClickAction(item) },
            modifier = modifier,
        )
        is QueueItem.EventItem -> EventQueueRow(
            item = item,
            onClick = { onClickEvent(item) },
            modifier = modifier,
        )
    }
}

@Composable
private fun ActionQueueRow(
    item: QueueItem.ActionItem,
    isCompleting: Boolean,
    onComplete: () -> Unit,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val timeText = item.action.scheduledTimeIso?.let { formatIsoTime(it) } ?: "Scheduled"

    ListItem(
        headlineContent = {
            Text(item.title, style = MaterialTheme.typography.bodyLarge)
        },
        supportingContent = {
            Text(timeText, style = MaterialTheme.typography.bodySmall)
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

@Composable
private fun EventQueueRow(
    item: QueueItem.EventItem,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val event = item.event
    val startText = formatIsoTime(event.time)
    val durationMins = event.durationSecs?.let { it / 60 }
    val timeText = if (durationMins != null && durationMins > 0) {
        "$startText · ${durationMins}min"
    } else {
        startText
    }

    ListItem(
        headlineContent = {
            Text(item.title, style = MaterialTheme.typography.bodyLarge)
        },
        supportingContent = {
            Text(timeText, style = MaterialTheme.typography.bodySmall)
        },
        leadingContent = {
            Icon(
                imageVector = Icons.Filled.Event,
                contentDescription = "Event",
                tint = MaterialTheme.colorScheme.secondary,
            )
        },
        // Events are not completable — no trailing slot
        colors = ListItemDefaults.colors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
        ),
        modifier = modifier.clickable(onClick = onClick),
    )
    HorizontalDivider()
}
