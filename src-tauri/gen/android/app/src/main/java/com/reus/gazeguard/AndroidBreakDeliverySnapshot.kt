package com.reus.gazeguard

import org.json.JSONObject

data class AndroidBreakDeliverySnapshot(
    val phase: String,
    val remainingSeconds: Long,
    val message: String,
    val shouldShowNotification: Boolean,
    val shouldShowOverlay: Boolean,
) {
    companion object {
        fun fromRustJson(raw: String): AndroidBreakDeliverySnapshot {
            val json = JSONObject(raw)
            return AndroidBreakDeliverySnapshot(
                phase = json.optString("phase"),
                remainingSeconds = json.optLong("remaining_seconds", 0),
                message = json.optString("message", "Break unavailable"),
                shouldShowNotification = json.optBoolean("should_show_notification", false),
                shouldShowOverlay = json.optBoolean("should_show_overlay", false),
            )
        }
    }
}
