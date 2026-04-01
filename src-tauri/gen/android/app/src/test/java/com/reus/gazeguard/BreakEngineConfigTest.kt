package com.reus.gazeguard

import org.junit.Assert.assertEquals
import org.junit.Test

class BreakEngineConfigTest {
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
}
