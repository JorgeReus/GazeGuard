package com.reus.gazeguard

import android.content.Context

object BreakEngineConfig {
    private const val CONFIG_ASSET_PATH = "config/safeeyes.json"
    private const val DEFAULT_BREAK_INTERVAL_MINUTES = 15L
    private const val DEFAULT_PRE_BREAK_WARNING_SECONDS = 0L
    private val BREAK_INTERVAL_PATTERN = Regex(""""break_interval"\s*:\s*(\d+)""")
    private val PRE_BREAK_WARNING_PATTERN = Regex(""""pre_break_warning_time"\s*:\s*(\d+)""")

    data class Schedule(
        val breakIntervalMillis: Long,
        val preBreakWarningMillis: Long,
    )

    fun loadSchedule(context: Context): Schedule {
        val json = context.assets.open(CONFIG_ASSET_PATH).bufferedReader().use { it.readText() }
        return parseSchedule(json)
    }

    fun loadBreakIntervalMillis(context: Context): Long {
        return loadSchedule(context).breakIntervalMillis
    }

    fun parseBreakIntervalMillis(json: String): Long {
        return parseSchedule(json).breakIntervalMillis
    }

    fun parseSchedule(json: String): Schedule {
        val breakIntervalMinutes = BREAK_INTERVAL_PATTERN
            .find(json)
            ?.groupValues
            ?.get(1)
            ?.toLongOrNull()
            ?: DEFAULT_BREAK_INTERVAL_MINUTES
        val preBreakWarningSeconds = PRE_BREAK_WARNING_PATTERN
            .find(json)
            ?.groupValues
            ?.get(1)
            ?.toLongOrNull()
            ?: DEFAULT_PRE_BREAK_WARNING_SECONDS

        return Schedule(
            breakIntervalMillis = breakIntervalMinutes * 60 * 1000L,
            preBreakWarningMillis = preBreakWarningSeconds * 1000L,
        )
    }
}
