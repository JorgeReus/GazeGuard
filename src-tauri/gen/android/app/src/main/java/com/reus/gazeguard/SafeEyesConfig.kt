package com.reus.gazeguard

import android.content.Context

object SafeEyesConfig {
    private const val CONFIG_ASSET_PATH = "config/safeeyes.json"
    private const val DEFAULT_BREAK_INTERVAL_MINUTES = 15L
    private val BREAK_INTERVAL_PATTERN = Regex(""""break_interval"\s*:\s*(\d+)""")

    fun loadBreakIntervalMillis(context: Context): Long {
        val json = context.assets.open(CONFIG_ASSET_PATH).bufferedReader().use { it.readText() }
        return parseBreakIntervalMillis(json)
    }

    fun parseBreakIntervalMillis(json: String): Long {
        val breakIntervalMinutes = BREAK_INTERVAL_PATTERN
            .find(json)
            ?.groupValues
            ?.get(1)
            ?.toLongOrNull()
            ?: DEFAULT_BREAK_INTERVAL_MINUTES
        return breakIntervalMinutes * 60 * 1000L
    }
}
