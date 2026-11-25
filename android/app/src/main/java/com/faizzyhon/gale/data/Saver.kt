package com.faizzyhon.gale.data

import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.provider.MediaStore
import android.webkit.MimeTypeMap
import com.faizzyhon.gale.engine.Category
import java.io.File

/**
 * Moves finished files from the app's work folder into shared storage, where the gallery, music player and file
 * manager see them: videos in Movies/Gale, audio in Music/Gale, pictures in Pictures/Gale, everything else in
 * Download/Gale. No storage permission is needed for files the app creates there (Android 10+).
 */
object Saver {
    fun mimeOf(name: String): String =
        MimeTypeMap.getSingleton().getMimeTypeFromExtension(name.substringAfterLast('.', "").lowercase()) ?: "application/octet-stream"

    /** Publishes [file]; [subdir] keeps a torrent's folder layout. Returns the new content:// address. */
    fun publish(context: Context, file: File, subdir: String = "", keepTogether: Boolean = false): Uri {
        val mime = mimeOf(file.name)
        val (collection, root) = when (if (keepTogether) Category.OTHER else Category.of(file.name)) {
            Category.VIDEO -> MediaStore.Video.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY) to "Movies/Gale"
            Category.MUSIC -> MediaStore.Audio.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY) to "Music/Gale"
            Category.PICTURE -> MediaStore.Images.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY) to "Pictures/Gale"
            else -> MediaStore.Downloads.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY) to "Download/Gale"
        }
        val values = ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, file.name) // MediaStore adds " (1)" if the name is taken
            put(MediaStore.MediaColumns.MIME_TYPE, mime)
            put(MediaStore.MediaColumns.RELATIVE_PATH, listOf(root, subdir).filter { it.isNotBlank() }.joinToString("/") + "/")
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }
        val resolver = context.contentResolver
        val uri = resolver.insert(collection, values) ?: error("Android refused to create ${file.name} in $root")
        try {
            resolver.openOutputStream(uri)!!.use { out -> file.inputStream().use { it.copyTo(out, 256 * 1024) } }
            resolver.update(uri, ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }, null, null)
        } catch (e: Exception) {
            resolver.delete(uri, null, null) // no half-copied file left behind
            throw e
        }
        file.delete()
        return uri
    }

    /** Publishes every file under [dir] (a torrent's folder) to Download/Gale/[name], keeping its sub-folders. */
    fun publishTree(context: Context, dir: File, name: String): List<Uri> =
        dir.walkTopDown().filter { it.isFile }.map { f ->
            val rel = f.parentFile!!.relativeTo(dir).path
            publish(context, f, listOf(name, rel).filter { it.isNotBlank() }.joinToString("/"), keepTogether = true)
        }.toList()

    fun folderFor(name: String) = when (Category.of(name)) {
        Category.VIDEO -> "Movies/Gale"
        Category.MUSIC -> "Music/Gale"
        Category.PICTURE -> "Pictures/Gale"
        else -> "Download/Gale"
    }
}
