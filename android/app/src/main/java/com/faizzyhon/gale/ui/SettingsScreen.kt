package com.faizzyhon.gale.ui

import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.faizzyhon.gale.Gale
import com.faizzyhon.gale.engine.Media
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(onClose: () -> Unit) {
    BackHandler(onBack = onClose)
    val g = LocalGale.current
    val ctx = LocalContext.current
    val p by Gale.settings.prefs.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var signedIn by remember { mutableStateOf(Gale.cookies.exists()) }
    LifecycleResumeEffect(Unit) { signedIn = Gale.cookies.exists(); onPauseOrDispose { } }
    var updating by remember { mutableStateOf(false) }
    var connections by remember { mutableIntStateOf(p.connections) }

    Scaffold(
        containerColor = g.app,
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = { IconButton(onClick = onClose) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") } },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = g.app),
            )
        },
    ) { pad ->
        Column(Modifier.padding(pad).fillMaxSize().verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Section("Downloads") {
                Text("Connections per file: $connections", color = g.fg)
                Slider(
                    value = connections.toFloat(), onValueChange = { connections = it.toInt() },
                    onValueChangeFinished = { Gale.settings.update { it.copy(connections = connections) } },
                    valueRange = 1f..16f, steps = 14,
                )
                Text("More connections help when a server is slow per connection. 8 suits most sites.", color = g.fg3, fontSize = 12.sp)
                Spacer(Modifier.height(4.dp))
                Text("Downloads at the same time", color = g.fg)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    for (n in 1..5) FilterChip(selected = p.maxActive == n, onClick = { Gale.settings.update { it.copy(maxActive = n) }; Gale.pump() }, label = { Text("$n") })
                }
                Toggle("Only download on Wi-Fi", "Waits on mobile data and metered networks", p.wifiOnly) { v -> Gale.settings.update { it.copy(wifiOnly = v) }; Gale.pump() }
                Toggle("Resume automatically", "Unfinished downloads continue when Gale opens again", p.autoResume) { v -> Gale.settings.update { it.copy(autoResume = v) } }
            }
            Section("Appearance") {
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    for ((k, label) in listOf("system" to "System", "light" to "Light", "dark" to "Dark"))
                        FilterChip(selected = p.theme == k, onClick = { Gale.settings.update { it.copy(theme = k) } }, label = { Text(label) })
                }
            }
            Section("Instagram") {
                Text(
                    if (signedIn) "Signed in. Stories and posts from private accounts you follow can be downloaded."
                    else "Reels and public posts work without signing in. Stories need you to sign in.",
                    color = g.fg2, fontSize = 13.sp,
                )
                if (signedIn) OutlinedButton(onClick = { InstagramLoginActivity.signOut(); signedIn = false }) { Text("Sign out") }
                else OutlinedButton(onClick = { ctx.startActivity(Intent(ctx, InstagramLoginActivity::class.java)) }) { Text("Sign in to Instagram") }
                Text("You sign in on Instagram's own page. Gale keeps the session only on this phone and never sees your password.", color = g.fg3, fontSize = 12.sp)
            }
            Section("Browser") {
                Toggle("Block ads and trackers", "Uses a list of about 76,000 ad and tracking domains", p.adBlock) { v -> Gale.settings.update { it.copy(adBlock = v) } }
                OutlinedButton(onClick = {
                    android.webkit.CookieManager.getInstance().removeAllCookies(null)
                    android.webkit.WebStorage.getInstance().deleteAllData()
                    Toast.makeText(ctx, "Browser data cleared", Toast.LENGTH_SHORT).show()
                }) { Text("Clear cookies and site data") }
            }
            if (!com.faizzyhon.gale.BuildConfig.PLAY) Section("Video support") {
                Text("Sites change often. Gale updates yt-dlp every few days; update now if a site stopped working.", color = g.fg2, fontSize = 13.sp)
                OutlinedButton(enabled = !updating, onClick = {
                    updating = true
                    scope.launch {
                        val r = runCatching { Media.update(ctx) }
                        updating = false
                        Toast.makeText(ctx, r.fold({ if (it == "ALREADY_UP_TO_DATE") "Already up to date" else "Updated" }, { "Update failed: ${it.message}" }), Toast.LENGTH_LONG).show()
                    }
                }) { Text(if (updating) "Updating…" else "Update now") }
            }
            Section("About") {
                Text("Gale for Android ${com.faizzyhon.gale.BuildConfig.VERSION_NAME}", color = g.fg, fontWeight = FontWeight.Medium)
                Text("Free and open source (MIT). No ads, no account, no tracking.", color = g.fg2, fontSize = 13.sp)
                Text("Developer: Muhammad Faizan", color = g.fg2, fontSize = 13.sp)
                Row {
                    TextButton(onClick = { view(ctx, "https://faizzyhon.com") }) { Text("faizzyhon.com") }
                    TextButton(onClick = { view(ctx, "https://github.com/faizzyhon/Gale") }) { Text("Source code") }
                }
                Text(
                    "Download only what you have the right to. Many sites, including YouTube and Instagram, do not allow downloading in their terms. " +
                        "DRM-protected video is not supported. Torrent peers can see your IP address.",
                    color = g.fg3, fontSize = 12.sp,
                )
            }
        }
    }
}

private fun view(ctx: android.content.Context, url: String) = runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) }

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    val g = LocalGale.current
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(g.surface).padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(title, style = MaterialTheme.typography.titleSmall, color = g.accent)
        content()
    }
}

@Composable
private fun Toggle(title: String, hint: String, value: Boolean, onChange: (Boolean) -> Unit) {
    val g = LocalGale.current
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(title, color = g.fg)
            Text(hint, color = g.fg3, fontSize = 12.sp)
        }
        Switch(value, onChange)
    }
}
