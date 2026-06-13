package com.example.subroutine_simple.data.models

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

@Serializable
data class Action(
    val id: String,
    @SerialName("lineage_id") val lineageId: String,
    @SerialName("routine_id") val routineId: String? = null,
    @SerialName("template_id") val templateId: String? = null,
    val title: String,
    val content: String? = null,
    val duration: Long? = null,
    val recurrence: JsonElement? = null,
    val state: JsonElement,
)

val Action.isQueued: Boolean
    get() {
        val obj = state as? JsonObject ?: return false
        return (obj["type"] as? JsonPrimitive)?.content == "queued"
    }

val Action.isBacklogged: Boolean
    get() {
        val obj = state as? JsonObject ?: return false
        return (obj["type"] as? JsonPrimitive)?.content == "backlogged"
    }

val Action.isCompleted: Boolean
    get() {
        val obj = state as? JsonObject ?: return false
        return (obj["type"] as? JsonPrimitive)?.content == "completed"
    }

/** ISO-8601 UTC string if the action is queued, null otherwise. */
val Action.scheduledTimeIso: String?
    get() {
        val obj = state as? JsonObject ?: return null
        if ((obj["type"] as? JsonPrimitive)?.content != "queued") return null
        return (obj["time"] as? JsonPrimitive)?.content
    }

@Serializable
data class ActionTemplate(
    val id: String,
    @SerialName("lineage_id") val lineageId: String,
    val title: String,
    val content: String? = null,
    val duration: Long? = null,
    val recurrence: JsonElement? = null,
)

@Serializable
data class EventTemplate(
    val id: String,
    @SerialName("lineage_id") val lineageId: String,
    val title: String,
    val content: String? = null,
    val duration: Long,
    val recurrence: JsonElement? = null,
)

@Serializable
data class AllData(
    val actions: List<Action>,
    val events: List<Event>,
    val routines: List<Routine> = emptyList(),
    @SerialName("action_templates") val actionTemplates: List<ActionTemplate> = emptyList(),
    @SerialName("event_templates") val eventTemplates: List<EventTemplate> = emptyList(),
)

@Serializable
data class CompleteResult(
    val completed: Action,
    val next: Action? = null,
)
