package com.example.subroutine_simple

import androidx.navigation3.runtime.NavKey
import kotlinx.serialization.Serializable

@Serializable
data object MainRoute : NavKey

/** Navigate to the edit screen for the action with [actionId]. */
@Serializable
data class EditActionRoute(val actionId: String) : NavKey

/** Navigate to the edit screen for the event with [eventId]. */
@Serializable
data class EditEventRoute(val eventId: String) : NavKey
