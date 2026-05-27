package com.example.subroutine_simple.data.network

import com.example.subroutine_simple.data.models.Action
import com.example.subroutine_simple.data.models.AllData
import com.example.subroutine_simple.data.models.CompleteResult
import com.example.subroutine_simple.data.models.Event
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Path

interface SubroutineApi {
    @GET("api/data")
    suspend fun getAllData(): AllData

    @PUT("api/actions/{id}")
    suspend fun upsertAction(@Path("id") id: String, @Body action: Action): Action

    @POST("api/actions/{id}/complete")
    suspend fun completeAction(@Path("id") id: String): CompleteResult

    @POST("api/actions/{id}/queue")
    suspend fun queueAction(@Path("id") id: String): List<Action>

    @POST("api/actions/{id}/backlog")
    suspend fun backlogAction(@Path("id") id: String): Action

    @PUT("api/events/{id}")
    suspend fun upsertEvent(@Path("id") id: String, @Body event: Event): Event

    @DELETE("api/events/{id}")
    suspend fun deleteEvent(@Path("id") id: String)
}
