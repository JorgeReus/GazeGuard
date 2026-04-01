package com.reus.gazeguard

import android.content.Context
import org.yaml.snakeyaml.Yaml

object BreakEngineConfig {
    private const val CONFIG_ASSET_PATH = "config/defaults.yaml"
    private const val DEFAULT_BREAK_INTERVAL_MINUTES = 15L
    private const val DEFAULT_PRE_BREAK_WARNING_SECONDS = 0L

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

    fun parseSchedule(yaml: String): Schedule {
        val parsed = Yaml().load<Map<String, Any?>>(yaml).orEmpty()
        val breakIntervalMinutes = (parsed["short_break_interval"] as? Number)?.toLong()
            ?: DEFAULT_BREAK_INTERVAL_MINUTES
        val preBreakWarningSeconds = (parsed["pre_break_warning_time"] as? Number)?.toLong()
            ?: DEFAULT_PRE_BREAK_WARNING_SECONDS

        return Schedule(
            breakIntervalMillis = breakIntervalMinutes * 60 * 1000L,
            preBreakWarningMillis = preBreakWarningSeconds * 1000L,
        )
    }
}
