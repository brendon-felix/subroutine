package com.example.subroutine_simple.data.network

import android.util.Log
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.logging.HttpLoggingInterceptor
import okhttp3.sse.EventSource
import okhttp3.sse.EventSourceListener
import okhttp3.sse.EventSources

private const val TAG = "SseManager"

class SseManager(private val baseUrl: String) {

    // A dedicated client for SSE — deliberately does NOT use BODY-level logging.
    // HttpLoggingInterceptor at BODY level tries to buffer the entire response
    // body before passing it on. An SSE stream never ends, so that interceptor
    // would block forever and no events would ever be delivered.
    private val sseClient = OkHttpClient.Builder()
        .addInterceptor(
            HttpLoggingInterceptor().apply { level = HttpLoggingInterceptor.Level.HEADERS }
        )
        .build()

    /**
     * Returns a [Flow] that emits [Unit] whenever the server broadcasts a
     * change event. The flow never completes on its own — it reconnects
     * automatically after failures. Cancel the collection (e.g. by cancelling
     * the coroutine scope) to stop listening.
     *
     * Callers should apply [kotlinx.coroutines.flow.retryWhen] for
     * reconnection on close (handled in [MainViewModel]).
     */
    fun changeEvents(): Flow<Unit> = callbackFlow {
        val request = Request.Builder()
            .url("${baseUrl}api/changes/stream")
            .header("Accept", "text/event-stream")
            .build()

        val listener = object : EventSourceListener() {
            override fun onEvent(
                eventSource: EventSource,
                id: String?,
                type: String?,
                data: String,
            ) {
                // The server sends "ping" as keep-alive data; ignore it.
                if (data.isNotBlank() && data != "ping") {
                    Log.d(TAG, "change event received: $data")
                    trySend(Unit)
                }
            }

            override fun onFailure(
                eventSource: EventSource,
                t: Throwable?,
                response: Response?,
            ) {
                Log.w(TAG, "SSE connection failed: ${t?.message ?: response?.code}")
                // Closing with an exception triggers retryWhen in the collector.
                close(t ?: Exception("SSE connection failed (HTTP ${response?.code})"))
            }

            override fun onClosed(eventSource: EventSource) {
                Log.d(TAG, "SSE stream closed by server")
                // Close with an exception so retryWhen reconnects, rather than
                // letting the flow terminate normally.
                close(Exception("SSE stream closed by server"))
            }
        }

        val eventSource = EventSources.createFactory(sseClient)
            .newEventSource(request, listener)

        // Called when the coroutine is cancelled (ViewModel cleared, etc.).
        awaitClose {
            Log.d(TAG, "cancelling SSE connection")
            eventSource.cancel()
        }
    }
}
