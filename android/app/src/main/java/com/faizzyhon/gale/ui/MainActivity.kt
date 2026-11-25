package com.faizzyhon.gale.ui

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.faizzyhon.gale.Gale
import com.faizzyhon.gale.engine.Links

class MainActivity : ComponentActivity() {
    /** Links handed to Gale by another app, shown in the Add sheet. */
    private var incoming by mutableStateOf<List<String>?>(null)

    private val pickTorrent = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri -> uri?.let { addTorrentFile(it) } }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT >= 33) registerForActivityResult(ActivityResultContracts.RequestPermission()) {}.launch(Manifest.permission.POST_NOTIFICATIONS)
        if (savedInstanceState == null) handle(intent) // not again after a rotation
        setContent {
            val prefs by Gale.settings.prefs.collectAsStateWithLifecycle()
            GaleTheme(prefs.theme) {
                App(
                    incoming = incoming,
                    onIncomingShown = { incoming = null },
                    pickTorrent = { pickTorrent.launch(arrayOf("application/x-bittorrent", "application/octet-stream")) },
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    private fun handle(intent: Intent?) {
        when (intent?.action) {
            Intent.ACTION_SEND -> {
                val text = listOfNotNull(intent.getStringExtra(Intent.EXTRA_TEXT), intent.getStringExtra(Intent.EXTRA_SUBJECT)).joinToString(" ")
                val links = Links.extract(text)
                @Suppress("DEPRECATION") val stream = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                when {
                    links.isNotEmpty() -> incoming = links
                    stream != null -> addTorrentFile(stream) // a .torrent file shared from a file manager or Telegram
                    else -> Toast.makeText(this, "No link found in what was shared", Toast.LENGTH_SHORT).show()
                }
            }
            Intent.ACTION_VIEW -> {
                val data = intent.data ?: return
                if (data.scheme == "magnet") incoming = listOf(data.toString())
                else addTorrentFile(data)
            }
        }
    }

    private fun addTorrentFile(uri: Uri) {
        val name = uri.lastPathSegment?.substringAfterLast('/')?.removeSuffix(".torrent") ?: "Torrent"
        val bytes = runCatching { contentResolver.openInputStream(uri)?.use { it.readNBytes((16 shl 20) + 1) } }.getOrNull()
        when {
            bytes == null -> Toast.makeText(this, "Could not read the file", Toast.LENGTH_SHORT).show()
            bytes.size > 16 shl 20 -> Toast.makeText(this, "The .torrent file is too large", Toast.LENGTH_SHORT).show()
            runCatching { com.faizzyhon.gale.engine.Torrents.info(bytes) }.isFailure ->
                Toast.makeText(this, "That is not a valid .torrent file", Toast.LENGTH_SHORT).show()
            else -> {
                Gale.add("torrent-file:$name", torrentBytes = bytes, name = name)
                Toast.makeText(this, "Torrent added", Toast.LENGTH_SHORT).show()
            }
        }
    }
}
