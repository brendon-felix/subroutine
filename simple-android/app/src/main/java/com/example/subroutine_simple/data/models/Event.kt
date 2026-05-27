package com.example.subroutine_simple.data.models

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import java.time.Instant

@Serializable
data class Event(
    val id: String,
    @SerialName("lineage_id") val lineageId: String,
    val title: String,
    val content: String? = null,
    /** ISO-8601 UTC string, e.g. "2024-01-15T14:00:00Z" */
    val time: String,
    val duration: JsonElement? = null,
    val recurrence: JsonElement? = null,
    val saved: Boolean = false,
)

/** Duration in seconds extracted from the server's `{"secs": N, "nanos": N}` format, or null. */
val Event.durationSecs: Long?
    get() {
        val obj = duration as? JsonObject ?: return null
        return (obj["secs"] as? JsonPrimitive)?.content?.toLongOrNull()
    }

/**
 * An event is "not fully passed" when its end time is in the future.
 * End time = start + duration, or just start if there's no duration.
 */
val Event.isNotFullyPassed: Boolean
    get() = try {
        val start = Instant.parse(time)
        val end = start.plusSeconds(durationSecs ?: 0L)
        end > Instant.now()
    } catch (_: Exception) {
        false
    }
