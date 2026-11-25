package com.faizzyhon.gale

import android.app.Application
import com.faizzyhon.gale.engine.Media
import kotlin.concurrent.thread

class GaleApp : Application() {
    override fun onCreate() {
        super.onCreate()
        Notifier.channels(this)
        Gale.init(this)
        // Unpacking Python and FFmpeg takes a few seconds the first time; do it before anyone asks for a video.
        thread(name = "media-init") { runCatching { Media.init(this) } }
    }
}
