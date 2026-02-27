package com.example.subroutine.wear

import android.util.Log
import com.google.android.gms.wearable.MessageEvent
import com.google.android.gms.wearable.Wearable
import com.google.android.gms.wearable.WearableListenerService
import com.example.subroutine.RustBridge
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.tasks.await

private const val TAG = "PhoneWearSvc"

class PhoneWearableListenerService : WearableListenerService() {

    private val dbPath: String
        get() = getDatabasePath("subroutine.db").absolutePath

    override fun onMessageReceived(event: MessageEvent) {
        Log.d(TAG, "onMessageReceived: path=${event.path} from=${event.sourceNodeId}")
        when (event.path) {
            "/pipeline/load" -> handleLoadPipeline(event)
            "/pipeline/action/complete" -> handleCompleteAction(event)
            "/pipeline/action/demote" -> handleDemoteAction(event)
            else -> Log.w(TAG, "onMessageReceived: unhandled path=${event.path}")
        }
    }

    private fun handleLoadPipeline(event: MessageEvent) {
        Log.d(TAG, "handleLoadPipeline: loading from db=$dbPath")
        val json = runCatching { RustBridge.loadPipeline(dbPath) }
            .onSuccess { Log.d(TAG, "handleLoadPipeline: loaded ${it.length} chars") }
            .onFailure { Log.e(TAG, "handleLoadPipeline: RustBridge.loadPipeline failed", it) }
            .getOrElse { """{"error":"${it.message}"}""" }
        sendReply(event.sourceNodeId, "/pipeline/load/response", json.toByteArray())
    }

    private fun handleCompleteAction(event: MessageEvent) {
        val actionId = event.data.decodeToString()
        Log.d(TAG, "handleCompleteAction: actionId=$actionId")
        val success = runCatching { RustBridge.deletePipelineAction(dbPath, actionId) }
            .onFailure { Log.e(TAG, "handleCompleteAction: deletePipelineAction failed", it) }
            .getOrElse { false }
        // After completing, send the refreshed pipeline back so the watch updates
        val json = if (success) {
            runCatching { RustBridge.loadPipeline(dbPath) }
                .onSuccess { Log.d(TAG, "handleCompleteAction: reload success, ${it.length} chars") }
                .onFailure { Log.e(TAG, "handleCompleteAction: reload after complete failed", it) }
                .getOrElse { """{"error":"load after complete failed"}""" }
        } else {
            Log.w(TAG, "handleCompleteAction: deletePipelineAction returned false")
            """{"error":"complete failed"}"""
        }
        sendReply(event.sourceNodeId, "/pipeline/load/response", json.toByteArray())
    }

    private fun handleDemoteAction(event: MessageEvent) {
        val entryId = event.data.decodeToString()
        Log.d(TAG, "handleDemoteAction: entryId=$entryId")
        val success = runCatching { RustBridge.demotePipelineEntry(dbPath, entryId) }
            .onFailure { Log.e(TAG, "handleDemoteAction: demotePipelineEntry failed", it) }
            .getOrElse { false }
        val json = if (success) {
            runCatching { RustBridge.loadPipeline(dbPath) }
                .onSuccess { Log.d(TAG, "handleDemoteAction: reload success, ${it.length} chars") }
                .onFailure { Log.e(TAG, "handleDemoteAction: reload after demote failed", it) }
                .getOrElse { """{"error":"load after demote failed"}""" }
        } else {
            Log.w(TAG, "handleDemoteAction: demotePipelineEntry returned false")
            """{"error":"demote failed"}"""
        }
        sendReply(event.sourceNodeId, "/pipeline/load/response", json.toByteArray())
    }

    private fun sendReply(nodeId: String, path: String, data: ByteArray) {
        Log.d(TAG, "sendReply: path=$path to=$nodeId size=${data.size}")
        runBlocking {
            runCatching {
                Wearable.getMessageClient(this@PhoneWearableListenerService)
                    .sendMessage(nodeId, path, data)
                    .await()
            }.onSuccess {
                Log.d(TAG, "sendReply: sent successfully")
            }.onFailure {
                Log.e(TAG, "sendReply: failed to send $path", it)
            }
        }
    }
}
