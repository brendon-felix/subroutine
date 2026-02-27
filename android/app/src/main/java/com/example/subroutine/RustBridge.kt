package com.example.subroutine

object RustBridge {
    init {
        System.loadLibrary("android_bridge")
    }

    external fun fetchSavedActions(dbPath: String): String
    external fun insertSavedAction(dbPath: String, title: String, content: String): Boolean
    external fun deleteSavedAction(dbPath: String, id: String): Boolean

    external fun instantiateSavedAction(dbPath: String, savedActionId: String): String
    external fun loadPipeline(dbPath: String): String
    external fun promotePipelineEntry(dbPath: String, entryId: String): Boolean
    external fun demotePipelineEntry(dbPath: String, entryId: String): Boolean
    external fun deletePipelineAction(dbPath: String, actionId: String): Boolean
}
