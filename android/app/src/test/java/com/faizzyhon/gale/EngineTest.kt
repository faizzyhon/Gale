package com.faizzyhon.gale

import com.faizzyhon.gale.data.Store
import com.faizzyhon.gale.engine.Category
import com.faizzyhon.gale.engine.Fmt
import com.faizzyhon.gale.engine.HttpEngine
import com.faizzyhon.gale.engine.LinkKind
import com.faizzyhon.gale.engine.Links
import com.faizzyhon.gale.engine.Media
import com.faizzyhon.gale.engine.Qualities
import com.faizzyhon.gale.engine.Segment
import com.faizzyhon.gale.ui.InstagramLoginActivity
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import mockwebserver3.Dispatcher
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest
import okio.Buffer
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.io.File
import java.nio.file.Files
import java.util.concurrent.atomic.AtomicLong
import kotlin.random.Random

class LinksTest {
    @Test fun extractsLinksFromSharedText() {
        val shared = "Watch this reel! https://www.instagram.com/reel/C9xYz12AbCd/?igsh=abc123. And https://youtu.be/dQw4w9WgXcQ"
        assertEquals(listOf("https://www.instagram.com/reel/C9xYz12AbCd/?igsh=abc123", "https://youtu.be/dQw4w9WgXcQ"), Links.extract(shared))
    }

    @Test fun kinds() {
        assertEquals(LinkKind.MEDIA, Links.kind("https://www.instagram.com/stories/natgeo/3456789012345678901/"))
        assertEquals(LinkKind.MEDIA, Links.kind("https://m.youtube.com/watch?v=abc"))
        assertEquals(LinkKind.MEDIA, Links.kind("https://vm.tiktok.com/ZMabc/"))
        assertEquals(LinkKind.FILE, Links.kind("https://releases.ubuntu.com/24.04/ubuntu.iso"))
        assertEquals(LinkKind.TORRENT, Links.kind("https://example.org/files/debian.torrent"))
        assertEquals(LinkKind.TORRENT, Links.kind("magnet:?xt=urn:btih:c9e15763f722f23e98a29decdfae341b98d53056&dn=Cosmos"))
        assertNull(Links.kind("magnet:?dn=no-hash"))
        assertNull(Links.kind("ftp://example.org/a"))
        assertNull(Links.kind("javascript:alert(1)"))
        assertNull(Links.kind("not a link"))
    }

    @Test fun names() {
        assertEquals("Cosmos Laundromat", Links.magnetName("magnet:?xt=urn:btih:abc&dn=Cosmos+Laundromat&tr=x"))
        assertEquals("report 2026.pdf", Links.fileNameFromUrl("https://x.org/files/report%202026.pdf?dl=1"))
        assertEquals("naïve.zip", Links.fileNameFromDisposition("attachment; filename=\"x.zip\"; filename*=UTF-8''na%C3%AFve.zip"))
        assertEquals("plain.bin", Links.fileNameFromDisposition("attachment; filename=plain.bin"))
        assertEquals("a_b_c.txt", Links.safeName("a/b\\c.txt"))
        assertEquals("download", Links.safeName("..."))
        assertEquals(Category.VIDEO, Category.of("clip.MP4"))
        assertEquals(Category.APP, Category.of("app.apk"))
        assertEquals(Category.TORRENT, Category.of("x", "torrent"))
        assertEquals(Category.MUSIC, Category.of("song.webm", "audio"))
    }
}

class QualitiesTest {
    @Test fun buildsTheListFromFormats() {
        val q = Qualities.from(
            listOf(
                Fmt(1080, hasVideo = true, hasAudio = false, size = 50_000_000),
                Fmt(720, hasVideo = true, hasAudio = false, size = 20_000_000),
                Fmt(714, hasVideo = true, hasAudio = false, size = 19_000_000), // odd height rounds to 720
                Fmt(null, hasVideo = false, hasAudio = true, size = 3_000_000),
            ),
        )
        assertEquals(listOf("1080p", "720p", "Audio (M4A)", "Audio (MP3)"), q.map { it.label })
        assertEquals(53_000_000L, q[0].size) // video + best audio
        assertTrue(q[0].args().joinToString(" ").contains("res:1080"))
        assertTrue(q[3].args().containsAll(listOf("-x", "mp3")))
    }

    @Test fun parsesYtDlpJson() {
        // Instagram-style: codecs left out, height present.
        val reel = JSONObject("""{"title":"A reel","formats":[{"width":1080,"height":1920,"filesize":9000000},{"width":360,"height":640,"filesize":2000000}]}""")
        val info = Media.parse(reel)
        assertEquals("A reel", info.title)
        assertEquals(listOf("1080p", "360p", "Audio (M4A)", "Audio (MP3)"), info.qualities.map { it.label })
        // A story reel is a playlist: count items, drop sizes.
        val story = JSONObject("""{"title":"natgeo","entries":[{"title":"1","formats":[{"height":1280,"vcodec":"avc1","acodec":"mp4a"}]},{"title":"2"}]}""")
        val s = Media.parse(story)
        assertEquals(2, s.items)
        assertTrue(s.qualities.all { it.size == null })
        // No formats at all (some sites give one URL): still offer "Best".
        assertEquals("Best", Media.parse(JSONObject("""{"title":"x"}""")).qualities.single().label)
    }

