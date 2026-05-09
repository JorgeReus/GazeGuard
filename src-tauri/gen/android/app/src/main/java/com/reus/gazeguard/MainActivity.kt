package com.reus.gazeguard

import android.content.Intent
import android.content.IntentFilter
import android.graphics.Color
import android.app.NotificationManager
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.core.app.NotificationManagerCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {
    private var breakReceiver: BreakReceiver? = null
    private var webView: WebView? = null
    private val TAG = "MainActivity"

    companion object {
        @Volatile
        private var appVisible: Boolean = false

        fun isAppVisible(): Boolean = appVisible
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Log.d(TAG, "onCreate")
        setImmersiveMode(false)

        // Register broadcast receiver
        breakReceiver = BreakReceiver()
        val filter = IntentFilter(BreakReminderService.ACTION_TRIGGER_BREAK)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            registerReceiver(breakReceiver, filter, RECEIVER_NOT_EXPORTED)
        } else {
            @Suppress("UnspecifiedRegisterReceiverFlag")
            registerReceiver(breakReceiver, filter)
        }

        // Check if we should show break screen
        if (intent.getBooleanExtra("show_break", false)) {
            showBreakScreen()
        }
    }

    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        this.webView = webView
        try {
            webView.addJavascriptInterface(AndroidBridge(), "AndroidBridge")
            Log.d(TAG, "AndroidBridge injected into WebView")
            syncBreakEngineSignals()
            logRustProbePhase("onWebViewCreate")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to inject AndroidBridge", e)
        }
    }

    // ----- JS bridge -----

    inner class AndroidBridge {
        @JavascriptInterface
        fun ping(): String {
            Log.d(TAG, "ping() called from JS")
            return "pong"
        }

        @JavascriptInterface
        fun startBreakService() {
            Log.d(TAG, "startBreakService called from JavaScript")
            startBreakServiceInternal()
        }

        @JavascriptInterface
        fun stopBreakService() {
            Log.d(TAG, "stopBreakService called from JavaScript")
            stopBreakServiceInternal()
        }

        @JavascriptInterface
        fun enterImmersiveMode() {
            setImmersiveMode(true)
        }

        @JavascriptInterface
        fun exitImmersiveMode() {
            setImmersiveMode(false)
        }

        @JavascriptInterface
        fun canUseFullScreenBreakAlerts(): Boolean {
            val notificationsEnabled = NotificationManagerCompat.from(this@MainActivity).areNotificationsEnabled()
            val fullScreenAllowed = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                val notificationManager = getSystemService(NotificationManager::class.java)
                notificationManager?.canUseFullScreenIntent() ?: false
            } else {
                true
            }
            return notificationsEnabled && fullScreenAllowed
        }

        @JavascriptInterface
        fun openFullScreenAlertSettings() {
            runOnUiThread {
                val intent = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                    Intent(Settings.ACTION_MANAGE_APP_USE_FULL_SCREEN_INTENT).apply {
                        data = android.net.Uri.parse("package:$packageName")
                    }
                } else {
                    Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).apply {
                        putExtra(Settings.EXTRA_APP_PACKAGE, packageName)
                    }
                }
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                startActivity(intent)
            }
        }

        @JavascriptInterface
        fun openNotificationSettings() {
            runOnUiThread {
                val intent = Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).apply {
                    putExtra(Settings.EXTRA_APP_PACKAGE, packageName)
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                startActivity(intent)
            }
        }

        @JavascriptInterface
        fun canDrawBreakOverlay(): Boolean {
            return Settings.canDrawOverlays(this@MainActivity)
        }

        @JavascriptInterface
        fun openOverlaySettings() {
            runOnUiThread {
                val intent = Intent(
                    Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                    android.net.Uri.parse("package:$packageName")
                ).apply {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                startActivity(intent)
            }
        }
    }

    private fun startBreakServiceInternal() {
        runOnUiThread {
            Log.d(TAG, "Starting break service")
            val intent = Intent(this, BreakReminderService::class.java).apply {
                action = BreakReminderService.ACTION_START
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                startForegroundService(intent)
            } else {
                startService(intent)
            }
        }
    }

    private fun stopBreakServiceInternal() {
        runOnUiThread {
            Log.d(TAG, "Stopping break service")
            val intent = Intent(this, BreakReminderService::class.java).apply {
                action = BreakReminderService.ACTION_STOP
            }
            startService(intent)
        }
    }

    // ----- lifecycle -----

    override fun onDestroy() {
        super.onDestroy()
        breakReceiver?.let { unregisterReceiver(it) }
    }

    override fun onResume() {
        super.onResume()
        appVisible = true
        setImmersiveMode(false)
        notifyBreakEngine(BreakEngineSignals.setFullscreenActiveScript(false))
    }

    override fun onPause() {
        appVisible = false
        super.onPause()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        Log.d(TAG, "onNewIntent show_break=${intent.getBooleanExtra("show_break", false)}")

        if (intent.getBooleanExtra("show_break", false)) {
            showBreakScreen()
        } else {
            setImmersiveMode(false)
        }
    }

    private fun showBreakScreen(attempt: Int = 0) {
        Log.d(TAG, "showBreakScreen called (attempt=$attempt)")
        runOnUiThread {
            val currentWebView = webView
            if (currentWebView != null) {
                val script = "window.location.href = 'break.html';"
                currentWebView.evaluateJavascript(script, null)
                Log.d(TAG, "Navigated to break.html")
                return@runOnUiThread
            }

            if (attempt < 50) {
                window.decorView.postDelayed({ showBreakScreen(attempt + 1) }, 100)
            } else {
                Log.e(TAG, "Failed to show break screen: WebView not found")
            }
        }
    }

    private fun syncBreakEngineSignals() {
        notifyBreakEngine(BreakEngineSignals.setFullscreenActiveScript(false))
    }

    private fun notifyBreakEngine(script: String) {
        runOnUiThread {
            webView?.evaluateJavascript(script, null)
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

    private fun setImmersiveMode(enabled: Boolean) {
        runOnUiThread {
            WindowCompat.setDecorFitsSystemWindows(window, !enabled)

            val controller = WindowInsetsControllerCompat(window, window.decorView)
            if (enabled) {
                controller.systemBarsBehavior =
                    WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                controller.isAppearanceLightStatusBars = false
                controller.isAppearanceLightNavigationBars = false
                controller.hide(WindowInsetsCompat.Type.systemBars())
            } else {
                window.statusBarColor = Color.WHITE
                window.navigationBarColor = Color.WHITE
                controller.isAppearanceLightStatusBars = true
                controller.isAppearanceLightNavigationBars = true
                controller.show(WindowInsetsCompat.Type.systemBars())
            }

            notifyBreakEngine(BreakEngineSignals.setFullscreenActiveScript(enabled))
        }
    }
}
