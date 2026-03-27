package com.reus.gazeguard

import org.json.JSONObject

data class AndroidPostponeOption(
    val duration: Long,
    val unit: String,
    val seconds: Long,
)

data class AndroidBreakDeliverySnapshot(
    val phase: String,
    val remainingSeconds: Long,
    val message: String,
    val shouldShowNotification: Boolean,
    val shouldShowOverlay: Boolean,
    val canPostpone: Boolean,
    val postponeOptions: List<AndroidPostponeOption>,
) {
    companion object {
        fun fromRustJson(raw: String): AndroidBreakDeliverySnapshot {
            val json = JSONObject(raw.trim())
            val options = json.optJSONArray("postpone_options")
            val postponeOptions = mutableListOf<AndroidPostponeOption>()
            if (options != null) {
                for (index in 0 until options.length()) {
                    val option = options.optJSONObject(index) ?: continue
                    postponeOptions += AndroidPostponeOption(
                        duration = option.optLong("duration", 0),
                        unit = option.optString("unit", "minutes"),
                        seconds = option.optLong("seconds", 0),
                    )
                }
            }
            return AndroidBreakDeliverySnapshot(
                phase = json.optString("phase"),
                remainingSeconds = json.optLong("remaining_seconds", 0),
                message = json.optString("message", "Break unavailable"),
                shouldShowNotification = json.optBoolean("should_show_notification", false),
                shouldShowOverlay = json.optBoolean("should_show_overlay", false),
                canPostpone = json.optBoolean("can_postpone", false),
                postponeOptions = postponeOptions,
            )
        }
    }
}
