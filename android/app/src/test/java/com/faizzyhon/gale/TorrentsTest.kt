package com.faizzyhon.gale

import com.faizzyhon.gale.engine.Torrents
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.libtorrent4j.SessionManager
import org.libtorrent4j.SessionParams
import org.libtorrent4j.SettingsPack
import org.libtorrent4j.TcpEndpoint
import org.libtorrent4j.TorrentBuilder
import org.libtorrent4j.TorrentInfo
import java.io.File
import java.nio.file.Files
import kotlin.random.Random

/** A seeder session on 127.0.0.1 and Gale's torrent engine downloading a two-file torrent from it. */
class TorrentsTest {
    companion object {
        /** The Linux build of libtorrent4j ships its .so inside the jar; point libtorrent4j at an extracted copy. */
        @JvmStatic @org.junit.BeforeClass fun loadNative() {
            val so = File.createTempFile("libtorrent4j", ".so").apply { deleteOnExit() }
            TorrentsTest::class.java.classLoader!!.getResourceAsStream("lib/x86_64/libtorrent4j.so")!!.use { i -> so.outputStream().use { i.copyTo(it) } }
            System.setProperty("libtorrent4j.jni.path", so.absolutePath)
        }
    }

    @Test fun downloadsAMultiFileTorrentFromALocalSeeder() {
        val root = Files.createTempDirectory("gale-torrent").toFile()
        val pack = File(root, "seed/Pack").apply { File(this, "sub").mkdirs() }
        val a = Random(1).nextBytes(700_000)
        val b = Random(2).nextBytes(300_000)
        File(pack, "a.bin").writeBytes(a)
        File(pack, "sub/b.bin").writeBytes(b)
        val bytes = TorrentBuilder().path(pack).pieceSize(16 * 1024).generate().entry().bencode()

        val seeder = SessionManager(false)
        seeder.start(SessionParams(SettingsPack().listenInterfaces("127.0.0.1:46881").apply { setEnableDht(false) }))
        try {
            seeder.download(TorrentInfo(bytes), pack.parentFile)
            val sh = seeder.find(TorrentInfo(bytes).infoHash())
            val start = System.currentTimeMillis()
            while (!sh.status().isSeeding && System.currentTimeMillis() - start < 20_000) Thread.sleep(100)
            assertTrue("seeder ready", sh.status().isSeeding)
            val port = 46881

            val out = File(root, "dl")
            val h = Torrents.add(bytes, out)
            h.swig().connect_peer(TcpEndpoint("127.0.0.1", port).swig())
            var s = Torrents.stats(h)
            val t0 = System.currentTimeMillis()
            while (!s.finished && System.currentTimeMillis() - t0 < 60_000) { Thread.sleep(200); s = Torrents.stats(h) }
            assertTrue("finished: $s", s.finished)
            assertEquals(1_000_000L, s.total)
            assertEquals(s.total, s.done)
            Torrents.remove(h, deleteFiles = false)
            assertArrayEquals(a, File(out, "Pack/a.bin").readBytes())
            assertArrayEquals(b, File(out, "Pack/sub/b.bin").readBytes())
        } finally {
            seeder.stop()
            root.deleteRecursively()
        }
    }
}
