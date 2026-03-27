package com.reus.gazeguard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class BreakDeliveryCoordinatorTest {
    @Test
    fun emits_warning_notification_once_when_phase_enters_warning() {
        val coordinator = BreakDeliveryCoordinator()
        val snapshot = AndroidBreakDeliverySnapshot(
            phase = "warning",
            remainingSeconds = 10,
            message = "Break starts in 10 seconds",
            shouldShowNotification = true,
            shouldShowOverlay = false,
            canPostpone = false,
            postponeOptions = emptyList(),
        )

        val first = coordinator.computeEffects(
            snapshot = snapshot,
            appVisible = false,
            overlayAllowed = true,
        )
        val second = coordinator.computeEffects(
            snapshot = snapshot,
            appVisible = false,
            overlayAllowed = true,
        )

        assertTrue(first.showWarningNotification)
        assertFalse(first.showBreakNotification)
        assertFalse(first.showBreakOverlay)
        assertFalse(first.hideBreakOverlay)
        assertFalse(second.showWarningNotification)
    }

    @Test
    fun emits_break_delivery_once_when_phase_enters_break() {
        val coordinator = BreakDeliveryCoordinator()
        coordinator.computeEffects(
            snapshot = AndroidBreakDeliverySnapshot(
                phase = "warning",
                remainingSeconds = 10,
                message = "Break starts in 10 seconds",
                shouldShowNotification = true,
                shouldShowOverlay = false,
                canPostpone = false,
                postponeOptions = emptyList(),
            ),
            appVisible = false,
            overlayAllowed = true,
        )

        val effects = coordinator.computeEffects(
            snapshot = AndroidBreakDeliverySnapshot(
                phase = "on_break",
                remainingSeconds = 15,
                message = "Take a Break",
                shouldShowNotification = true,
                shouldShowOverlay = true,
                canPostpone = false,
                postponeOptions = emptyList(),
            ),
            appVisible = false,
            overlayAllowed = true,
        )

        assertFalse(effects.showWarningNotification)
        assertTrue(effects.showBreakNotification)
        assertTrue(effects.showBreakOverlay)
        assertFalse(effects.sendBreakBroadcast)
        assertEquals("on_break", effects.phase)
    }
}
