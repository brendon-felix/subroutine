package com.example.subroutine_simple.data.network

import com.example.subroutine_simple.data.models.Action
import com.example.subroutine_simple.data.models.ActionTemplate
import com.example.subroutine_simple.data.models.AllData
import com.example.subroutine_simple.data.models.CompleteResult
import com.example.subroutine_simple.data.models.Event
import com.example.subroutine_simple.data.models.EventTemplate
import com.example.subroutine_simple.data.models.Routine
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Path

interface SubroutineApi {
    @GET("api/data")
    suspend fun getAllData(): AllData

    // ── Actions ───────────────────────────────────────────────────────────────

    @GET("api/actions")
    suspend fun listActions(): List<Action>

    @POST("api/actions")
    suspend fun createAction(@Body action: Action): Action

    @GET("api/actions/{id}")
    suspend fun getAction(@Path("id") id: String): Action

    @PUT("api/actions/{id}")
    suspend fun upsertAction(@Path("id") id: String, @Body action: Action): Action

    @DELETE("api/actions/{id}")
    suspend fun deleteAction(@Path("id") id: String)

    @POST("api/actions/{id}/complete")
    suspend fun completeAction(@Path("id") id: String): CompleteResult

    @POST("api/actions/{id}/queue")
    suspend fun queueAction(@Path("id") id: String): List<Action>

    @POST("api/actions/{id}/backlog")
    suspend fun backlogAction(@Path("id") id: String): Action

    @POST("api/actions/{id}/save")
    suspend fun saveAction(@Path("id") id: String): Action

    @POST("api/actions/{id}/clear_duration")
    suspend fun clearActionDuration(@Path("id") id: String): Action

    // ── Action templates ──────────────────────────────────────────────────────

    @GET("api/actions/templates")
    suspend fun listActionTemplates(): List<ActionTemplate>

    @POST("api/actions/templates")
    suspend fun createActionTemplate(@Body template: ActionTemplate): ActionTemplate

    @GET("api/actions/templates/{id}")
    suspend fun getActionTemplate(@Path("id") id: String): ActionTemplate

    @PUT("api/actions/templates/{id}")
    suspend fun upsertActionTemplate(
        @Path("id") id: String,
        @Body template: ActionTemplate,
    ): ActionTemplate

    @DELETE("api/actions/templates/{id}")
    suspend fun deleteActionTemplate(@Path("id") id: String)

    // ── Events ────────────────────────────────────────────────────────────────

    @GET("api/events")
    suspend fun listEvents(): List<Event>

    @POST("api/events")
    suspend fun createEvent(@Body event: Event): Event

    @GET("api/events/{id}")
    suspend fun getEvent(@Path("id") id: String): Event

    @PUT("api/events/{id}")
    suspend fun upsertEvent(@Path("id") id: String, @Body event: Event): Event

    @DELETE("api/events/{id}")
    suspend fun deleteEvent(@Path("id") id: String)

    @POST("api/events/{id}/save")
    suspend fun saveEvent(@Path("id") id: String): Event

    // ── Event templates ───────────────────────────────────────────────────────

    @GET("api/events/templates")
    suspend fun listEventTemplates(): List<EventTemplate>

    @POST("api/events/templates")
    suspend fun createEventTemplate(@Body template: EventTemplate): EventTemplate

    @GET("api/events/templates/{id}")
    suspend fun getEventTemplate(@Path("id") id: String): EventTemplate

    @PUT("api/events/templates/{id}")
    suspend fun upsertEventTemplate(
        @Path("id") id: String,
        @Body template: EventTemplate,
    ): EventTemplate

    @DELETE("api/events/templates/{id}")
    suspend fun deleteEventTemplate(@Path("id") id: String)

    // ── Routines ──────────────────────────────────────────────────────────────

    @GET("api/routines")
    suspend fun listRoutines(): List<Routine>

    @POST("api/routines")
    suspend fun createRoutine(@Body routine: Routine): Routine

    @GET("api/routines/{id}")
    suspend fun getRoutine(@Path("id") id: String): Routine

    @PUT("api/routines/{id}")
    suspend fun upsertRoutine(@Path("id") id: String, @Body routine: Routine): Routine

    @DELETE("api/routines/{id}")
    suspend fun deleteRoutine(@Path("id") id: String)

    @POST("api/routines/{id}/instantiate")
    suspend fun instantiateRoutine(
        @Path("id") id: String,
        @Body request: Map<String, String>,
    ): List<Action>

    // ── Pipeline ──────────────────────────────────────────────────────────────

    @POST("api/pipeline/refresh")
    suspend fun refreshPipeline(): List<Action>

    @POST("api/pipeline/expedite")
    suspend fun expeditePipeline(): List<Action>
}
