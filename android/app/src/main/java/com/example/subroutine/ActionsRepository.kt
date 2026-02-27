package com.example.subroutine

import android.content.Context
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

@Serializable
data class SavedAction(
    val id: String,
    val title: String,
    val content: String? = null,
    @SerialName("target_time") val targetTime: String? = null,
    val context: ActionContext,
    val constraints: SavedConstraints,
    val recurrence: RecurrenceRule? = null,
)

@Serializable
data class ActionContext(
    @SerialName("energy_rate") val energyRate: Int? = null,
    @SerialName("attention_level") val attentionLevel: Int? = null,
    @SerialName("transition_difficulty") val transitionDifficulty: Int? = null,
    val importance: Int? = null,
)

@Serializable
data class SavedConstraints(
    @SerialName("valid_times_of_day") val validTimesOfDay: Int? = null,
    val deadline: String? = null,
    @SerialName("minimum_duration") val minimumDuration: Long? = null,
    @SerialName("transition_time") val transitionTime: Long? = null,
    @SerialName("spoons_required") val spoonsRequired: Int? = null,
    val dependencies: List<String> = emptyList(),
)

@Serializable
data class RecurrenceRule(
    @SerialName("min_interval") val minInterval: Long? = null,
    @SerialName("max_interval") val maxInterval: Long? = null,
    @SerialName("auto_reschedule") val autoReschedule: Boolean,
)

class ActionsRepository(private val appContext: Context) {

    private val json = Json { ignoreUnknownKeys = true }

    private val dbPath: String
        get() = appContext.getDatabasePath("subroutine.db").absolutePath

    fun fetchSavedActions(): List<SavedAction> {
        val raw = RustBridge.fetchSavedActions(dbPath)
        return json.decodeFromString(raw)
    }

    fun insertSavedAction(title: String, content: String?): Boolean {
        return RustBridge.insertSavedAction(dbPath, title, content ?: "")
    }

    fun deleteSavedAction(id: String): Boolean {
        return RustBridge.deleteSavedAction(dbPath, id)
    }
}
