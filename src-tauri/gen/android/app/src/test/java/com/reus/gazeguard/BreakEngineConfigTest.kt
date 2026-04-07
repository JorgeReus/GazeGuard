package com.reus.gazeguard

import java.io.File
import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Test

class BreakEngineConfigTest {
    private fun createTempRoot(): File = Files.createTempDirectory("gazeguard-config-test").toFile()

    @Test
    fun parses_break_interval_from_defaults_yaml() {
        val yaml = """
            short_break_interval: 15
            long_break_interval: 75
            pre_break_warning_time: 10
        """.trimIndent()

        assertEquals(15 * 60 * 1000L, BreakEngineConfig.parseBreakIntervalMillis(yaml))
    }

    @Test
    fun parses_warning_schedule_from_defaults_yaml() {
        val yaml = """
            short_break_interval: 15
            long_break_interval: 75
            pre_break_warning_time: 10
        """.trimIndent()

        val schedule = BreakEngineConfig.parseSchedule(yaml)

        assertEquals(15 * 60 * 1000L, schedule.breakIntervalMillis)
        assertEquals(10 * 1000L, schedule.preBreakWarningMillis)
    }

    @Test
    fun parses_schedule_from_canonical_defaults_yaml() {
        val schedule = BreakEngineConfig.parseSchedule(
            File("../../../config/defaults.yaml").readText(),
        )

        assertEquals(1 * 60 * 1000L, schedule.breakIntervalMillis)
        assertEquals(10 * 1000L, schedule.preBreakWarningMillis)
    }

    @Test
    fun ensureConfigFileWritesDefaultsWhenMissing() {
        val root = createTempRoot()
        val configFile = File(root, "config/config.yaml")

        val resolved = BreakEngineConfig.ensureConfigFile(configFile, "short_break_interval: 15\n")

        assertEquals(configFile.absolutePath, resolved.absolutePath)
        assertEquals("short_break_interval: 15\n", configFile.readText())
    }

    @Test
    fun ensureConfigFileKeepsExistingContents() {
        val root = createTempRoot()
        val configFile = File(root, "config/config.yaml")
        configFile.parentFile!!.mkdirs()
        configFile.writeText("short_break_interval: 33\n")

        BreakEngineConfig.ensureConfigFile(configFile, "short_break_interval: 15\n")

        assertEquals("short_break_interval: 33\n", configFile.readText())
    }

    @Test
    fun parseScheduleReadsSeededConfigFileContents() {
        val root = createTempRoot()
        val configFile = File(root, "config/config.yaml")
        BreakEngineConfig.ensureConfigFile(
            configFile,
            """
            short_break_interval: 15
            long_break_interval: 75
            pre_break_warning_time: 10
            """.trimIndent(),
        )

        val schedule = BreakEngineConfig.parseSchedule(configFile.readText())

        assertEquals(15 * 60 * 1000L, schedule.breakIntervalMillis)
        assertEquals(10 * 1000L, schedule.preBreakWarningMillis)
    }
}
