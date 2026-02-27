package com.example.subroutine.data

import android.content.Context
import com.google.android.gms.wearable.Wearable
import kotlinx.coroutines.tasks.await
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
    val backlog: List<PipelineEntry> = emptyList(),
    val queue: List<PipelineEntry> = emptyList(),
)

sealed interface PipelineResult {
    data class Success(val pipeline: PipelinePayload) : PipelineResult
    data class Error(val message: String) : PipelineResult
}

class WearPipelineRepository(private val context: Context) {

    private val json = Json { ignoreUnknownKeys = true }

    // Lazy so that Play Services client construction is deferred until first use on an IO
    // thread, rather than happening on the main thread during ViewModel initialisation.
    // `by lazy` is thread-safe by default (LazyThreadSafetyMode.SYNCHRONIZED).
    private val messageClient by lazy { Wearable.getMessageClient(context) }
    private val nodeClient by lazy { Wearable.getNodeClient(context) }

    suspend fun findPhoneNodeId(): String? {
        val nodes = nodeClient.connectedNodes.await()
        // Prefer a nearby (directly connected) node, fall back to any reachable node.
        return nodes.firstOrNull { it.isNearby }?.id
            ?: nodes.firstOrNull()?.id
    }

    suspend fun sendLoadPipeline(nodeId: String) {
        messageClient.sendMessage(nodeId, "/pipeline/load", byteArrayOf()).await()
    }

    suspend fun sendCompleteAction(nodeId: String, actionId: String) {
        messageClient.sendMessage(nodeId, "/pipeline/action/complete", actionId.toByteArray()).await()
    }

    suspend fun sendDemoteAction(nodeId: String, entryId: String) {
        messageClient.sendMessage(nodeId, "/pipeline/action/demote", entryId.toByteArray()).await()
    }

    fun decodePipeline(responseBytes: ByteArray): PipelineResult {
        val raw = responseBytes.decodeToString()
        return runCatching {
            val payload = json.decodeFromString<PipelinePayload>(raw)
            PipelineResult.Success(payload)
        }.getOrElse {
            PipelineResult.Error("Failed to parse pipeline: ${it.message}")
        }
    }
}
