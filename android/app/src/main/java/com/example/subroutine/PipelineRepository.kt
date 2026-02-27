package com.example.subroutine

import android.content.Context
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

@Serializable
data class ActionPayload(
    val id: String,
    val title: String,
    val content: String? = null,
    @SerialName("created_at") val createdAt: String,
    @SerialName("target_time") val targetTime: String? = null,
    val ephemeral: Boolean,
    @SerialName("saved_action_id") val savedActionId: String? = null,
)

@Serializable
data class PipelineEntry(
    val id: String,
    @SerialName("entry_type") val entryType: String,
    val title: String,
    val action: ActionPayload? = null,
)

@Serializable
data class PipelinePayload(
    val backlog: List<PipelineEntry>,
    val queue: List<PipelineEntry>,
)

class PipelineRepository(private val appContext: Context) {

    private val json = Json { ignoreUnknownKeys = true }

    private val dbPath: String
        get() = appContext.getDatabasePath("subroutine.db").absolutePath

    fun loadPipeline(): PipelinePayload {
        val raw = RustBridge.loadPipeline(dbPath)
        return json.decodeFromString(raw)
    }

    fun instantiateSavedAction(savedActionId: String): String {
        return RustBridge.instantiateSavedAction(dbPath, savedActionId)
    }

    fun promotePipelineEntry(entryId: String): Boolean {
        return RustBridge.promotePipelineEntry(dbPath, entryId)
    }

    fun demotePipelineEntry(entryId: String): Boolean {
        return RustBridge.demotePipelineEntry(dbPath, entryId)
    }

    fun deletePipelineAction(actionId: String): Boolean {
        return RustBridge.deletePipelineAction(dbPath, actionId)
    }
}
