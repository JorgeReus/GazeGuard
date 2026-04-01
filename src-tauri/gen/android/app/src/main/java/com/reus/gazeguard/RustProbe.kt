package com.reus.gazeguard

object RustProbe {
    init {
        System.loadLibrary("gazeguard_lib")
    }

    external fun debugEnginePhase(): String
    external fun forceBreakNow(): String
    external fun postponeBreak(seconds: Long): String
    external fun breakOverlaySnapshot(): String
}
