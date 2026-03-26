package com.reus.gazeguard

import org.junit.Assert.assertEquals
import org.junit.Test

class BreakEngineConfigTest {
    @Test
    fun parses_break_interval_from_safe_eyes_json() {
        val json = """
            {
              "break_interval": 15,
              "pre_break_warning_time": 10,
              "short_break_duration": 15,
              "long_break_duration": 60,
              "no_of_short_breaks_per_long_break": 5,
              "strict_break": false
            }
        """.trimIndent()

        assertEquals(15 * 60 * 1000L, BreakEngineConfig.parseBreakIntervalMillis(json))
    }

    @Test
    fun parses_warning_schedule_from_safe_eyes_json() {
        val json = """
            {
              "break_interval": 15,
              "pre_break_warning_time": 10
            }
        """.trimIndent()

        val schedule = BreakEngineConfig.parseSchedule(json)

        assertEquals(15 * 60 * 1000L, schedule.breakIntervalMillis)
        assertEquals(10 * 1000L, schedule.preBreakWarningMillis)
    }
}
