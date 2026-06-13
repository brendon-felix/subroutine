package com.example.subroutine_simple.data.repository

import android.util.Log
import com.example.subroutine_simple.RustBridge
import com.example.subroutine_simple.data.models.Action
import com.example.subroutine_simple.data.models.ActionTemplate
import com.example.subroutine_simple.data.models.Event
import com.example.subroutine_simple.data.models.EventTemplate
import com.example.subroutine_simple.data.models.Routine
import com.example.subroutine_simple.data.network.RetrofitClient
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import retrofit2.HttpException

data class AllItems(
    val actions: List<Action>,
    val events: List<Event>,
    val routines: List<Routine>,
    val actionTemplates: List<ActionTemplate>,
    val eventTemplates: List<EventTemplate>,
)

class SubroutineRepository {

    private val api = RetrofitClient.api
    private val json = Json { ignoreUnknownKeys = true }

    /** Returns all actions, events, routines, and templates in a single network call. */
    suspend fun fetchAll(): AllItems =
        withContext(Dispatchers.IO) {
            val data = api.getAllData()
            AllItems(
                actions = data.actions,
                events = data.events,
                routines = data.routines,
                actionTemplates = data.actionTemplates,
                eventTemplates = data.eventTemplates,
            )
        }

    // ── Actions ────────────────────────────────────────────────────────────────

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

    suspend fun deleteAction(id: String) = withContext(Dispatchers.IO) {
        try {
            api.deleteAction(id)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "deleteAction HTTP ${e.code()}: $errorBody")
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

    suspend fun saveAction(id: String): Action = withContext(Dispatchers.IO) {
        try {
            api.saveAction(id)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "saveAction HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    suspend fun clearActionDuration(id: String): Action = withContext(Dispatchers.IO) {
        try {
            api.clearActionDuration(id)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "clearActionDuration HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    // ── Events ─────────────────────────────────────────────────────────────────

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

    suspend fun saveEvent(id: String): Event = withContext(Dispatchers.IO) {
        try {
            api.saveEvent(id)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "saveEvent HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    // ── Action templates ──────────────────────────────────────────────────────

    suspend fun upsertActionTemplate(template: ActionTemplate): ActionTemplate =
        withContext(Dispatchers.IO) {
            try {
                api.upsertActionTemplate(template.id, template)
            } catch (e: HttpException) {
                val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
                Log.e("SubroutineRepo", "upsertActionTemplate HTTP ${e.code()}: $errorBody")
                throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
            }
        }

    suspend fun deleteActionTemplate(id: String) = withContext(Dispatchers.IO) {
        try {
            api.deleteActionTemplate(id)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "deleteActionTemplate HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    // ── Event templates ───────────────────────────────────────────────────────

    suspend fun upsertEventTemplate(template: EventTemplate): EventTemplate =
        withContext(Dispatchers.IO) {
            try {
                api.upsertEventTemplate(template.id, template)
            } catch (e: HttpException) {
                val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
                Log.e("SubroutineRepo", "upsertEventTemplate HTTP ${e.code()}: $errorBody")
                throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
            }
        }

    suspend fun deleteEventTemplate(id: String) = withContext(Dispatchers.IO) {
        try {
            api.deleteEventTemplate(id)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "deleteEventTemplate HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    // ── Routines ───────────────────────────────────────────────────────────────

    suspend fun upsertRoutine(routine: Routine): Routine = withContext(Dispatchers.IO) {
        try {
            api.upsertRoutine(routine.id, routine)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "upsertRoutine HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    suspend fun deleteRoutine(id: String) = withContext(Dispatchers.IO) {
        try {
            api.deleteRoutine(id)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "deleteRoutine HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    suspend fun instantiateRoutine(
        id: String,
        startTime: String?,
    ): List<Action> = withContext(Dispatchers.IO) {
        val body = buildMap {
            if (startTime != null) put("start_time", startTime)
        }
        try {
            api.instantiateRoutine(id, body)
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "instantiateRoutine HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    // ── Pipeline ───────────────────────────────────────────────────────────────

    suspend fun refreshPipeline(): List<Action> = withContext(Dispatchers.IO) {
        try {
            api.refreshPipeline()
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "refreshPipeline HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }

    suspend fun expeditePipeline(): List<Action> = withContext(Dispatchers.IO) {
        try {
            api.expeditePipeline()
        } catch (e: HttpException) {
            val errorBody = e.response()?.errorBody()?.string() ?: "(no body)"
            Log.e("SubroutineRepo", "expeditePipeline HTTP ${e.code()}: $errorBody")
            throw RuntimeException("HTTP ${e.code()}: $errorBody", e)
        }
    }
}
