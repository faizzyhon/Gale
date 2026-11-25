package com.faizzyhon.gale

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.Uri
import com.faizzyhon.gale.data.Download
import com.faizzyhon.gale.data.Saver
import com.faizzyhon.gale.data.Settings
import com.faizzyhon.gale.data.Status
import com.faizzyhon.gale.data.Store
import com.faizzyhon.gale.engine.HttpEngine
import com.faizzyhon.gale.engine.LinkKind
import com.faizzyhon.gale.engine.Links
import com.faizzyhon.gale.engine.Media
import com.faizzyhon.gale.engine.Quality
import com.faizzyhon.gale.engine.Torrents
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.Request
import org.libtorrent4j.TorrentHandle
import java.io.File
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

/** Live numbers for a running download (kept out of the database). */
data class Live(val done: Long, val total: Long?, val speed: Long, val peers: Int = 0, val stage: String? = null)

/** Owns every download: queue, runners, progress. The UI and the service only read its state and call its verbs. */
object Gale {
    lateinit var app: Context
        private set
    lateinit var store: Store
        private set
    lateinit var settings: Settings
        private set

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val jobs = ConcurrentHashMap<Long, Job>()
    private val reported = ConcurrentHashMap<Long, Live>()
    private val counters = ConcurrentHashMap<Long, AtomicLong>()
    private val _live = MutableStateFlow<Map<Long, Live>>(emptyMap())
    val live: StateFlow<Map<Long, Live>> = _live
    val http = HttpEngine(online = { online() })
    /** Automatic retries used per download after a dropped connection; cleared on success or a manual resume. */
    private val retries = ConcurrentHashMap<Long, Int>()

    /** Instagram sign-in cookies (Netscape format) for yt-dlp; app-private, deleted on sign-out. */
    val cookies: File get() = File(app.filesDir, "instagram-cookies.txt")

    fun init(context: Context) {
        app = context.applicationContext
        store = Store(app)
        settings = Settings(app)
        store.reload()
        // Downloads that were running when Android stopped the app: carry on, or wait for a tap.
        val resume = settings.prefs.value.autoResume
        store.interrupted().forEach { store.update(it.copy(status = if (resume) Status.QUEUED else Status.PAUSED), refresh = false) }
        store.reload()
        watchNetwork()
        scope.launch { tick() }
        if (!BuildConfig.PLAY) scope.launch(Dispatchers.IO) { updateYtDlpIfOld() }
        pump()
    }

    /** Sites change often and the bundled yt-dlp ages; refresh it every few days (GitHub build only). */
    private suspend fun updateYtDlpIfOld() {
        val prefs = app.getSharedPreferences("gale", Context.MODE_PRIVATE)
        if (System.currentTimeMillis() - prefs.getLong("ytdlp_updated", 0) < 3 * 24 * 3600_000L) return
        if (!online()) return
        runCatching { Media.update(app) }.onSuccess { prefs.edit().putLong("ytdlp_updated", System.currentTimeMillis()).apply() }
    }

    fun online(): Boolean {
        val cm = app.getSystemService(ConnectivityManager::class.java)
        return cm.getNetworkCapabilities(cm.activeNetwork)?.hasCapability(android.net.NetworkCapabilities.NET_CAPABILITY_INTERNET) == true
    }

    /** True while a download waits for the connection to come back (the service stays up for it). */
    fun retrying() = retries.isNotEmpty() && store.all.value.any { it.status == Status.QUEUED && retries.containsKey(it.id) }

    // --- verbs ---------------------------------------------------------------------------------------------------

