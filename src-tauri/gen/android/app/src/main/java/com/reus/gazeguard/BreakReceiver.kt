package com.reus.gazeguard

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

class BreakReceiver : BroadcastReceiver() {
    private val tag = "BreakReceiver"

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == "com.reus.gazeguard.TRIGGER_BREAK") {
            Log.d(tag, "Received TRIGGER_BREAK broadcast")
            val launchIntent = Intent(context, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
                putExtra("show_break", true)
            }
            Log.d(tag, "Starting MainActivity for break screen")
            context.startActivity(launchIntent)
        }
    }
}
