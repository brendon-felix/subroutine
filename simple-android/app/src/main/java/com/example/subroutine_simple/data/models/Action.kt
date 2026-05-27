package com.example.subroutine_simple.data.models

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

@Serializable
data class Action(
    val id: String,
    @SerialName("lineage_id") val lineageId: String,
    @SerialName("origin_routine_id") val originRoutineId: String? = null,
    val title: String,
    val content: String? = null,
    val duration: JsonElement? = null,
    val recurrence: JsonElement? = null,
    val saved: Boolean = false,
    val state: JsonElement,
)

val Action.isQueued: Boolean
    get() = state is JsonObject && (state as JsonObject).containsKey("Queued")

val Action.isBacklogged: Boolean
    get() = state is JsonObject && (state as JsonObject).containsKey("Backlogged")

val Action.isCompleted: Boolean
    get() = state is JsonObject && (state as JsonObject).containsKey("Completed")

/** ISO-8601 UTC string if the action is queued, null otherwise. */
val Action.scheduledTimeIso: String?
    get() {
        val obj = state as? JsonObject ?: return null
        val queued = obj["Queued"] as? JsonObject ?: return null
        return (queued["time"] as? JsonPrimitive)?.content
    }

@Serializable
data class AllData(
    val actions: List<Action>,
    val events: List<Event>,
)

@Serializable
data class CompleteResult(
    val completed: Action,
    val next: Action? = null,
)
