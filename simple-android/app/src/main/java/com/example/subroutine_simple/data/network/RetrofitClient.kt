package com.example.subroutine_simple.data.network

import com.jakewharton.retrofit2.converter.kotlinx.serialization.asConverterFactory
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.logging.HttpLoggingInterceptor
import retrofit2.Retrofit

object RetrofitClient {

    private const val BASE_URL = "http://100.112.215.8:3000/"

    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    private val okhttp = OkHttpClient.Builder()
        .addInterceptor(
            HttpLoggingInterceptor().apply {
                            level = HttpLoggingInterceptor.Level.BODY
                        }
        )
        .build()

    val api: SubroutineApi = Retrofit.Builder()
        .baseUrl(BASE_URL)
        .client(okhttp)
        .addConverterFactory(json.asConverterFactory("application/json".toMediaType()))
        .build()
        .create(SubroutineApi::class.java)
}
