package com.example.subroutine_simple.ui.components

/**
 * Formats an ISO-8601 UTC string to a compact human-readable form, e.g. "Jan 15, 14:00".
 * Falls back to the raw string on any parse error.
 */
internal fun formatIsoTime(iso: String): String =
    runCatching {
        val datePart = iso.substring(0, 10)  // "2024-01-15"
        val timePart = iso.substring(11, 16) // "14:00"
        val (_, month, day) = datePart.split("-")
        val monthName = listOf(
            "", "Jan", "Feb", "Mar", "Apr", "May", "Jun",
            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"
        )[month.toInt()]
        "$monthName ${day.trimStart('0')}, $timePart"
    }.getOrDefault(iso)
