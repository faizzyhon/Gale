package com.faizzyhon.gale.engine

/** What yt-dlp reported about one available format. [height] is the shorter side (portrait videos too). */
data class Fmt(val height: Int?, val hasVideo: Boolean, val hasAudio: Boolean, val size: Long?)

/** One line in the quality picker. [height] null = audio only. */
data class Quality(val label: String, val height: Int?, val mp3: Boolean = false, val size: Long? = null) {
    /** yt-dlp arguments that produce this choice; video is merged into MP4 so every phone plays it. */
    fun args(): List<String> = when {
        height == null && mp3 -> listOf("-f", "ba/b", "-x", "--audio-format", "mp3")
        height == null -> listOf("-f", "ba[ext=m4a]/ba/b", "-x", "--audio-format", "m4a")
        // "res" is the shorter side, so a 1080×1920 reel counts as 1080p; H.264/AAC first because every phone plays it.
        else -> listOf("-f", "bv*+ba/b", "-S", "res:$height,vcodec:h264,acodec:aac,ext:mp4:m4a", "--merge-output-format", "mp4")
    }
}

object Qualities {
    private val STEPS = listOf(2160, 1440, 1080, 720, 480, 360, 240)

    /** Heights the page really has (rounded to common steps), best first, then two audio options. */
    fun from(formats: List<Fmt>): List<Quality> {
        val video = formats.filter { it.hasVideo && (it.height ?: 0) > 0 }
        val bestAudio = formats.filter { it.hasAudio && !it.hasVideo }.mapNotNull { it.size }.maxOrNull() ?: 0
        val heights = video.mapNotNull { f -> STEPS.firstOrNull { f.height!! >= it - it / 10 } ?: f.height }.distinct().sortedDescending()
        val out = heights.map { h ->
            val v = video.filter { (it.height ?: 0) <= h + h / 10 }.maxByOrNull { it.height ?: 0 }
            val size = v?.size?.let { if (v.hasAudio) it else it + bestAudio }
            Quality(if (h >= 2160) "4K" else "${h}p", h, size = size)
        }.toMutableList()
        if (out.isEmpty() && formats.any { it.hasVideo }) out += Quality("Best", 100_000)
        if (formats.any { it.hasAudio }) {
            out += Quality("Audio (M4A)", null, size = bestAudio.takeIf { it > 0 })
            out += Quality("Audio (MP3)", null, mp3 = true, size = bestAudio.takeIf { it > 0 })
        }
        return out
    }
}