    /** Adds a link. [quality] is for video pages; [torrentBytes] for a .torrent file opened from the phone. */
    fun add(url: String, quality: Quality? = null, torrentBytes: ByteArray? = null, name: String? = null, headers: Map<String, String> = emptyMap()): Long {
        if (BuildConfig.PLAY && Links.isYouTube(url)) throw IllegalArgumentException(Links.YOUTUBE_BLOCKED)
        val kind = when {
            torrentBytes != null -> "torrent"
            else -> when (Links.kind(url) ?: throw IllegalArgumentException("Not a web address or magnet link")) {
                LinkKind.TORRENT -> "torrent"
                LinkKind.MEDIA -> if (quality?.height == null && quality != null) "audio" else "video"
                LinkKind.FILE -> if (quality != null) (if (quality.height == null) "audio" else "video") else "http"
            }
        }
        val title = name ?: when {
            Links.isMagnet(url) -> Links.magnetName(url) ?: "Torrent"
            kind == "http" -> Links.fileNameFromUrl(url) ?: "download"
            else -> Links.fileNameFromUrl(url) ?: "Video"
        }
        val q = quality?.let { if (it.height != null) it.height.toString() else if (it.mp3) "mp3" else "m4a" }
        val id = store.insert(Download(url = url.trim(), name = Links.safeName(title), kind = kind, quality = q, headers = headers))
        torrentBytes?.let { torrentFile(id).apply { parentFile?.mkdirs() }.writeBytes(it) }
        pump()
        return id
    }

    fun pause(id: Long) {
        store.get(id)?.let { if (it.status != Status.DONE) store.update(it.copy(status = Status.PAUSED)) }
        stop(id)
    }

    fun resume(id: Long) {
        retries.remove(id)
        store.get(id)?.takeIf { it.status in setOf(Status.PAUSED, Status.FAILED) }?.let { store.update(it.copy(status = Status.QUEUED, error = null)) }
        pump()
    }

    fun pauseAll() = store.all.value.filter { it.status in setOf(Status.RUNNING, Status.QUEUED, Status.WAITING_WIFI) }.forEach { pause(it.id) }

    /** Removes from the list; with [deleteFile] also the saved file (a torrent's folder files are left in place). */
    fun remove(id: Long, deleteFile: Boolean) {
        val d = store.get(id) ?: return
        stop(id)
        scope.launch(Dispatchers.IO) {
            jobs[id]?.join()
            work(id).deleteRecursively()
            torrentFile(id).delete()
            if (deleteFile) d.savedUri?.takeIf { it.startsWith("content:") }?.let { runCatching { app.contentResolver.delete(Uri.parse(it), null, null) } }
        }
        store.delete(id)
    }

    fun activeCount() = jobs.size

    // --- queue -----------------------------------------------------------------------------------------------------

    @Synchronized
    fun pump() {
        val p = settings.prefs.value
        val waiting = store.all.value.filter { it.status == Status.QUEUED || it.status == Status.WAITING_WIFI }.sortedBy { it.id }
        if (!online()) return // started again by the network callback
        val metered = (app.getSystemService(ConnectivityManager::class.java)).isActiveNetworkMetered
        for (d in waiting) {
            if (jobs.size >= p.maxActive) break
            if (p.wifiOnly && metered) {
                if (d.status != Status.WAITING_WIFI) store.update(d.copy(status = Status.WAITING_WIFI))
                continue
            }
            start(d)
        }
    }

    private fun start(d0: Download) {
        val d = d0.copy(status = Status.RUNNING, error = null)
        store.update(d)
        counters[d.id] = AtomicLong(0)
        DownloadService.ensure(app)
        jobs[d.id] = scope.launch(Dispatchers.IO) {
            try {
                val result = when (d.kind) {
                    "torrent" -> runTorrent(d)
                    "video", "audio" -> runMedia(d)
                    else -> runHttp(d)
                }
                val done = result.copy(status = Status.DONE, finishedAt = System.currentTimeMillis(), error = null)
                store.update(done)
                retries.remove(d.id)
                Notifier.done(app, done)
            } catch (e: CancellationException) {
                // paused or removed; the caller already recorded the new status
            } catch (e: Throwable) {
                val n = (retries[d.id] ?: 0) + 1
                if (transient(e) && n <= 8) {
                    // A dropped connection is not a failure: keep the progress and try again (after the network is back).
                    retries[d.id] = n
                    store.get(d.id)?.takeIf { it.status == Status.RUNNING }?.let { store.update(it.copy(status = Status.QUEUED, error = "Connection lost, trying again")) }
                    delay(5000L * n)
                } else {
                    retries.remove(d.id)
                    val cur = store.get(d.id)
                    if (cur != null && cur.status in setOf(Status.RUNNING, Status.QUEUED)) {
                        val msg = friendly(e)
                        store.update(cur.copy(status = Status.FAILED, error = msg))
                        Notifier.failed(app, cur, msg)
                    }
                }
            } finally {
                jobs.remove(d.id)
                reported.remove(d.id)
                counters.remove(d.id)
                pump()
            }
        }
    }