    @Test fun parsesProgressLines() {
        assertEquals((12.34 * 1024 * 1024).toLong() to (1.2 * 1024 * 1024).toLong(), Media.parseLine("[download]  45.3% of ~ 12.34MiB at  1.20MiB/s ETA 00:09"))
        assertEquals(512L * 1024 to 0L, Media.parseLine("[download]   1.0% of 512.00KiB at Unknown B/s ETA Unknown"))
        assertEquals(null to 0L, Media.parseLine("[Merger] Merging formats"))
    }

    @Test fun instagramCookiesFile() {
        val txt = InstagramLoginActivity.netscape("sessionid=abc%3A123; csrftoken=xyz", now = 1000)
        val lines = txt.lines().filter { it.isNotBlank() }
        assertEquals("# Netscape HTTP Cookie File", lines[0])
        assertEquals(".instagram.com\tTRUE\t/\tTRUE\t${1000 + 365L * 24 * 3600}\tsessionid\tabc%3A123", lines[1])
        assertEquals(3, lines.size)
    }

    @Test fun segmentsRoundTrip() {
        val s = listOf(Segment(0, 99, 50), Segment(100, 199, 0))
        assertEquals(s, Store.decodeSegments(Store.encodeSegments(s)))
        assertEquals(emptyList<Segment>(), Store.decodeSegments(""))
    }
}

class HttpEngineTest {
    private lateinit var server: MockWebServer
    private lateinit var dir: File
    private val data = Random(7).nextBytes(5_300_000)
    private var ranges = true
    private val requests = AtomicLong()

