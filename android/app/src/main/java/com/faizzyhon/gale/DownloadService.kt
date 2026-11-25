package com.faizzyhon.gale

import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import com.faizzyhon.gale.ui.fmtBytes
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/** Keeps Gale running while something downloads, with one notification for all downloads. */
class DownloadService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private var wake: PowerManager.WakeLock? = null
    private var started = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        startForeground(Notifier.ONGOING_ID, Notifier.ongoing(this, "Starting…", null), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        if (intent?.action == PAUSE_ALL) Gale.pauseAll()
        if (!started) {
            started = true
            // The screen turning off must not stall downloads.
            wake = getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "gale:downloads").apply {
                setReferenceCounted(false)
                acquire(6 * 60 * 60 * 1000L)
            }
            scope.launch {
                // Polled, not collected: a download waiting for the network has no live numbers to emit.
                while (isActive) {
                    delay(1000) // also lets the download that started the service register first
                    if (Gale.activeCount() == 0 && !Gale.retrying()) {
                        stopSelf()
                        break
                    }
                    val live = Gale.live.value
                    val speed = live.values.sumOf { it.speed }
                    val one = live.values.singleOrNull()
                    val pct = one?.total?.takeIf { it > 0 }?.let { (one.done * 100 / it).toInt() }
                    val text = when {
                        live.isEmpty() -> "Waiting for the connection"
                        live.size == 1 -> {
                            val name = Gale.store.all.value.firstOrNull { it.id == live.keys.first() }?.name ?: "Downloading"
                            "$name · ${one?.stage ?: "${fmtBytes(speed)}/s"}"
                        }
                        else -> "${live.size} downloads · ${fmtBytes(speed)}/s"
                    }
                    getSystemService(NotificationManager::class.java).notify(Notifier.ONGOING_ID, Notifier.ongoing(this@DownloadService, text, pct))
                }
            }
        }
        return START_NOT_STICKY
    }

    /** Android 15 limits data-sync services to 6 hours a day; pause cleanly instead of being killed. */
    override fun onTimeout(startId: Int, fgsType: Int) {
        Gale.pauseAll()
        stopSelf()
    }

    override fun onDestroy() {
        scope.cancel()
        wake?.takeIf { it.isHeld }?.release()
        super.onDestroy()
    }

    companion object {
        const val PAUSE_ALL = "com.faizzyhon.gale.PAUSE_ALL"

        fun ensure(context: Context) {
            try {
                context.startForegroundService(Intent(context, DownloadService::class.java))
            } catch (e: Exception) {
                // Android refuses while the app is in the background; downloads still run, the service starts next time.
                Log.w("Gale", "foreground service not started: $e")
            }
        }
    }
}
