package com.faizzyhon.gale.ui

import androidx.compose.runtime.Composable
import com.faizzyhon.gale.Live
import com.faizzyhon.gale.data.Download
import com.faizzyhon.gale.data.Status
import com.github.takahirom.roborazzi.captureRoboImage
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** Renders the real Compose UI on the JVM (Robolectric native graphics) and saves PNGs under build/outputs/roborazzi. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], qualifiers = "w411dp-h891dp-xxhdpi", application = android.app.Application::class)
class ScreenshotTest {
    private val mb = 1L shl 20
    private val items = listOf(
        Download(1, "https://www.instagram.com/reel/x", "Sunset timelapse over Madinah.mp4", "video", Status.RUNNING, total = 48 * mb, done = 31 * mb),
        Download(2, "magnet:?xt=urn:btih:abc", "Ubuntu 24.04 Desktop", "torrent", Status.RUNNING, total = 5900 * mb, done = 1200 * mb),
        Download(3, "https://example.org/report.pdf", "Annual report 2026.pdf", "http", Status.PAUSED, total = 12 * mb, done = 5 * mb),
        Download(4, "https://www.instagram.com/stories/x/1", "natgeo story (3 files)", "video", Status.DONE, total = 9 * mb, done = 9 * mb, savedUri = "content://x"),
        Download(5, "https://youtu.be/x", "Podcast episode 42.m4a", "audio", Status.DONE, total = 61 * mb, done = 61 * mb, savedUri = "content://y"),
        Download(6, "https://example.org/app.apk", "gale-setup.apk", "http", Status.FAILED, error = "The file is not on the server any more (404)."),
    )
    private val live = mapOf(
        1L to Live(31 * mb, 48 * mb, 2_400_000),
        2L to Live(1200 * mb, 5900 * mb, 6_100_000, peers = 37),
    )

    private fun shot(name: String, content: @Composable () -> Unit) = captureRoboImage("build/outputs/roborazzi/$name.png", content = content)

    @Test fun homeLight() = shot("home-light") { GaleTheme("light") { Home(items, live, "All", {}, {}, {}) } }
    @Test fun homeDark() = shot("home-dark") { GaleTheme("dark") { Home(items, live, "All", {}, {}, {}) } }
    @Test fun empty() = shot("home-empty") { GaleTheme("light") { Home(emptyList(), emptyMap(), "All", {}, {}, {}) } }
}
