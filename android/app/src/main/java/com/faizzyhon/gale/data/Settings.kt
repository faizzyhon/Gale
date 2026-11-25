package com.faizzyhon.gale.data

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

data class Prefs(
    val connections: Int = 8,
    val maxActive: Int = 3,
    val wifiOnly: Boolean = false,
    val autoResume: Boolean = true,
    /** system, light or dark */
    val theme: String = "system",
    /** Block ad and tracker domains in Gale's browser. */
    val adBlock: Boolean = true,
)

class Settings(context: Context) {
    private val sp = context.getSharedPreferences("settings", Context.MODE_PRIVATE)
    private val _prefs = MutableStateFlow(read())
    val prefs: StateFlow<Prefs> = _prefs

    private fun read() = Prefs(
        connections = sp.getInt("connections", 8),
        maxActive = sp.getInt("maxActive", 3),
        wifiOnly = sp.getBoolean("wifiOnly", false),
        autoResume = sp.getBoolean("autoResume", true),
        theme = sp.getString("theme", "system") ?: "system",
        adBlock = sp.getBoolean("adBlock", true),
    )

    fun update(change: (Prefs) -> Prefs) {
        val p = change(_prefs.value)
        sp.edit()
            .putInt("connections", p.connections.coerceIn(1, 16))
            .putInt("maxActive", p.maxActive.coerceIn(1, 5))
            .putBoolean("wifiOnly", p.wifiOnly)
            .putBoolean("autoResume", p.autoResume)
            .putString("theme", p.theme)
            .putBoolean("adBlock", p.adBlock)
            .apply()
        _prefs.value = read()
    }
}
