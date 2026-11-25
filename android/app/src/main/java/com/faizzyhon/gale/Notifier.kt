package com.faizzyhon.gale

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import com.faizzyhon.gale.data.Download
import com.faizzyhon.gale.ui.MainActivity
import com.faizzyhon.gale.ui.fmtBytes

object Notifier {
    const val ACTIVE = "active"
    private const val FINISHED = "finished"
    const val ONGOING_ID = 1

    fun channels(context: Context) {
        val nm = context.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(ACTIVE, "Downloads in progress", NotificationManager.IMPORTANCE_LOW))
        nm.createNotificationChannel(NotificationChannel(FINISHED, "Finished downloads", NotificationManager.IMPORTANCE_DEFAULT))
    }

    private fun openApp(context: Context) = PendingIntent.getActivity(
        context, 0, Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE,
    )

    fun ongoing(context: Context, text: String, progress: Int?): Notification =
        NotificationCompat.Builder(context, ACTIVE)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(context.getString(R.string.app_name))
            .setContentText(text)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(openApp(context))
            .apply { if (progress != null) setProgress(100, progress, false) else setProgress(0, 0, true) }
            .addAction(0, "Pause all", PendingIntent.getService(
                context, 1, Intent(context, DownloadService::class.java).setAction(DownloadService.PAUSE_ALL), PendingIntent.FLAG_IMMUTABLE,
            ))
            .build()

    fun done(context: Context, d: Download) {
        val open = d.savedUri?.takeIf { it.startsWith("content:") }?.let {
            PendingIntent.getActivity(
                context, d.id.toInt(),
                Intent(Intent.ACTION_VIEW).setDataAndType(Uri.parse(it), d.mime ?: com.faizzyhon.gale.data.Saver.mimeOf(d.name))
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
                PendingIntent.FLAG_IMMUTABLE,
            )
        } ?: openApp(context)
        post(context, d.id, "Download complete", "${d.name} · ${fmtBytes(d.total ?: d.done)}", open)
    }

    fun failed(context: Context, d: Download, why: String) = post(context, d.id, "Download failed: ${d.name}", why, openApp(context))

    private fun post(context: Context, id: Long, title: String, text: String, tap: PendingIntent) {
        if (ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) return
        val n = NotificationCompat.Builder(context, FINISHED)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setAutoCancel(true)
            .setContentIntent(tap)
            .build()
        context.getSystemService(NotificationManager::class.java).notify(1000 + id.toInt(), n)
    }
}
