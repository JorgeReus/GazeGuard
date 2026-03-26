package com.reus.gazeguard

import android.app.*
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import java.util.Timer
import kotlin.concurrent.schedule

class BreakReminderService : Service() {
    private var timer: Timer? = null
    private var breakIntervalMillis = 15 * 60 * 1000L
    private var preBreakWarningMillis = 0L
    private val CHANNEL_ID = "BreakReminderChannel"
    private val NOTIFICATION_ID = 1
    private val TAG = "BreakReminderService"

    companion object {
        const val ACTION_START = "START_SERVICE"
        const val ACTION_STOP = "STOP_SERVICE"
        const val ACTION_TRIGGER_BREAK = "com.reus.gazeguard.TRIGGER_BREAK"
    }

    override fun onCreate() {
        super.onCreate()
        Log.d(TAG, "Service onCreate")
        val schedule = BreakEngineConfig.loadSchedule(this)
        breakIntervalMillis = schedule.breakIntervalMillis
        preBreakWarningMillis = schedule.preBreakWarningMillis
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d(TAG, "onStartCommand: ${intent?.action}")
        when (intent?.action) {
            ACTION_START -> {
                Log.d(TAG, "Starting service")
                startForeground(NOTIFICATION_ID, createNotification("GazeGuard is running"))
                startBreakTimer()
            }
            ACTION_STOP -> {
                Log.d(TAG, "Stopping service")
                stopBreakTimer()
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                    stopForeground(STOP_FOREGROUND_REMOVE)
                } else {
                    @Suppress("DEPRECATION")
                    stopForeground(true)
                }
                stopSelf()
            }
        }
        return START_STICKY
    }

    private fun startBreakTimer() {
        Log.d(TAG, "Starting break timer with interval: $breakIntervalMillis ms")
        stopBreakTimer()
        timer = Timer("break-reminder-cycle", true)
        scheduleNextCycle()
    }

    private fun stopBreakTimer() {
        Log.d(TAG, "Stopping break timer")
        timer?.cancel()
        timer = null
    }

    private fun scheduleNextCycle() {
        val cycleTimer = timer ?: return
        val warningDelay = (breakIntervalMillis - preBreakWarningMillis).coerceAtLeast(0L)

        if (preBreakWarningMillis > 0L && warningDelay > 0L) {
            cycleTimer.schedule(warningDelay) {
                Log.d(TAG, "Warning timer triggered")
                showWarningNotification()
            }
        }

        cycleTimer.schedule(breakIntervalMillis) {
            Log.d(TAG, "Break timer triggered")
            triggerBreak()
            scheduleNextCycle()
        }
    }

    private fun triggerBreak() {
        Log.d(TAG, "Triggering break")
        val intent = Intent(this, BreakReceiver::class.java).apply {
            action = ACTION_TRIGGER_BREAK
        }
        sendBroadcast(intent)
        Log.d(TAG, "Broadcast sent")

        val notification = createNotification("Time for a break!", true)
        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.notify(NOTIFICATION_ID + 1, notification)
        Log.d(TAG, "Break notification shown")
    }

    private fun showWarningNotification() {
        val text = if (preBreakWarningMillis >= 1000L) {
            "Break starts in ${preBreakWarningMillis / 1000L} seconds"
        } else {
            "Break is coming up"
        }
        val notification = createNotification(text)
        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.notify(NOTIFICATION_ID + 2, notification)
    }

    private fun createNotification(text: String, isBreakNotification: Boolean = false): Notification {
        val intent = Intent(this, MainActivity::class.java)
        if (isBreakNotification) {
            intent.putExtra("show_break", true)
            intent.flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }

        val pendingIntent = PendingIntent.getActivity(
            this, if (isBreakNotification) 1 else 0, intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("GazeGuard")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentIntent(pendingIntent)
            .setPriority(if (isBreakNotification) NotificationCompat.PRIORITY_HIGH else NotificationCompat.PRIORITY_LOW)
            .setAutoCancel(isBreakNotification)
            .build()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Break Reminders",
                NotificationManager.IMPORTANCE_DEFAULT
            ).apply {
                description = "Notifications for break reminders"
            }
            val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.createNotificationChannel(channel)
            Log.d(TAG, "Notification channel created")
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        Log.d(TAG, "Service onDestroy")
        stopBreakTimer()
    }
}
