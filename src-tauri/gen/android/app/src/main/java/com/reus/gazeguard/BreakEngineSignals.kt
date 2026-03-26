package com.reus.gazeguard

object BreakEngineSignals {
    fun setIdleActiveScript(active: Boolean): String {
        return buildInvokeScript("set_idle_active", active)
    }

    fun setFullscreenActiveScript(active: Boolean): String {
        return buildInvokeScript("set_fullscreen_active", active)
    }

    private fun buildInvokeScript(command: String, active: Boolean): String {
        return """
            if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
                window.__TAURI__.core.invoke('$command', { active: $active });
            }
        """.trimIndent()
    }
}
