package com.reus.gazeguard

import android.app.*
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import org.json.JSONObject
import java.util.Timer
import kotlin.concurrent.schedule

class BreakReminderService : Service() {
    private var timer: Timer? = null
    private var breakIntervalMillis = 15 * 60 * 1000L
    private var preBreakWarningMillis = 0L
    private val SERVICE_CHANNEL_ID = "BreakReminderServiceChannel"
    private val WARNING_CHANNEL_ID = "BreakReminderWarningChannel"
    private val BREAK_CHANNEL_ID = "BreakReminderBreakChannel"
    private val NOTIFICATION_ID = 1
    private val TAG = "BreakReminderService"
    private val overlayHandler = Handler(Looper.getMainLooper())
    private var overlayView: View? = null
    private var overlayMessageView: TextView? = null
    private var overlayTimerView: TextView? = null
    private val overlayRefreshRunnable = object : Runnable {
        override fun run() {
            refreshOverlayFromRust()
        }
    }

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
        logRustProbePhase("onStartCommand")
        when (intent?.action) {
            ACTION_START -> {
                Log.d(TAG, "Starting service")
                startForeground(NOTIFICATION_ID, createNotification("GazeGuard is running", NotificationKind.Service))
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
        logBreakAlertCapability("triggerBreak")
        forceRustBreakState()
        if (MainActivity.isAppVisible()) {
            val intent = Intent(this, BreakReceiver::class.java).apply {
                action = ACTION_TRIGGER_BREAK
            }
            sendBroadcast(intent)
            Log.d(TAG, "Broadcast sent while app is visible")
        } else {
            if (canDrawBreakOverlay()) {
                Log.d(TAG, "App is backgrounded; showing break overlay")
                showBreakOverlay()
            } else {
                Log.d(TAG, "App is backgrounded; relying on break notification instead of activity launch")
            }
        }

        val notification = createNotification("Time for a break!", NotificationKind.Break)
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
        val notification = createNotification(text, NotificationKind.Warning)
        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.notify(NOTIFICATION_ID + 2, notification)
    }

    private enum class NotificationKind {
        Service,
        Warning,
        Break,
    }

    private fun createNotification(
        text: String,
        kind: NotificationKind = NotificationKind.Service,
    ): Notification {
        val intent = Intent(this, MainActivity::class.java)
        val isBreakNotification = kind == NotificationKind.Break
        if (isBreakNotification) {
            intent.putExtra("show_break", true)
            intent.flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
        }

        val pendingIntent = PendingIntent.getActivity(
            this, if (isBreakNotification) 1 else 0, intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        val channelId = when (kind) {
            NotificationKind.Service -> SERVICE_CHANNEL_ID
            NotificationKind.Warning -> WARNING_CHANNEL_ID
            NotificationKind.Break -> BREAK_CHANNEL_ID
        }

        val builder = NotificationCompat.Builder(this, channelId)
            .setContentTitle("GazeGuard")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentIntent(pendingIntent)
            .setPriority(
                when (kind) {
                    NotificationKind.Service -> NotificationCompat.PRIORITY_LOW
                    NotificationKind.Warning -> NotificationCompat.PRIORITY_DEFAULT
                    NotificationKind.Break -> NotificationCompat.PRIORITY_MAX
                }
            )
            .setAutoCancel(isBreakNotification)

        if (isBreakNotification) {
            builder
                .setCategory(NotificationCompat.CATEGORY_ALARM)
                .setFullScreenIntent(pendingIntent, true)
        }

        return builder.build()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            notificationManager.createNotificationChannels(
                listOf(
                    NotificationChannel(
                        SERVICE_CHANNEL_ID,
                        "Break Reminder Service",
                        NotificationManager.IMPORTANCE_LOW
                    ).apply {
                        description = "Foreground service for break reminders"
                    },
                    NotificationChannel(
                        WARNING_CHANNEL_ID,
                        "Break Warnings",
                        NotificationManager.IMPORTANCE_DEFAULT
                    ).apply {
                        description = "Upcoming break warnings"
                    },
                    NotificationChannel(
                        BREAK_CHANNEL_ID,
                        "Break Alerts",
                        NotificationManager.IMPORTANCE_HIGH
                    ).apply {
                        description = "Immediate break alerts"
                        lockscreenVisibility = Notification.VISIBILITY_PUBLIC
                    }
                )
            )
            Log.d(TAG, "Notification channels created")
        }
    }

    private fun logRustProbePhase(source: String) {
        try {
            val phase = RustProbe.debugEnginePhase()
            Log.d(TAG, "Rust probe phase from $source: $phase")
        } catch (e: Throwable) {
            Log.e(TAG, "Rust probe failed from $source", e)
        }
    }

    private fun forceRustBreakState() {
        try {
            val phase = RustProbe.forceBreakNow()
            Log.d(TAG, "Rust forceBreakNow result: $phase")
        } catch (e: Throwable) {
            Log.e(TAG, "Rust forceBreakNow failed", e)
        }
    }

    private fun canDrawBreakOverlay(): Boolean {
        return Settings.canDrawOverlays(this)
    }

    private fun showBreakOverlay() {
        overlayHandler.post {
            if (overlayView == null) {
                val windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager
                val type = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
                } else {
                    @Suppress("DEPRECATION")
                    WindowManager.LayoutParams.TYPE_PHONE
                }
                val params = WindowManager.LayoutParams(
                    WindowManager.LayoutParams.MATCH_PARENT,
                    WindowManager.LayoutParams.MATCH_PARENT,
                    type,
                    WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN or
                        WindowManager.LayoutParams.FLAG_FULLSCREEN or
                        WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                        WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE,
                    android.graphics.PixelFormat.TRANSLUCENT
                ).apply {
                    gravity = Gravity.CENTER
                }

                val root = FrameLayout(this).apply {
                    setBackgroundColor(Color.argb(245, 0, 0, 0))
                    isClickable = true
                }
                val content = LinearLayout(this).apply {
                    orientation = LinearLayout.VERTICAL
                    gravity = Gravity.CENTER
                }
                val message = TextView(this).apply {
                    textSize = 28f
                    setTextColor(Color.WHITE)
                    gravity = Gravity.CENTER
                }
                val timer = TextView(this).apply {
                    textSize = 64f
                    setTextColor(Color.WHITE)
                    gravity = Gravity.CENTER
                }
                content.addView(message)
                content.addView(timer)
                root.addView(
                    content,
                    FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        FrameLayout.LayoutParams.MATCH_PARENT
                    )
                )
                root.setOnClickListener {
                    val launchIntent = Intent(this, MainActivity::class.java).apply {
                        flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
                        putExtra("show_break", true)
                    }
                    startActivity(launchIntent)
                }
                windowManager.addView(root, params)
                overlayView = root
                overlayMessageView = message
                overlayTimerView = timer
            }

            refreshOverlayFromRust()
        }
    }

    private fun refreshOverlayFromRust() {
        val snapshot = runCatching { JSONObject(RustProbe.breakOverlaySnapshot()) }
            .getOrNull()
        if (snapshot == null) {
            Log.e(TAG, "Overlay snapshot unavailable")
            hideBreakOverlay()
            return
        }

        val phase = snapshot.optString("phase")
        if (phase != "on_break") {
            Log.d(TAG, "Overlay closing because phase=$phase")
            hideBreakOverlay()
            return
        }

        overlayMessageView?.text = snapshot.optString("message", "Take a Break")
        overlayTimerView?.text = formatOverlayTime(snapshot.optLong("remaining_seconds", 0))
        overlayHandler.removeCallbacks(overlayRefreshRunnable)
        overlayHandler.postDelayed(overlayRefreshRunnable, 1000)
    }

    private fun hideBreakOverlay() {
        overlayHandler.removeCallbacks(overlayRefreshRunnable)
        val view = overlayView ?: return
        val windowManager = getSystemService(Context.WINDOW_SERVICE) as WindowManager
        runCatching { windowManager.removeView(view) }
        overlayView = null
        overlayMessageView = null
        overlayTimerView = null
    }

    private fun formatOverlayTime(totalSeconds: Long): String {
        val minutes = totalSeconds / 60
        val seconds = totalSeconds % 60
        return "%d:%02d".format(minutes, seconds)
    }

    private fun logBreakAlertCapability(source: String) {
        val notificationsEnabled = NotificationManagerCompat.from(this).areNotificationsEnabled()
        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        val channel = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            notificationManager.getNotificationChannel(BREAK_CHANNEL_ID)
        } else {
            null
        }
        val channelImportance = channel?.importance ?: -1
        val fullScreenAllowed = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            notificationManager.canUseFullScreenIntent()
        } else {
            true
        }

        Log.d(
            TAG,
            "Break alert capability from $source: notificationsEnabled=$notificationsEnabled, " +
                "fullScreenAllowed=$fullScreenAllowed, channelImportance=$channelImportance"
        )
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        Log.d(TAG, "Service onDestroy")
        hideBreakOverlay()
        stopBreakTimer()
    }
}
