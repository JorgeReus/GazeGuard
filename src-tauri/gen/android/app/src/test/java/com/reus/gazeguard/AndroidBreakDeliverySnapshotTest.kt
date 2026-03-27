package com.reus.gazeguard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidBreakDeliverySnapshotTest {
    @Test
    fun parses_postpone_fields_from_rust_snapshot() {
        val snapshot = AndroidBreakDeliverySnapshot.fromRustJson(
            """
            {
              "phase":"on_break",
              "remaining_seconds":15,
              "message":"Take a Break",
              "should_show_notification":true,
              "should_show_overlay":true,
              "can_postpone":true,
              "postpone_options":[
                {"duration":5,"unit":"minutes","seconds":300},
                {"duration":10,"unit":"minutes","seconds":600}
              ]
            }
            """.trimIndent()
        )

        assertEquals("on_break", snapshot.phase)
        assertTrue(snapshot.canPostpone)
        assertEquals(2, snapshot.postponeOptions.size)
        assertEquals(300L, snapshot.postponeOptions[0].seconds)
        assertEquals("minutes", snapshot.postponeOptions[1].unit)
    }
}
