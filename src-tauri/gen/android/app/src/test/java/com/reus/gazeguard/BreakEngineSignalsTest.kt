package com.reus.gazeguard

import org.junit.Assert.assertTrue
import org.junit.Test

class BreakEngineSignalsTest {
    @Test
    fun builds_idle_signal_script() {
        val script = BreakEngineSignals.setIdleActiveScript(true)

        assertTrue(script.contains("set_idle_active"))
        assertTrue(script.contains("active: true"))
    }

    @Test
    fun builds_fullscreen_signal_script() {
        val script = BreakEngineSignals.setFullscreenActiveScript(false)

        assertTrue(script.contains("set_fullscreen_active"))
        assertTrue(script.contains("active: false"))
    }
}
