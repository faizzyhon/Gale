package com.faizzyhon.gale.engine

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.io.IOException
import java.io.RandomAccessFile
import java.util.concurrent.TimeUnit
import kotlin.coroutines.coroutineContext

/** One byte range of a file. `end` is inclusive; -1 means "until the server stops" (size unknown). */
data class Segment(val start: Long, val end: Long, var done: Long = 0) {
    val remaining get() = if (end < 0) Long.MAX_VALUE else end - start + 1 - done
    val finished get() = end >= 0 && remaining <= 0
}

data class Probe(val finalUrl: String, val total: Long?, val ranges: Boolean, val fileName: String?, val mime: String?)

class HttpError(val status: Int) : IOException("The server answered $status")

class HttpEngine(
    val client: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(20, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .build(),
    /** False while the phone has no internet: segments then wait instead of using up their retries. */
    private val online: () -> Boolean = { true },
) {
    /** One GET for byte 0 tells size, name and whether the server can send parts (206 + Content-Range). */
    suspend fun probe(url: String, headers: Map<String, String> = emptyMap()): Probe = withContext(Dispatchers.IO) {
        val req = Request.Builder().url(url).header("Range", "bytes=0-0").apply { headers.forEach { (k, v) -> header(k, v) } }.build()
        client.newCall(req).execute().use { r ->
            if (!r.isSuccessful) throw HttpError(r.code)
            val ranged = r.code == 206
            val total = if (ranged) r.header("Content-Range")?.substringAfter('/')?.toLongOrNull()
            else r.body.contentLength().takeIf { it >= 0 }
            Probe(
                finalUrl = r.request.url.toString(),
                total = total,
                ranges = ranged && total != null,
                fileName = Links.fileNameFromDisposition(r.header("Content-Disposition")) ?: Links.fileNameFromUrl(r.request.url.toString()),
                mime = r.header("Content-Type")?.substringBefore(';')?.trim(),
            )
        }
    }

    /**
     * Downloads into [file], resuming every segment from its `done` count. [onBytes] is called for every chunk written
     * (from several threads); [checkpoint] is called with the segments about once a second so they can be saved.
     */
    suspend fun download(
        url: String,
        file: File,
        segments: List<Segment>,
        headers: Map<String, String> = emptyMap(),
        onBytes: (Int) -> Unit,
        checkpoint: (List<Segment>) -> Unit,
    ) = withContext(Dispatchers.IO) {
        file.parentFile?.mkdirs()
        RandomAccessFile(file, "rw").use { raf ->
            val lock = Any()
            var lastSave = System.currentTimeMillis()
            coroutineScope {
                for (seg in segments.filter { !it.finished }) launch {
                    fetchSegment(url, raf, seg, headers, lock) { n ->
                        onBytes(n)
                        val now = System.currentTimeMillis()
                        if (now - lastSave > 1000) synchronized(lock) {
                            if (now - lastSave > 1000) { lastSave = now; checkpoint(segments.map { it.copy() }) }
                        }
                    }
                }
            }
            checkpoint(segments.map { it.copy() })
            val expected = segments.lastOrNull()?.end?.takeIf { it >= 0 }?.plus(1)
            if (expected != null && raf.length() != expected) raf.setLength(expected)
        }
    }

    private suspend fun fetchSegment(url: String, raf: RandomAccessFile, seg: Segment, headers: Map<String, String>, lock: Any, onBytes: (Int) -> Unit) {
        var attempt = 0
        while (true) {
            try {
                readRange(url, raf, seg, headers, lock, onBytes)
                return
            } catch (e: CancellationException) {
                throw e
            } catch (e: IOException) {
                // 4xx other than 408/429 will not get better by retrying.
                if (e is HttpError && e.status in 400..499 && e.status != 408 && e.status != 429) throw e
                if (!online()) {
                    while (!online()) delay(2000) // Wi-Fi to mobile data, a tunnel, flight mode: wait it out
                    continue
                }
                if (++attempt > 8) throw e
                delay(1000L shl (attempt - 1).coerceAtMost(4))
            }
        }
    }

    private suspend fun readRange(url: String, raf: RandomAccessFile, seg: Segment, headers: Map<String, String>, lock: Any, onBytes: (Int) -> Unit) {
        val from = seg.start + seg.done
        val req = Request.Builder().url(url).apply {
            headers.forEach { (k, v) -> header(k, v) }
            if (seg.end >= 0) header("Range", "bytes=$from-${seg.end}")
        }.build()
        val call = client.newCall(req)
        val job = coroutineContext[kotlinx.coroutines.Job]
        val handle = job?.invokeOnCompletion { call.cancel() } // a pause closes the connection at once
        try {
            call.execute().use { r ->
                if (!r.isSuccessful) throw HttpError(r.code)
                if (seg.end >= 0) {
                    // The server must send exactly the part we asked for; anything else would corrupt the file.
                    val cr = r.header("Content-Range")
                    if (r.code != 206 || cr == null || !cr.startsWith("bytes $from-")) throw IOException("Server ignored the requested range")
                } else if (seg.done > 0) {
                    seg.done = 0 // size unknown: no resume possible, start over
                    synchronized(lock) { raf.setLength(0) }
                }
                val src = r.body.byteStream()
                val buf = ByteArray(64 * 1024)
                while (true) {
                    coroutineContext.ensureActive()
                    val max = if (seg.end >= 0) minOf(buf.size.toLong(), seg.remaining).toInt() else buf.size
                    if (max <= 0) break
                    val n = src.read(buf, 0, max)
                    if (n < 0) break
                    synchronized(lock) {
                        raf.seek(seg.start + seg.done)
                        raf.write(buf, 0, n)
                    }
                    seg.done += n
                    onBytes(n)
                }
                if (seg.end >= 0 && !seg.finished) throw IOException("Connection closed early")
            }
        } finally {
            handle?.dispose()
        }
    }

    companion object {
        const val MIN_SEGMENT = 1L shl 20 // 1 MiB: smaller parts cost more in requests than they gain

        fun plan(total: Long?, ranges: Boolean, connections: Int): List<Segment> {
            if (total == null || total <= 0 || !ranges) return listOf(Segment(0, if (total != null && total > 0 && ranges) total - 1 else -1))
            val n = connections.coerceIn(1, 32).toLong().coerceAtMost(maxOf(1, total / MIN_SEGMENT))
            val size = total / n
            return (0 until n).map { i ->
                val start = i * size
                Segment(start, if (i == n - 1) total - 1 else start + size - 1)
            }
        }
    }
}
