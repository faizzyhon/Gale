package com.faizzyhon.gale.data

import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteOpenHelper
import com.faizzyhon.gale.engine.Segment
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

enum class Status { QUEUED, RUNNING, PAUSED, DONE, FAILED, WAITING_WIFI }

data class Download(
    val id: Long = 0,
    val url: String,
    val name: String,
    /** http, video, audio or torrent */
    val kind: String,
    val status: Status = Status.QUEUED,
    val total: Long? = null,
    val done: Long = 0,
    val error: String? = null,
    val createdAt: Long = System.currentTimeMillis(),
    val finishedAt: Long? = null,
    /** content:// address of the finished file (or folder for multi-file torrents) */
    val savedUri: String? = null,
    val mime: String? = null,
    /** yt-dlp quality: height, "m4a" or "mp3" */
    val quality: String? = null,
    val segments: List<Segment> = emptyList(),
    val resumable: Boolean = false,
    val headers: Map<String, String> = emptyMap(),
)

/** SQLite with no generated code; small enough that a list re-read after each change is cheap. */
class Store(context: Context) : SQLiteOpenHelper(context, "gale.db", null, 1) {
    private val _all = MutableStateFlow<List<Download>>(emptyList())
    val all: StateFlow<List<Download>> = _all

    override fun onCreate(db: SQLiteDatabase) {
        db.execSQL(
            """CREATE TABLE downloads (
                id INTEGER PRIMARY KEY AUTOINCREMENT, url TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL,
                status TEXT NOT NULL, total INTEGER, done INTEGER NOT NULL DEFAULT 0, error TEXT,
                created_at INTEGER NOT NULL, finished_at INTEGER, saved_uri TEXT, mime TEXT, quality TEXT,
                segments TEXT NOT NULL DEFAULT '', resumable INTEGER NOT NULL DEFAULT 0, headers TEXT NOT NULL DEFAULT '')""",
        )
    }

    override fun onUpgrade(db: SQLiteDatabase, oldVersion: Int, newVersion: Int) = Unit

    fun reload() {
        _all.value = readableDatabase.rawQuery("SELECT * FROM downloads ORDER BY id DESC", null).use { c ->
            generateSequence { if (c.moveToNext()) read(c) else null }.toList()
        }
    }

    fun get(id: Long): Download? = readableDatabase.rawQuery("SELECT * FROM downloads WHERE id = ?", arrayOf(id.toString())).use {
        if (it.moveToFirst()) read(it) else null
    }

    fun insert(d: Download): Long = writableDatabase.insert("downloads", null, values(d)).also { reload() }

    fun update(d: Download, refresh: Boolean = true) {
        writableDatabase.update("downloads", values(d), "id = ?", arrayOf(d.id.toString()))
        if (refresh) reload()
    }

    /** Progress only: never touches status, so a pause recorded meanwhile is not overwritten. */
    fun progress(id: Long, done: Long, total: Long?, segments: List<Segment>) {
        writableDatabase.update("downloads", ContentValues().apply {
            put("done", done); put("total", total)
            if (segments.isNotEmpty()) put("segments", encodeSegments(segments))
        }, "id = ?", arrayOf(id.toString()))
    }

    fun delete(id: Long) {
        writableDatabase.delete("downloads", "id = ?", arrayOf(id.toString()))
        reload()
    }

    /** Rows left running when the app was killed. */
    fun interrupted(): List<Download> = _all.value.filter { it.status == Status.RUNNING }

    private fun values(d: Download) = ContentValues().apply {
        put("url", d.url); put("name", d.name); put("kind", d.kind); put("status", d.status.name)
        put("total", d.total); put("done", d.done); put("error", d.error); put("created_at", d.createdAt)
        put("finished_at", d.finishedAt); put("saved_uri", d.savedUri); put("mime", d.mime); put("quality", d.quality)
        put("segments", encodeSegments(d.segments)); put("resumable", if (d.resumable) 1 else 0)
        put("headers", d.headers.entries.joinToString("\n") { "${it.key}: ${it.value}" })
    }

    private fun read(c: Cursor): Download {
        fun s(n: String) = c.getString(c.getColumnIndexOrThrow(n))
        fun l(n: String) = c.getColumnIndexOrThrow(n).let { if (c.isNull(it)) null else c.getLong(it) }
        return Download(
            id = l("id")!!, url = s("url"), name = s("name"), kind = s("kind"),
            status = runCatching { Status.valueOf(s("status")) }.getOrDefault(Status.PAUSED),
            total = l("total"), done = l("done") ?: 0, error = s("error"), createdAt = l("created_at")!!,
            finishedAt = l("finished_at"), savedUri = s("saved_uri"), mime = s("mime"), quality = s("quality"),
            segments = decodeSegments(s("segments")), resumable = l("resumable") == 1L,
            headers = s("headers").lines().filter { ':' in it }.associate { it.substringBefore(':').trim() to it.substringAfter(':').trim() },
        )
    }

    companion object {
        fun encodeSegments(s: List<Segment>) = s.joinToString(";") { "${it.start},${it.end},${it.done}" }
        fun decodeSegments(s: String?): List<Segment> = s.orEmpty().split(';').filter { it.isNotBlank() }.mapNotNull {
            val p = it.split(',').mapNotNull(String::toLongOrNull)
            if (p.size == 3) Segment(p[0], p[1], p[2]) else null
        }
    }
}