    @Before fun setUp() {
        dir = Files.createTempDirectory("gale").toFile()
        server = MockWebServer()
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse {
                requests.incrementAndGet()
                val range = request.headers["Range"]
                if (!ranges || range == null) return MockResponse.Builder().code(200).body(Buffer().write(data)).build()
                val (a, b) = range.removePrefix("bytes=").split('-').map { it.toLong() }
                val end = minOf(b, data.size - 1L)
                return MockResponse.Builder().code(206)
                    .addHeader("Content-Range", "bytes $a-$end/${data.size}")
                    .addHeader("Content-Disposition", "attachment; filename=\"data.bin\"")
                    .body(Buffer().write(data, a.toInt(), (end - a + 1).toInt()))
                    .build()
            }
        }
        server.start()
    }

    @After fun tearDown() { server.close(); dir.deleteRecursively() }

    @Test fun sharedLinksThatJavaUriRejects() {
        // Spaces, pipes and unicode as apps really share them.
        assertEquals(LinkKind.FILE, Links.kind("https://example.com/files/my file|v2.zip"))
        assertEquals(LinkKind.FILE, Links.kind("https://example.com/تحميل.pdf"))
        assertEquals("my file|v2.zip", Links.fileNameFromUrl("https://example.com/files/my%20file%7Cv2.zip"))
        assertEquals(LinkKind.MEDIA, Links.kind("https://m.youtube.com/watch?v=x&list=a|b"))
        assertEquals(null, Links.kind("ftp://example.com/a.zip"))
    }

    @Test fun youTubeDetection() {
        assertTrue(Links.isYouTube("https://youtu.be/dQw4w9WgXcQ?si=1"))
        assertTrue(Links.isYouTube("https://music.youtube.com/watch?v=1"))
        assertTrue(!Links.isYouTube("https://notyoutube.com/v"))
        assertTrue(!Links.isYouTube("https://vimeo.com/1"))
    }

    @Test fun addressBarTurnsWordsIntoASearch() {
        assertEquals("https://example.com", com.faizzyhon.gale.ui.addressFor("example.com"))
        assertEquals("http://x.org/a", com.faizzyhon.gale.ui.addressFor(" http://x.org/a "))
        assertEquals("https://duckduckgo.com/?q=linux+iso", com.faizzyhon.gale.ui.addressFor("linux iso"))
    }

    @Test fun adBlockMatchesParentDomains() {
        val b = com.faizzyhon.gale.engine.AdBlock(sequenceOf("# comment", "doubleclick.net", "ads.example.org", ""))
        assertEquals(2, b.size)
        assertTrue(b.blocks("doubleclick.net"))
        assertTrue(b.blocks("stats.g.DoubleClick.net."))
        assertTrue(b.blocks("x.ads.example.org"))
        assertTrue(!b.blocks("example.org"))
        assertTrue(!b.blocks("net"))
        assertTrue(!b.blocks(null))
    }

    @Test fun bundledAdListLoads() {
        val f = File("src/main/assets/adblock-hosts.txt")
        val b = com.faizzyhon.gale.engine.AdBlock(f.readLines().asSequence())
        assertTrue(b.size > 50_000)
        assertTrue(b.blocks("pagead2.googlesyndication.com"))
        assertTrue(!b.blocks("github.com"))
        assertTrue(!b.blocks("duckduckgo.com"))
    }

    @Test fun planSplitsIntoAtLeastOneMegabyteParts() {
        assertEquals(8, HttpEngine.plan(100L shl 20, true, 8).size)
        assertEquals(5, HttpEngine.plan(5_300_000, true, 8).size)
        assertEquals(1, HttpEngine.plan(1000, true, 8).size)
        assertEquals(listOf(Segment(0, -1)), HttpEngine.plan(1000, false, 8))
        val p = HttpEngine.plan(5_300_000, true, 8)
        assertEquals(5_300_000L, p.sumOf { it.end - it.start + 1 })
    }

    @Test fun segmentedDownloadMatchesTheSource() = runBlocking {
        val e = HttpEngine()
        val probe = e.probe(server.url("/f").toString())
        assertEquals(5_300_000L, probe.total)
        assertTrue(probe.ranges)
        assertEquals("data.bin", probe.fileName)
        val segs = HttpEngine.plan(probe.total, probe.ranges, 8)
        val f = File(dir, "out")
        val got = AtomicLong()
        e.download(probe.finalUrl, f, segs, onBytes = { got.addAndGet(it.toLong()) }, checkpoint = {})
        assertArrayEquals(data, f.readBytes())
        assertEquals(data.size.toLong(), got.get())
    }

    @Test fun pauseThenResumeContinuesWhereItStopped() = runBlocking {
        val e = HttpEngine()
        val segs = HttpEngine.plan(data.size.toLong(), true, 4)
        val f = File(dir, "out")
        var saved: List<Segment> = emptyList()
        val got = AtomicLong()
        val job = launch(Dispatchers.IO) {
            e.download(server.url("/f").toString(), f, segs, onBytes = { got.addAndGet(it.toLong()) }, checkpoint = { saved = it })
        }
        while (got.get() < 1_000_000) withContext(Dispatchers.IO) { Thread.sleep(1) }
        job.cancel()
        job.join()
        val partial = segs.map { it.copy() } // what the app saves at a pause
        val before = partial.sumOf { it.done }
        assertTrue("paused part-way: $before", before in 1 until data.size)
        e.download(server.url("/f").toString(), f, partial, onBytes = {}, checkpoint = {})
        assertArrayEquals(data, f.readBytes())
        assertTrue(saved.isNotEmpty() || before > 0)
    }

    @Test fun serverWithoutRangesDownloadsInOnePiece() = runBlocking {
        ranges = false
        val e = HttpEngine()
        val p = e.probe(server.url("/f").toString())
        assertTrue(!p.ranges)
        val f = File(dir, "out")
        e.download(p.finalUrl, f, HttpEngine.plan(p.total, p.ranges, 8), onBytes = {}, checkpoint = {})
        assertArrayEquals(data, f.readBytes())
    }

    @Test fun offlineWaitsInsteadOfUsingUpRetries() = runBlocking {
        // Every request fails while the phone is "offline" (more failures than the 8 allowed retries); once it is
        // back online the server answers again and the download must finish.
        val offline = java.util.concurrent.atomic.AtomicBoolean(true)
        val good = server.dispatcher
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse =
                if (offline.get()) MockResponse.Builder().code(503).build().also { requests.incrementAndGet() } else good.dispatch(request)
        }
        val e = HttpEngine(online = { !offline.get() })
        val f = File(dir, "out")
        val t0 = System.currentTimeMillis()
        Thread { Thread.sleep(3000); offline.set(false) }.start()
        e.download(server.url("/f").toString(), f, HttpEngine.plan(data.size.toLong(), true, 2), onBytes = {}, checkpoint = {})
        assertArrayEquals(data, f.readBytes())
        assertTrue("waited for the network", System.currentTimeMillis() - t0 >= 2500)
    }

    @Test fun notFoundIsNotRetried() = runBlocking {
        server.dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest) = MockResponse.Builder().code(404).build().also { requests.incrementAndGet() }
        }
        val e = HttpEngine()
        val err = runCatching { e.download(server.url("/f").toString(), File(dir, "x"), listOf(Segment(0, 99)), onBytes = {}, checkpoint = {}) }.exceptionOrNull()
        assertTrue(err is com.faizzyhon.gale.engine.HttpError && err.status == 404)
        assertEquals(1L, requests.get())
        Unit
    }
}
