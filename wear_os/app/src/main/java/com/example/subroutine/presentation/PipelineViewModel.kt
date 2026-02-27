package com.example.subroutine.presentation

import android.app.Application
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.util.Log
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.example.subroutine.WearMessageListenerService
import com.example.subroutine.data.PipelinePayload
import com.example.subroutine.data.PipelineResult
import com.example.subroutine.data.WearPipelineRepository
import com.google.android.gms.wearable.MessageClient
import com.google.android.gms.wearable.Wearable
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.tasks.await
import java.util.concurrent.atomic.AtomicLong

private const val TAG = "PipelineViewModel"

sealed interface PipelineUiState {
    data object Loading : PipelineUiState
    data object NoPhone : PipelineUiState

    // Refreshing is shown when the user explicitly requests a reload while data is already
    // visible. The existing pipeline is kept so the list stays on screen — only a spinner
    // is shown above it rather than collapsing the whole list to a Loading state.
    data class Refreshing(val pipeline: PipelinePayload) : PipelineUiState
    data class Success(val pipeline: PipelinePayload) : PipelineUiState
    data class Error(val message: String) : PipelineUiState
}

class PipelineViewModel(application: Application) : AndroidViewModel(application) {

    private val repository = WearPipelineRepository(application)

    private val _uiState = MutableStateFlow<PipelineUiState>(PipelineUiState.Loading)
    val uiState: StateFlow<PipelineUiState> = _uiState

    private var phoneNodeId: String? = null

    // Tracks the last time the foreground MessageClient listener handled a pipeline response.
    // The broadcast path (WearMessageListenerService) is a fallback for when the app is
    // backgrounded — if the foreground listener already handled the message we suppress the
    // redundant broadcast delivery to avoid processing the same payload twice.
    private val lastForegroundHandledAt = AtomicLong(0L)

    // Foreground listener: the callback fires on the main thread (Wearable API contract),
    // so we immediately dispatch to IO to do the JSON decoding off the main thread.
    private val messageListener = MessageClient.OnMessageReceivedListener { event ->
        Log.d(TAG, "onMessageReceived: path=${event.path} size=${event.data.size}")
        if (event.path == "/pipeline/load/response") {
            lastForegroundHandledAt.set(System.currentTimeMillis())
            val data = event.data
            viewModelScope.launch(Dispatchers.IO) {
                handlePipelineResponse(data)
            }
        }
    }