    private fun stop(id: Long) {
        jobs[id]?.cancel()
        Media.cancel(id)
    }

    // --- runners ---------------------------------------------------------------------------------------------------

    private fun work(id: Long) = File(app.getExternalFilesDir(null) ?: app.filesDir, "work/$id")
    private fun torrentFile(id: Long) = File(app.filesDir, "torrents/$id.torrent")

    private suspend fun runHttp(d0: Download): Download {
        var d = d0
        if (d.segments.isEmpty()) {
            reported[d.id] = Live(0, null, 0, stage = "Connecting")
            val p = http.probe(d.url, headersFor(d))
            val name = Links.safeName(p.fileName ?: d.name)
            d = d.copy(
                name = name, total = p.total, resumable = p.ranges, mime = p.mime,
                segments = HttpEngine.plan(p.total, p.ranges, settings.prefs.value.connections),
            )
            store.update(d)
        }
        val file = File(work(d.id), "data")
        counters[d.id]!!.set(d.segments.sumOf { it.done })
        val segs = d.segments.map { it.copy() }
        http.download(d.url, file, segs, headersFor(d), onBytes = { counters[d.id]?.addAndGet(it.toLong()) }) { s ->
            store.progress(d.id, s.sumOf { it.done }, d.total, s)
        }
        val named = File(file.parentFile, d.name)
        file.renameTo(named)
        val uri = Saver.publish(app, named)
        work(d.id).deleteRecursively()
        return d.copy(done = d.total ?: named.length(), total = d.total ?: named.length(), savedUri = uri.toString(), segments = segs)
    }

    /** The download's own headers plus the browser's cookies for that site, read fresh (never stored with the download). */
    private fun headersFor(d: Download): Map<String, String> {
        val cookie = runCatching { android.webkit.CookieManager.getInstance().getCookie(d.url) }.getOrNull()
        return if (cookie.isNullOrBlank()) d.headers else d.headers + ("Cookie" to cookie)
    }

    private suspend fun runMedia(d: Download): Download {
        val q = when (d.quality) {
            "mp3" -> Quality("MP3", null, mp3 = true)
            "m4a" -> Quality("M4A", null)
            else -> Quality("", d.quality?.toIntOrNull() ?: 100_000)
        }
        reported[d.id] = Live(0, null, 0, stage = "Reading the page")
        val dir = work(d.id)
        val files = Media.download(app, d.id, d.url, q, dir, cookies.takeIf { it.exists() }) { f, total, speed ->
            reported[d.id] = Live(((total ?: 0) * f).toLong(), total, speed, stage = if (f >= 1f) "Finishing" else null)
        }
        if (files.isEmpty()) throw IllegalStateException("Nothing to download on this page")
        val total = files.sumOf { it.length() }
        val uris = files.sortedBy { it.name }.map { Saver.publish(app, it) }
        dir.deleteRecursively()
        val name = if (files.size == 1) files[0].name else "${d.name} (${files.size} files)"
        return d.copy(name = name, total = total, done = total, savedUri = uris.first().toString())
    }

