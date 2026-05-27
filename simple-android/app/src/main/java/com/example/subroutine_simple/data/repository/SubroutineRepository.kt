package com.example.subroutine_simple.data.repository

import android.util.Log
import com.example.subroutine_simple.RustBridge
import com.example.subroutine_simple.data.models.Action
import com.example.subroutine_simple.data.models.Event
import com.example.subroutine_simple.data.network.RetrofitClient
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import retrofit2.HttpException

class SubroutineRepository {

    private val api = RetrofitClient.api
    private val json = Json { ignoreUnknownKeys = true }

    /** Returns all actions and events in a single network call. */
    suspend fun fetchAll(): Pair<List<Action>, List<Event>> = withContext(Dispatchers.IO) {
        val data = api.getAllData()
        Pair(data.actions, data.events)
    }

    /**
     * Creates a new backlogged Action via the Rust bridge and persists it to the server.
     * The bridge constructs the Action (assigning a UUID and default state); the
     * repository sends it straight to PUT /api/actions/:id.
     */
    suspend fun createAction(title: String): Action = withContext(Dispatchers.IO) {
        val actionJson = RustBridge.createAction(title)
        Log.d("SubroutineRepo", "createAction stub JSON: $actionJson")
        val action = json.decodeFromString<Action>(actionJson)
        try {
            api.upsertAction(action.id, action)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "upsertAction HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    suspend fun completeAction(id: String): Action = withContext(Dispatchers.IO) {
        api.completeAction(id).completed
    }

    /**
     * Saves edits to an action's title and/or content in place.
     * The action's state (queued/backlogged) is preserved — use [queueAction] / [backlogAction]
     * to transition state separately.
     */
    suspend fun updateAction(action: Action, title: String, content: String?): Action =
        withContext(Dispatchers.IO) {
            val updated = action.copy(title = title, content = content)
            try {
                api.upsertAction(updated.id, updated)
            } catch (e: HttpException) {
                val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
                Log.e("SubroutineRepo", "updateAction HTTP ${e.code()}: $errorBody")
                throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
            }
        }

    suspend fun queueAction(id: String) = withContext(Dispatchers.IO) {
        try {
            api.queueAction(id)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "queueAction HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    suspend fun backlogAction(id: String): Action = withContext(Dispatchers.IO) {
        try {
            api.backlogAction(id)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "backlogAction HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    suspend fun updateEvent(event: Event, title: String, content: String?): Event =
        withContext(Dispatchers.IO) {
            val updated = event.copy(title = title, content = content)
            try {
                api.upsertEvent(updated.id, updated)
            } catch (e: HttpException) {
                val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
                Log.e("SubroutineRepo", "updateEvent HTTP ${e.code()}: $errorBody")
                throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
            }
        }

    suspend fun deleteEvent(id: String) = withContext(Dispatchers.IO) {
        try {
            api.deleteEvent(id)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "deleteEvent HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }
}
