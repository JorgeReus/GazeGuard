package com.reus.gazeguard

data class BreakDeliveryEffects(
    val phase: String,
    val showWarningNotification: Boolean,
    val showBreakNotification: Boolean,
    val sendBreakBroadcast: Boolean,
    val showBreakOverlay: Boolean,
    val hideBreakOverlay: Boolean,
)

class BreakDeliveryCoordinator {
    private var lastPhase: String? = null

    fun computeEffects(
        snapshot: AndroidBreakDeliverySnapshot,
        appVisible: Boolean,
        overlayAllowed: Boolean,
    ): BreakDeliveryEffects {
        val phaseChanged = snapshot.phase != lastPhase
        val wasOnBreak = lastPhase == "on_break"

        val showWarningNotification =
            phaseChanged && snapshot.phase == "warning" && snapshot.shouldShowNotification
        val showBreakNotification =
            phaseChanged && snapshot.phase == "on_break" && snapshot.shouldShowNotification
        val sendBreakBroadcast = showBreakNotification && appVisible
        val showBreakOverlay =
            snapshot.phase == "on_break" && snapshot.shouldShowOverlay && !appVisible && overlayAllowed
        val hideBreakOverlay = wasOnBreak && !showBreakOverlay

        lastPhase = snapshot.phase

        return BreakDeliveryEffects(
            phase = snapshot.phase,
            showWarningNotification = showWarningNotification,
            showBreakNotification = showBreakNotification,
            sendBreakBroadcast = sendBreakBroadcast,
            showBreakOverlay = showBreakOverlay,
            hideBreakOverlay = hideBreakOverlay,
        )
    }

    fun reset() {
        lastPhase = null
    }
}