    // Background listener: fires when the app is not in the foreground. When the app IS in
    // the foreground, the MessageClient listener above already handled the message — the
    // WearableListenerService still receives it and broadcasts, so we suppress that duplicate
    // by checking whether the foreground path ran within the last 500ms.
    private val broadcastReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            Log.d(TAG, "broadcastReceiver.onReceive: action=${intent.action}")
            val payload = intent.getByteArrayExtra(WearMessageListenerService.EXTRA_PAYLOAD)
                ?: return
            val timeSinceForeground = System.currentTimeMillis() - lastForegroundHandledAt.get()
            if (timeSinceForeground < 2000L) {
                Log.d(TAG, "broadcastReceiver: suppressing duplicate (foreground handled ${timeSinceForeground}ms ago)")
                return
            }
            viewModelScope.launch(Dispatchers.IO) {
                handlePipelineResponse(payload)
            }
        }
    }

    init {
        // BroadcastReceiver must be registered on the main thread (Context API is not thread-safe).
        ContextCompat.registerReceiver(
            application,
            broadcastReceiver,
            IntentFilter(WearMessageListenerService.ACTION_PIPELINE_RESPONSE),
            ContextCompat.RECEIVER_NOT_EXPORTED,
        )
        // Register the Wearable message listener and then send the initial load request in the
        // same coroutine so there is no window between "listener registered" and "request sent"
        // where a response could arrive and be dropped.
        viewModelScope.launch(Dispatchers.IO) {
            runCatching {
                Wearable.getMessageClient(application).addListener(messageListener)
            }.onFailure { error ->
                Log.e(TAG, "Failed to register Wearable message listener", error)
            }
            loadPipelineInternal()
        }
    }

    override fun onCleared() {
        super.onCleared()
        val app = getApplication<Application>()
        // Remove the listener synchronously — if we dispatch to a new coroutine here the
        // ViewModel scope may already be cancelled before the remove completes, leaving a
        // stale listener registered and causing duplicate deliveries on the next launch.
        runCatching {
            Wearable.getMessageClient(app).removeListener(messageListener).also { task ->
                // Block briefly to let the remove complete before the process continues.
                // This is called from onCleared which runs on the main thread; the Wearable
                // task completes quickly so this won't ANR.
                task.addOnCompleteListener {
                    Log.d(TAG, "MessageClient listener removed")
                }
            }
        }.onFailure { error ->
            Log.e(TAG, "Failed to remove Wearable message listener", error)
        }
        app.unregisterReceiver(broadcastReceiver)
    }

    // Public entry point: called from the UI (Retry / Refresh buttons).
    fun loadPipeline() {
        viewModelScope.launch(Dispatchers.IO) {
            loadPipelineInternal()
        }
    }

    // Internal implementation — must already be on a coroutine; not necessarily IO but
    // all callers dispatch to IO before calling this.
    private suspend fun loadPipelineInternal() {
        // If we already have data, transition to Refreshing so the list stays visible
        // while the new payload is in flight. Only go to the full Loading spinner when
        // there is nothing to show yet.
        val current = _uiState.value
        _uiState.value = when (current) {
            is PipelineUiState.Success -> PipelineUiState.Refreshing(current.pipeline)
            is PipelineUiState.Refreshing -> current // already refreshing, leave it
            else -> PipelineUiState.Loading
        }
        Log.d(TAG, "loadPipeline: discovering phone node…")

        val nodeId = runCatching { repository.findPhoneNodeId() }
            .onFailure { error -> Log.e(TAG, "findPhoneNodeId failed", error) }
            .getOrNull()

        if (nodeId == null) {
            Log.w(TAG, "loadPipeline: no phone node found — showing NoPhone state")
            _uiState.value = PipelineUiState.NoPhone
            return
        }

        Log.d(TAG, "loadPipeline: found phone node=$nodeId, sending /pipeline/load")
        phoneNodeId = nodeId

        runCatching { repository.sendLoadPipeline(nodeId) }
            .onSuccess { Log.d(TAG, "loadPipeline: /pipeline/load sent successfully") }
            .onFailure { error ->
                Log.e(TAG, "loadPipeline: failed to send /pipeline/load", error)
                _uiState.value = PipelineUiState.Error("Failed to contact phone: ${error.message}")
            }
    }

    fun completeAction(actionId: String) {
        // Optimistically remove the item from the visible queue immediately so the list
        // doesn't flash to Loading mid-scroll. The phone will respond with the authoritative
        // state and handlePipelineResponse will reconcile.
        removeEntryOptimistically(actionId)
        viewModelScope.launch(Dispatchers.IO) {
            val nodeId = phoneNodeId ?: run {
                Log.w(TAG, "completeAction: no phoneNodeId cached, reloading pipeline")
                loadPipelineInternal()
                return@launch
            }
            Log.d(TAG, "completeAction: sending complete for actionId=$actionId")
            runCatching { repository.sendCompleteAction(nodeId, actionId) }
                .onFailure { error ->
                    Log.e(TAG, "completeAction: failed", error)
                    _uiState.value = PipelineUiState.Error("Failed to complete action: ${error.message}")
                }
        }
    }

    fun demoteAction(entryId: String) {
        // Optimistically remove the item from the visible queue immediately so the list
        // doesn't flash to Loading mid-scroll.
        removeEntryOptimistically(entryId)
        viewModelScope.launch(Dispatchers.IO) {
            val nodeId = phoneNodeId ?: run {
                Log.w(TAG, "demoteAction: no phoneNodeId cached, reloading pipeline")
                loadPipelineInternal()
                return@launch
            }
            Log.d(TAG, "demoteAction: sending demote for entryId=$entryId")
            runCatching { repository.sendDemoteAction(nodeId, entryId) }
                .onFailure { error ->
                    Log.e(TAG, "demoteAction: failed", error)
                    _uiState.value = PipelineUiState.Error("Failed to demote action: ${error.message}")
                }
        }
    }

    private fun removeEntryOptimistically(entryId: String) {
        val pipeline = when (val current = _uiState.value) {
            is PipelineUiState.Success -> current.pipeline
            is PipelineUiState.Refreshing -> current.pipeline
            else -> return
        }
        val updatedQueue = pipeline.queue.filterNot { it.id == entryId }
        _uiState.value = PipelineUiState.Success(
            pipeline.copy(queue = updatedQueue),
        )
    }

    // Called from IO dispatcher — StateFlow.value is thread-safe for writes,
    // so we can update it directly without switching to Main.
    private fun handlePipelineResponse(data: ByteArray) {
        val raw = data.decodeToString()
        Log.d(TAG, "handlePipelineResponse: ${raw.take(200)}")
        when (val result = repository.decodePipeline(data)) {
            is PipelineResult.Success -> {
                Log.d(
                    TAG,
                    "handlePipelineResponse: success — queue=${result.pipeline.queue.size} backlog=${result.pipeline.backlog.size}"
                )
                // Skip the update if the payload is identical to what's already displayed.
                // The MessageClient can deliver the same message multiple times when multiple
                // listener instances are registered (e.g. across ViewModel recreations), and
                // each redundant StateFlow update causes a full list recomposition.
                val current = _uiState.value
                if (current is PipelineUiState.Success && current.pipeline == result.pipeline) {
                    Log.d(TAG, "handlePipelineResponse: skipping duplicate payload")
                    return
                }
                _uiState.value = PipelineUiState.Success(result.pipeline)
            }

            is PipelineResult.Error -> {
                Log.e(TAG, "handlePipelineResponse: decode error — ${result.message}")
                _uiState.value = PipelineUiState.Error(result.message)
            }
        }
    }
}