    private suspend fun runTorrent(d0: Download): Download {
        var d = d0
        val tf = torrentFile(d.id)
        if (!tf.exists()) {
            val bytes = when {
                Links.isMagnet(d.url) -> {
                    reported[d.id] = Live(0, null, 0, stage = "Getting torrent info from peers")
                    var b: ByteArray? = null
                    while (b == null) {
                        b = Torrents.metadata(d.url, 60, File(app.cacheDir, "magnet"))
                        currentCoroutineContext().ensureActive() // paused while waiting for peers
                    }
                    b
                }
                else -> fetchTorrentFile(d)
            }
            tf.parentFile?.mkdirs()
            tf.writeBytes(bytes)
        }
        val bytes = tf.readBytes()
        val info = Torrents.info(bytes)
        if (d.total == null) {
            d = d.copy(name = Links.safeName(info.name()), total = info.totalSize())
            store.update(d)
        }
        val dir = work(d.id)
        var h: TorrentHandle? = null
        try {
            h = Torrents.add(bytes, dir)
            while (true) {
                val s = Torrents.stats(h)
                s.error?.let { throw IllegalStateException(it) }
                reported[d.id] = Live(s.done, s.total, s.speed, s.peers, stage = if (s.checking) "Checking files" else null)
                if (!s.checking) store.progress(d.id, s.done, s.total, emptyList())
                if (s.finished) break
                delay(1000)
            }
        } finally {
            h?.let { Torrents.remove(it, deleteFiles = false) } // never seed after finishing or while paused
        }
        reported[d.id] = Live(d.total ?: 0, d.total, 0, stage = "Saving")
        val uris = Saver.publishTree(app, dir, "")
        dir.deleteRecursively()
        tf.delete()
        return d.copy(done = d.total ?: 0, savedUri = if (uris.size == 1) uris[0].toString() else "folder:Download/Gale/${d.name}")
    }

    private suspend fun fetchTorrentFile(d: Download): ByteArray = withContext(Dispatchers.IO) {
        http.client.newCall(Request.Builder().url(d.url).build()).execute().use { r ->
            if (!r.isSuccessful) throw com.faizzyhon.gale.engine.HttpError(r.code)
            val len = r.body.contentLength()
            if (len > 16L shl 20) throw IllegalStateException("The .torrent file is too large")
            r.body.bytes().also { if (it.size > 16 shl 20) throw IllegalStateException("The .torrent file is too large") }
        }
    }

    // --- progress --------------------------------------------------------------------------------------------------

    /** Once a second: turn byte counters into speeds, publish live numbers, refresh the list. */
    private suspend fun tick() {
        val last = HashMap<Long, Long>()
        while (true) {
            delay(1000)
            val now = HashMap<Long, Live>()
            for (id in jobs.keys) {
                val c = counters[id]
                val r = reported[id]
                now[id] = if (c != null && c.get() > 0 && (r == null || r.stage == "Connecting")) {
                    val bytes = c.get()
                    val speed = (bytes - (last[id] ?: bytes)).coerceAtLeast(0)
                    last[id] = bytes
                    Live(bytes, store.all.value.firstOrNull { it.id == id }?.total, speed)
                } else r ?: Live(0, null, 0, stage = "Starting")
            }
            last.keys.retainAll(jobs.keys)
            _live.value = now
            if (now.isNotEmpty()) store.reload()
        }
    }

    private fun watchNetwork() {
        app.getSystemService(ConnectivityManager::class.java).registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) { scope.launch { delay(1500); pump() } }
            override fun onCapabilitiesChanged(network: Network, caps: android.net.NetworkCapabilities) { scope.launch { pump() } }
        })
    }

    private val NETWORK_WORDS = Regex("(?i)timed out|connection|network|unreachable|temporar|reset by peer|unable to download webpage|eof")

    /** Worth retrying: the network, a busy server, or anything while the phone is offline. Not 403/404 and the like. */
    private fun transient(e: Throwable): Boolean = when {
        e is com.faizzyhon.gale.engine.HttpError -> e.status !in 400..499 || e.status == 408 || e.status == 429
        e is java.io.IOException -> true
        !online() -> true
        else -> NETWORK_WORDS.containsMatchIn(e.message.orEmpty())
    }

    fun friendly(e: Throwable): String = when (e) {
        is com.faizzyhon.gale.engine.HttpError -> when (e.status) {
            403 -> "The server refused the download (403). The link may have expired."
            404 -> "The file is not on the server any more (404)."
            else -> e.message ?: "Server error"
        }
        is java.net.UnknownHostException -> "No internet connection, or the site's address is wrong."
        is java.net.SocketTimeoutException -> "The server stopped answering."
        is java.io.IOException -> e.message ?: "Network error"
        else -> (e.message ?: e.javaClass.simpleName).lineSequence().lastOrNull { it.contains("ERROR") }?.substringAfter("ERROR:")?.trim()
            ?: (e.message ?: e.javaClass.simpleName).take(300)
    }
}
