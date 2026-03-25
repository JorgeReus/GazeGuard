package com.reus.gazeguard

import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {
    private var breakReceiver: BreakReceiver? = null
    private var webView: WebView? = null
    private val TAG = "MainActivity"

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Log.d(TAG, "onCreate")

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

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)

        if (intent.getBooleanExtra("show_break", false)) {
            showBreakScreen()
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

    private fun setImmersiveMode(enabled: Boolean) {
        runOnUiThread {
            WindowCompat.setDecorFitsSystemWindows(window, !enabled)

            val controller = WindowInsetsControllerCompat(window, window.decorView)
            if (enabled) {
                controller.systemBarsBehavior =
                    WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
                controller.hide(WindowInsetsCompat.Type.systemBars())
            } else {
                controller.show(WindowInsetsCompat.Type.systemBars())
            }
        }
    }
}
