package com.example.subroutine_simple.data.models

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import java.time.Instant

@Serializable
data class Event(
    val id: String,
    @SerialName("lineage_id") val lineageId: String,
    @SerialName("template_id") val templateId: String? = null,
    val title: String,
    val content: String? = null,
    /** ISO-8601 UTC string, e.g. "2024-01-15T14:00:00Z" */
    val time: String,
    /** Duration in nanoseconds (serialized as i64 from Rust's duration_nanos). */
    val duration: Long,
    val recurrence: JsonElement? = null,
)

/** Duration in seconds extracted from the `duration` nanosecond value. */
val Event.durationSecs: Long
    get() = duration / 1_000_000_000

/**
 * An event is "not fully passed" when its end time is in the future.
 * End time = start + duration, or just start if there's no duration.
 */
val Event.isNotFullyPassed: Boolean
    get() = try {
        val start = Instant.parse(time)
        val end = start.plusSeconds(durationSecs)
        end > Instant.now()
    } catch (_: Exception) {
        false
    }
