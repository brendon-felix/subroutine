package com.example.subroutine_simple.data.models

/**
 * A unified item for the queue list, which can be either a scheduled Action or an Event.
 * Items are sorted by [sortKey] (ISO-8601), so they appear chronologically.
 */
sealed class QueueItem {
    abstract val id: String
    abstract val title: String
    abstract val content: String?
    /** ISO-8601 string used for chronological sorting. */
    abstract val sortKey: String

    data class ActionItem(val action: Action) : QueueItem() {
        override val id = action.id
        override val title = action.title
        override val content = action.content
        override val sortKey = action.scheduledTimeIso ?: ""
    }

    data class EventItem(val event: Event) : QueueItem() {
        override val id = event.id
        override val title = event.title
        override val content = event.content
        override val sortKey = event.time
    }
}
