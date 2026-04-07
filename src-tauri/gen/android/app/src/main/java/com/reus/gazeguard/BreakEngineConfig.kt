package com.reus.gazeguard

import android.content.Context
import java.io.File
import org.yaml.snakeyaml.Yaml

object BreakEngineConfig {
    private const val CONFIG_ASSET_PATH = "defaults.yaml"
    private const val CONFIG_DIR_NAME = "config"
    private const val CONFIG_FILE_NAME = "config.yaml"
    private const val DEFAULT_BREAK_INTERVAL_MINUTES = 15L
    private const val DEFAULT_PRE_BREAK_WARNING_SECONDS = 0L

    data class Schedule(
        val breakIntervalMillis: Long,
        val preBreakWarningMillis: Long,
    )

    fun ensureConfigFile(configFile: File, defaultYaml: String): File {
        configFile.parentFile?.mkdirs()
        if (!configFile.exists()) {
            configFile.writeText(defaultYaml)
        }
        return configFile
    }

    fun resolveConfigFile(context: Context): File {
        return File(File(context.filesDir, CONFIG_DIR_NAME), CONFIG_FILE_NAME)
    }

    fun loadSchedule(context: Context): Schedule {
        val defaultYaml = context.assets.open(CONFIG_ASSET_PATH).bufferedReader().use { it.readText() }
        val configFile = ensureConfigFile(resolveConfigFile(context), defaultYaml)
        return parseSchedule(configFile.readText())
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
