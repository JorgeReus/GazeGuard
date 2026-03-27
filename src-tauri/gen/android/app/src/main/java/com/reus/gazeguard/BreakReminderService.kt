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
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class BreakReminderService : Service() {
    private val SERVICE_CHANNEL_ID = "BreakReminderServiceChannel"
    private val WARNING_CHANNEL_ID = "BreakReminderWarningChannel"
    private val BREAK_CHANNEL_ID = "BreakReminderBreakChannel"
    private val NOTIFICATION_ID = 1
    private val TAG = "BreakReminderService"
    private val mainHandler = Handler(Looper.getMainLooper())
    private val deliveryCoordinator = BreakDeliveryCoordinator()
    @Volatile
    private var pollingActive = false
    private var overlayView: View? = null
    private var overlayMessageView: TextView? = null
    private var overlayTimerView: TextView? = null
    private val rustDeliveryPollRunnable = object : Runnable {
        override fun run() {
            pollRustDeliveryState()
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
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Log.d(TAG, "onStartCommand: ${intent?.action}")
        when (intent?.action) {
            ACTION_START -> {
                Log.d(TAG, "Starting service")
                startForeground(NOTIFICATION_ID, createNotification("GazeGuard is running", NotificationKind.Service))
                startRustDeliveryPolling()
            }
            ACTION_STOP -> {
                Log.d(TAG, "Stopping service")
                stopRustDeliveryPolling()
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

    private fun startRustDeliveryPolling() {
        Log.d(TAG, "Starting Rust delivery polling")
        stopRustDeliveryPolling()
        pollingActive = true
        deliveryCoordinator.reset()
        mainHandler.post(rustDeliveryPollRunnable)
    }

    private fun stopRustDeliveryPolling() {
        Log.d(TAG, "Stopping Rust delivery polling")
        pollingActive = false
        mainHandler.removeCallbacks(rustDeliveryPollRunnable)
        deliveryCoordinator.reset()
        hideBreakOverlay()
    }

    private fun pollRustDeliveryState() {
        try {
            val snapshot = AndroidBreakDeliverySnapshot.fromRustJson(RustProbe.breakOverlaySnapshot())
            val effects = deliveryCoordinator.computeEffects(
                snapshot = snapshot,
                appVisible = MainActivity.isAppVisible(),
                overlayAllowed = canDrawBreakOverlay(),
            )

            if (effects.showWarningNotification) {
                Log.d(TAG, "Rust warning phase detected")
                showWarningNotification(snapshot.message)
            }

            if (effects.showBreakNotification) {
                Log.d(TAG, "Rust break phase detected")
                showBreakNotification(snapshot.message)
            }

            if (effects.sendBreakBroadcast) {
                val intent = Intent(this, BreakReceiver::class.java).apply {
                    action = ACTION_TRIGGER_BREAK
                }
                sendBroadcast(intent)
                Log.d(TAG, "Broadcast sent while app is visible")
            }

            if (effects.showBreakOverlay) {
                showBreakOverlay(snapshot)
            } else if (effects.hideBreakOverlay) {
                hideBreakOverlay()
            }
        } catch (e: Throwable) {
            Log.e(TAG, "Rust delivery poll failed", e)
            hideBreakOverlay()
        } finally {
            mainHandler.removeCallbacks(rustDeliveryPollRunnable)
            if (pollingActive) {
                mainHandler.postDelayed(rustDeliveryPollRunnable, 1000L)
            }
        }
    }

    private fun showBreakNotification(text: String) {
        val notification = createNotification(text, NotificationKind.Break)
        val notificationManager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        notificationManager.notify(NOTIFICATION_ID + 1, notification)
        Log.d(TAG, "Break notification shown")
    }

    private fun showWarningNotification(text: String) {
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

    private fun canDrawBreakOverlay(): Boolean {
        return Settings.canDrawOverlays(this)
    }

    private fun showBreakOverlay(snapshot: AndroidBreakDeliverySnapshot) {
        mainHandler.post {
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
                        WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS or
                        WindowManager.LayoutParams.FLAG_FULLSCREEN or
                        WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                        WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE,
                    android.graphics.PixelFormat.TRANSLUCENT
                ).apply {
                    gravity = Gravity.CENTER
                }

                val root = FrameLayout(this).apply {
                    setBackgroundColor(Color.BLACK)
                    isClickable = true
                    fitsSystemWindows = false
                }
                val bottomScrim = View(this).apply {
                    setBackgroundColor(Color.BLACK)
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
                root.addView(
                    bottomScrim,
                    FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        0,
                        Gravity.BOTTOM
                    )
                )
                ViewCompat.setOnApplyWindowInsetsListener(root) { _, insets ->
                    val navInsets = insets.getInsets(WindowInsetsCompat.Type.navigationBars())
                    val layoutParams = bottomScrim.layoutParams as FrameLayout.LayoutParams
                    layoutParams.height = navInsets.bottom
                    bottomScrim.layoutParams = layoutParams
                    insets
                }
                root.requestApplyInsets()
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
            overlayMessageView?.text = snapshot.message
            overlayTimerView?.text = formatOverlayTime(snapshot.remainingSeconds)
        }
    }

    private fun hideBreakOverlay() {
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

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        Log.d(TAG, "Service onDestroy")
        hideBreakOverlay()
        stopRustDeliveryPolling()
    }
}
