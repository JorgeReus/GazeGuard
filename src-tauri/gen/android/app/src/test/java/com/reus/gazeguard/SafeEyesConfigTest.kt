package com.reus.gazeguard

import org.junit.Assert.assertEquals
import org.junit.Test

class SafeEyesConfigTest {
    @Test
    fun parses_break_interval_from_safe_eyes_json() {
        val json = """
            {
              "break_interval": 15,
              "short_break_duration": 15,
              "long_break_duration": 60,
              "no_of_short_breaks_per_long_break": 5,
              "strict_break": false
            }
        """.trimIndent()

        assertEquals(15 * 60 * 1000L, SafeEyesConfig.parseBreakIntervalMillis(json))
    }
}
