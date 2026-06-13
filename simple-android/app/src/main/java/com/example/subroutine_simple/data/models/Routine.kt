package com.example.subroutine_simple.data.models

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement

@Serializable
data class RoutineStep(
    val title: String,
    /** Duration in nanoseconds, nullable. */
    val duration: Long? = null,
)

@Serializable
data class Routine(
    val id: String,
    val title: String,
    val content: String? = null,
    /** ISO-8601 UTC string, e.g. "2024-01-15T14:00:00Z" */
    val target: String? = null,
    val steps: List<RoutineStep> = emptyList(),
    val recurrence: JsonElement? = null,
)
