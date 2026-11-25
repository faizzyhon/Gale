package com.faizzyhon.gale.engine

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.libtorrent4j.SessionManager
import org.libtorrent4j.SessionHandle
import org.libtorrent4j.TorrentHandle
import org.libtorrent4j.TorrentInfo
import org.libtorrent4j.swig.add_torrent_params
import org.libtorrent4j.swig.error_code
import java.io.File

data class TorrentStats(val done: Long, val total: Long, val speed: Long, val peers: Int, val finished: Boolean, val checking: Boolean, val error: String?)

/**
 * libtorrent through libtorrent4j. One session for the whole app, started on the first torrent. Gale keeps the
 * .torrent data itself; pausing removes the torrent from the session and resuming adds it again, letting libtorrent
 * check the pieces already on disk (the same approach as the desktop app). Nothing is shared after completion.
 */
object Torrents {
    private val session by lazy { SessionManager(false).also { it.start() } }

    /** Gets a magnet link's metadata (the .torrent data) from peers. Null if none answered within [seconds]. */
    suspend fun metadata(magnet: String, seconds: Int, tmp: File): ByteArray? = withContext(Dispatchers.IO) {
        tmp.mkdirs()
        session.fetchMagnet(magnet, seconds, tmp)
    }

    fun info(bytes: ByteArray) = TorrentInfo(bytes)

    fun add(bytes: ByteArray, dir: File): TorrentHandle {
        dir.mkdirs()
        val ti = TorrentInfo(bytes)
        session.find(ti.infoHash())?.takeIf { it.isValid }?.let { return it }
        // SessionManager.download() adds in the background, so the torrent may not exist yet when it returns.
        val p = add_torrent_params().apply { set_ti(ti.swig()); setSave_path(dir.absolutePath) }
        val ec = error_code()
        val th = session.swig().add_torrent(p, ec)
        if (ec.value() != 0 || !th.is_valid()) throw IllegalStateException("Could not start the torrent: ${ec.message()}")
        return TorrentHandle(th).also { it.resume() }
    }

    fun stats(h: TorrentHandle): TorrentStats {
        val s = h.status()
        val err = s.errorCode()?.takeIf { it.isError }?.message
        val state = s.state().name
        return TorrentStats(
            done = s.totalWantedDone(), total = s.totalWanted(), speed = s.downloadRate().toLong(), peers = s.numPeers(),
            finished = s.isFinished || s.isSeeding, checking = state.startsWith("CHECKING"), error = err,
        )
    }

    fun remove(h: TorrentHandle, deleteFiles: Boolean) {
        if (deleteFiles) session.remove(h, SessionHandle.DELETE_FILES) else session.remove(h)
    }
}
