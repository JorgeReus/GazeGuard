package com.reus.gazeguard

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

class BreakReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == "com.reus.gazeguard.TRIGGER_BREAK") {
            val launchIntent = Intent(context, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
                putExtra("show_break", true)
            }
            context.startActivity(launchIntent)
        }
    }
}