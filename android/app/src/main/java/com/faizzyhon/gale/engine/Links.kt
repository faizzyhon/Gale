package com.faizzyhon.gale.engine

import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import java.net.URLDecoder

/** What Gale does with an address someone pasted or shared. */
enum class LinkKind { FILE, MEDIA, TORRENT }

object Links {
    private val URL = Regex("""(?i)\b(?:https?://|magnet:\?)[^\s<>"']+""")

    // Sites whose pages are a video (or a story/reel/post) rather than a file; yt-dlp reads them.
    private val MEDIA_HOSTS = listOf(
        "youtube.com", "youtu.be", "instagram.com", "tiktok.com", "facebook.com", "fb.watch", "x.com", "twitter.com",
        "vimeo.com", "dailymotion.com", "twitch.tv", "reddit.com", "v.redd.it", "pinterest.com", "pin.it",
        "soundcloud.com", "bilibili.com", "rumble.com", "threads.net", "threads.com", "snapchat.com", "linkedin.com",
        "streamable.com", "bandcamp.com", "mixcloud.com", "likee.video", "vk.com", "ok.ru",
    )

    /** Every link in shared text, in order, without duplicates ("Check this reel https://… #fun" → the link). */
    fun extract(text: String): List<String> =
        URL.findAll(text).map { it.value.trimEnd('.', ',', ')', ']', '!', '?', ';') }.distinct().toList()

    fun kind(url: String): LinkKind? {
        if (isMagnet(url)) return if (validMagnet(url)) LinkKind.TORRENT else null
        // OkHttp's parser accepts what apps really share (spaces, |, unicode, unencoded query parts); java.net.URI does not.
        val u = url.trim().toHttpUrlOrNull() ?: return null
        if (u.encodedPath.lowercase().endsWith(".torrent")) return LinkKind.TORRENT
        val host = u.host.lowercase().removePrefix("www.").removePrefix("m.")
        if (MEDIA_HOSTS.any { host == it || host.endsWith(".$it") }) return LinkKind.MEDIA
        return LinkKind.FILE
    }

    const val YOUTUBE_BLOCKED = "Downloading from YouTube isn't available in this version of Gale."

    fun isYouTube(url: String): Boolean {
        val host = url.trim().toHttpUrlOrNull()?.host?.lowercase() ?: return false
        return listOf("youtube.com", "youtu.be", "youtube-nocookie.com").any { host == it || host.endsWith(".$it") }
    }

    fun isMagnet(s: String) = s.trimStart().startsWith("magnet:?", ignoreCase = true)

    /** A magnet with a BitTorrent v1/v2 hash and a sane length (same rule as the desktop app). */
    fun validMagnet(s: String): Boolean {
        val m = s.trim().lowercase()
        return isMagnet(m) && m.length <= 8192 && ("xt=urn:btih:" in m || "xt=urn:btmh:" in m)
    }

    fun magnetName(magnet: String): String? = magnet.substringAfter('?', "").split('&')
        .firstOrNull { it.startsWith("dn=") }
        ?.let { runCatching { URLDecoder.decode(it.removePrefix("dn="), "UTF-8") }.getOrNull() }
        ?.trim()?.takeIf { it.isNotEmpty() }

    fun fileNameFromUrl(url: String): String? = url.trim().toHttpUrlOrNull()?.pathSegments?.lastOrNull()?.takeIf { it.isNotBlank() }

    /** filename="…" / filename*=UTF-8''… from a Content-Disposition header. */
    fun fileNameFromDisposition(cd: String?): String? {
        if (cd == null) return null
        Regex("""(?i)filename\*\s*=\s*[^']*'[^']*'([^;]+)""").find(cd)?.let {
            return runCatching { URLDecoder.decode(it.groupValues[1].trim().trim('"'), "UTF-8") }.getOrNull()
        }
        return Regex("""(?i)filename\s*=\s*"?([^";]+)"?""").find(cd)?.groupValues?.get(1)?.trim()
    }

    /** A name that is safe as one path component on Android's file systems. */
    fun safeName(name: String): String {
        val cleaned = name.replace(Regex("""[\\/:*?"<>|\u0000-\u001f]"""), "_").trim().trim('.').take(150)
        return cleaned.ifBlank { "download" }
    }
}

enum class Category(val label: String) {
    VIDEO("Video"), MUSIC("Music"), PICTURE("Pictures"), DOCUMENT("Documents"), COMPRESSED("Compressed"),
    APP("Apps"), TORRENT("Torrents"), OTHER("Other");

    companion object {
        private val EXT = mapOf(
            VIDEO to "mp4 mkv webm avi mov m4v wmv flv 3gp ts mpg mpeg",
            MUSIC to "mp3 m4a aac flac wav ogg opus wma",
            PICTURE to "jpg jpeg png gif webp heic bmp svg",
            DOCUMENT to "pdf doc docx xls xlsx ppt pptx txt epub csv odt rtf",
            COMPRESSED to "zip rar 7z tar gz bz2 xz iso",
            APP to "apk apks xapk exe msi",
        ).mapValues { it.value.split(' ').toSet() }

        fun of(name: String, kind: String = "http"): Category {
            if (kind == "torrent") return TORRENT
            if (kind == "audio") return MUSIC
            if (kind == "video") return VIDEO
            val ext = name.substringAfterLast('.', "").lowercase()
            return EXT.entries.firstOrNull { ext in it.value }?.key ?: OTHER
        }
    }
}
