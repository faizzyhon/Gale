package com.faizzyhon.gale.engine

import android.content.Context
import com.yausername.ffmpeg.FFmpeg
import com.yausername.youtubedl_android.YoutubeDL
import com.yausername.youtubedl_android.YoutubeDLRequest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File

data class MediaInfo(val title: String, val items: Int, val qualities: List<Quality>)

/** yt-dlp (with its own Python and FFmpeg) through youtubedl-android. */
object Media {
    @Volatile private var ready = false

    fun init(context: Context) {
        if (ready) return
        synchronized(this) {
            if (ready) return
            YoutubeDL.getInstance().init(context)
            FFmpeg.getInstance().init(context)
            ready = true
        }
    }

    private fun base(url: String, cookies: File?) = YoutubeDLRequest(url).apply {
        addOption("--no-warnings")
        addOption("--no-mtime")
        cookies?.takeIf { it.exists() }?.let { addOption("--cookies", it.absolutePath) }
    }

    suspend fun probe(context: Context, url: String, cookies: File?): MediaInfo = withContext(Dispatchers.IO) {
        init(context)
        val req = base(url, cookies).apply {
            addOption("--dump-single-json")
            addOption("--skip-download")
        }
        val json = JSONObject(YoutubeDL.getInstance().execute(req).out)
        parse(json)
    }

    /** Pure part of [probe], kept separate so it can be tested on the JVM. */
    fun parse(json: JSONObject): MediaInfo {
        val entries = json.optJSONArray("entries")
        val first = if (entries != null && entries.length() > 0) entries.optJSONObject(0) else json
        val fmts = first?.optJSONArray("formats")
        val list = (0 until (fmts?.length() ?: 0)).map { i ->
            val f = fmts!!.getJSONObject(i)
            val h = listOf(f.optInt("height", 0), f.optInt("width", 0)).filter { it > 0 }.minOrNull()
            // Some extractors (Instagram among them) leave the codecs out: then height decides, and audio is assumed.
            fun codec(k: String) = if (f.isNull(k) || !f.has(k)) null else f.getString(k)
            val v = codec("vcodec"); val a = codec("acodec")
            Fmt(
                height = h,
                hasVideo = if (v != null) v != "none" else h != null,
                hasAudio = if (a != null) a != "none" else v == null || v != "none",
                size = f.optLong("filesize", 0).takeIf { it > 0 } ?: f.optLong("filesize_approx", 0).takeIf { it > 0 },
            )
        }
        var q = Qualities.from(list)
        if (q.isEmpty()) q = listOf(Quality("Best", 100_000)) // single-file sites list no formats
        val count = entries?.length() ?: 1
        if (count > 1) q = q.map { it.copy(size = null) } // sizes are for the first item only
        return MediaInfo(json.optString("title").ifBlank { first?.optString("title").orEmpty() }.ifBlank { "Video" }, count, q)
    }

    /** Runs yt-dlp into [dir]; returns the files it made. [onProgress] gets (fraction 0..1, total bytes or null, bytes/s). */
    suspend fun download(
        context: Context, id: Long, url: String, quality: Quality, dir: File, cookies: File?,
        onProgress: (Float, Long?, Long) -> Unit,
    ): List<File> = withContext(Dispatchers.IO) {
        init(context)
        dir.mkdirs()
        val req = base(url, cookies).apply {
            // Stories and carousels are playlists: number the files so they don't overwrite each other.
            addOption("-o", File(dir, "%(title).90B%(playlist_index& - {}|)s.%(ext)s").absolutePath)
            addOption("--windows-filenames")
            addOption("--newline")
            quality.args().chunked(2).forEach { p -> if (p.size == 2) addOption(p[0], p[1]) else addOption(p[0]) }
        }
        YoutubeDL.getInstance().execute(req, id.toString()) { pct, _, line ->
            val (total, speed) = parseLine(line)
            onProgress((pct / 100f).coerceIn(0f, 1f), total, speed)
        }
        dir.listFiles { f -> f.isFile && !f.name.endsWith(".part") && !f.name.endsWith(".ytdl") }.orEmpty().toList()
    }

    fun cancel(id: Long) = runCatching { YoutubeDL.getInstance().destroyProcessById(id.toString()) }

    suspend fun update(context: Context): String = withContext(Dispatchers.IO) {
        init(context)
        YoutubeDL.getInstance().updateYoutubeDL(context, YoutubeDL.UpdateChannel.STABLE)?.name ?: "UNKNOWN"
    }

    private val LINE = Regex("""([\d.]+)%\s+of\s+~?\s*([\d.]+)([KMG]i?B)(?:.*?at\s+([\d.]+)([KMG]i?B)/s)?""")

    /** "[download]  45.3% of ~ 12.34MiB at  1.20MiB/s ETA 00:09" → (12.34 MiB, 1.2 MiB/s). */
    fun parseLine(line: String): Pair<Long?, Long> {
        val m = LINE.find(line) ?: return null to 0L
        fun bytes(n: String, u: String): Long = (n.toDoubleOrNull() ?: 0.0).times(
            when (u.first()) { 'K' -> 1024.0; 'M' -> 1024.0 * 1024; 'G' -> 1024.0 * 1024 * 1024; else -> 1.0 },
        ).toLong()
        val total = bytes(m.groupValues[2], m.groupValues[3])
        val speed = if (m.groupValues[4].isNotEmpty()) bytes(m.groupValues[4], m.groupValues[5]) else 0L
        return total to speed
    }
}
