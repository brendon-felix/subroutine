package com.example.subroutine_simple

import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/**
 * Bridge to the Rust android-bridge cdylib.
 *
 * To activate the native library:
 *   1. Build with: cargo ndk -t arm64-v8a -o app/src/main/jniLibs build -p android-bridge --release
 *   2. Uncomment the init block and external fun below.
 *   3. Remove the Kotlin stub implementations.
 */
object RustBridge {

    // ── Native library (uncomment when .so is built) ──────────────────────────
    // init { System.loadLibrary("android_bridge") }
    // external fun createAction(title: String): String

    // ── Kotlin stubs (remove when switching to native) ────────────────────────

    /**
     * Creates a new backlogged Action from a title and returns it as a JSON string.
     * Mirrors what the Rust bridge will produce via simple_core::Action::new(title).
     */
    fun createAction(title: String): String {
        val id = java.util.UUID.randomUUID().toString()
        return buildJsonObject {
            put("id", id)
            put("lineage_id", id)
            put("origin_routine_id", JsonNull)
            put("title", title)
            put("content", JsonNull)
            put("duration", JsonNull)
            put("recurrence", JsonNull)
            put("saved", false)
            put("state", buildJsonObject {
                put("Backlogged", JsonNull)
            })
        }.toString()
    }
}
