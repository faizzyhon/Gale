package com.faizzyhon.gale.ui

import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.widget.Toast
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.InsertDriveFile
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Apps
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Public
import androidx.compose.material.icons.filled.Description
import androidx.compose.material.icons.filled.Hub
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.Movie
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.MusicNote
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.UploadFile
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.faizzyhon.gale.Gale
import com.faizzyhon.gale.Live
import com.faizzyhon.gale.data.Download
import com.faizzyhon.gale.data.Saver
import com.faizzyhon.gale.data.Status
import com.faizzyhon.gale.engine.Category
import com.faizzyhon.gale.engine.LinkKind
import com.faizzyhon.gale.engine.Links
import com.faizzyhon.gale.engine.Media
import com.faizzyhon.gale.engine.MediaInfo
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

private val FILTERS = listOf("All", "Downloading", "Finished") + Category.entries.filter { it != Category.OTHER }.map { it.label }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun App(incoming: List<String>?, onIncomingShown: () -> Unit, pickTorrent: () -> Unit) {
    val all by Gale.store.all.collectAsStateWithLifecycle()
    val live by Gale.live.collectAsStateWithLifecycle()
    var filter by rememberSaveable { mutableStateOf("All") }
    var adding by remember { mutableStateOf<List<String>?>(null) }
    var settings by rememberSaveable { mutableStateOf(false) }
    /** Page open in Gale's browser ("" = its start page), or null. */
    var browsing by rememberSaveable { mutableStateOf<String?>(null) }
    val g = LocalGale.current

    LaunchedEffect(incoming) { if (incoming != null) { adding = incoming; onIncomingShown() } }

    if (settings) { SettingsScreen(onClose = { settings = false }); return }

    val shown = all.filter { d ->
        when (filter) {
            "All" -> true
            "Downloading" -> d.status != Status.DONE
            "Finished" -> d.status == Status.DONE
            else -> Category.of(d.name, d.kind).label == filter
        }
    }

    if (browsing != null) BrowserScreen(browsing!!.ifEmpty { null }, onClose = { browsing = null }, onMediaPage = { adding = listOf(it) })
    else Home(shown, live, filter, onFilter = { filter = it }, onAdd = { adding = emptyList() }, onSettings = { settings = true }, onBrowser = { browsing = "" })

    adding?.let { links ->
        AddSheet(
            links, onDismiss = { adding = null }, pickTorrent = { adding = null; pickTorrent() },
            openInBrowser = { url -> adding = null; browsing = url },
        )
    }
}

/** The main list, without state of its own (so it can be rendered in screenshot tests). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun Home(shown: List<Download>, live: Map<Long, Live>, filter: String, onFilter: (String) -> Unit, onAdd: () -> Unit, onSettings: () -> Unit, onBrowser: () -> Unit = {}) {
    val g = LocalGale.current
    Scaffold(
        containerColor = g.app,
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        AppMark()
                        Spacer(Modifier.width(10.dp))
                        Text("Gale", fontWeight = FontWeight.SemiBold)
                    }
                },
                actions = {
                    IconButton(onClick = onBrowser) { Icon(Icons.Filled.Public, "Browser") }
                    IconButton(onClick = onSettings) { Icon(Icons.Filled.Settings, "Settings") }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = g.app),
            )
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = onAdd,
                icon = { Icon(Icons.Filled.Add, null) },
                text = { Text("Add") },
                containerColor = g.accent, contentColor = g.accentFg,
            )
        },
    ) { pad ->
        Column(Modifier.padding(pad).fillMaxSize()) {
            Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                for (f in FILTERS) FilterChip(selected = filter == f, onClick = { onFilter(f) }, label = { Text(f) })
            }
            if (shown.isEmpty()) Empty(filter == "All")
            else LazyColumn(contentPadding = PaddingValues(12.dp, 8.dp, 12.dp, 96.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                items(shown, key = { it.id }) { d -> DownloadRow(d, live[d.id]) }
            }
        }
    }
}

@Composable
private fun AppMark() {
    // The launcher icon's strokes on Gale blue, the same mark as the desktop app.
    Box(Modifier.size(30.dp).clip(RoundedCornerShape(7.dp)).background(androidx.compose.ui.graphics.Color(0xFF005FB8))) {
        androidx.compose.foundation.Image(
            androidx.compose.ui.res.painterResource(com.faizzyhon.gale.R.drawable.ic_launcher_foreground), null,
            Modifier.size(30.dp).graphicsLayer(scaleX = 1.45f, scaleY = 1.45f),
        )
    }
}

@Composable
private fun Empty(nothingAtAll: Boolean) {
    val g = LocalGale.current
    Column(Modifier.fillMaxSize().padding(32.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(if (nothingAtAll) "No downloads yet" else "Nothing here", style = MaterialTheme.typography.titleMedium, color = g.fg)
        if (nothingAtAll) {
            Spacer(Modifier.height(8.dp))
            Text(
                "Tap Add and paste a link, or use Share → Gale in Instagram, YouTube, TikTok or your browser. Magnet links and .torrent files work too.",
                color = g.fg2, style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

private fun iconFor(d: Download): ImageVector = when (Category.of(d.name, d.kind)) {
    Category.VIDEO -> Icons.Filled.Movie
    Category.MUSIC -> Icons.Filled.MusicNote
    Category.PICTURE -> Icons.Filled.Image
    Category.DOCUMENT -> Icons.Filled.Description
    Category.COMPRESSED -> Icons.Filled.Archive
    Category.APP -> Icons.Filled.Apps
    Category.TORRENT -> Icons.Filled.Hub
    Category.OTHER -> Icons.AutoMirrored.Filled.InsertDriveFile
}

@Composable
private fun DownloadRow(d: Download, l: Live?) {
    val g = LocalGale.current
    val ctx = LocalContext.current
    var menu by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf(false) }
    val done = l?.done ?: d.done
    val total = l?.total ?: d.total
    val frac = if (d.status == Status.DONE) 1f else if (total != null && total > 0) (done.toFloat() / total).coerceIn(0f, 1f) else 0f
    val animated by animateFloatAsState(frac, label = "progress")

    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(g.surface).border(1.dp, g.line, RoundedCornerShape(8.dp))
            .clickable { primary(ctx, d) }.padding(12.dp).animateContentSize(),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(36.dp).clip(RoundedCornerShape(6.dp)).background(g.accent.copy(alpha = 0.12f)), contentAlignment = Alignment.Center) {
                Icon(iconFor(d), null, tint = g.accent, modifier = Modifier.size(20.dp))
            }
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(d.name, maxLines = 1, overflow = TextOverflow.Ellipsis, color = g.fg, fontWeight = FontWeight.Medium)
                Text(meta(d, l), maxLines = 1, overflow = TextOverflow.Ellipsis, color = if (d.status == Status.FAILED) g.bad else g.fg2, fontSize = 12.sp)
            }
            when (d.status) {
                Status.RUNNING, Status.QUEUED, Status.WAITING_WIFI -> IconButton(onClick = { Gale.pause(d.id) }) { Icon(Icons.Filled.Pause, "Pause", tint = g.fg2) }
                Status.PAUSED -> IconButton(onClick = { Gale.resume(d.id) }) { Icon(Icons.Filled.PlayArrow, "Resume", tint = g.accent) }
                Status.FAILED -> IconButton(onClick = { Gale.resume(d.id) }) { Icon(Icons.Filled.Refresh, "Try again", tint = g.accent) }
                Status.DONE -> Unit
            }
            Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, "More", tint = g.fg3) }
                DropdownMenu(menu, onDismissRequest = { menu = false }) {
                    if (d.status == Status.DONE && d.savedUri?.startsWith("content:") == true) {
                        DropdownMenuItem(text = { Text("Open") }, onClick = { menu = false; open(ctx, d) })
                        DropdownMenuItem(text = { Text("Share") }, onClick = { menu = false; share(ctx, d) })
                    }
                    DropdownMenuItem(text = { Text("Copy link") }, onClick = {
                        menu = false
                        ctx.getSystemService(ClipboardManager::class.java).setPrimaryClip(android.content.ClipData.newPlainText("link", d.url))
                    })
                    DropdownMenuItem(text = { Text("Delete") }, onClick = { menu = false; confirmDelete = true })
                }
            }
        }
        if (d.status != Status.DONE) {
            Spacer(Modifier.height(8.dp))
            if (d.status == Status.RUNNING && (total == null || total <= 0)) LinearProgressIndicator(Modifier.fillMaxWidth().height(3.dp).clip(RoundedCornerShape(2.dp)), color = g.accent, trackColor = g.line)
            else LinearProgressIndicator(
                { animated }, Modifier.fillMaxWidth().height(3.dp).clip(RoundedCornerShape(2.dp)),
                color = if (d.status == Status.FAILED) g.bad else if (d.status == Status.RUNNING) g.accent else g.fg3, trackColor = g.line,
                drawStopIndicator = {},
            )
        }
    }

    if (confirmDelete) DeleteDialog(d, onDone = { confirmDelete = false })
}

private fun meta(d: Download, l: Live?): String {
    val total = l?.total ?: d.total
    val done = l?.done ?: d.done
    val size = if (total != null && total > 0) "${fmtBytes(done)} of ${fmtBytes(total)}" else if (done > 0) fmtBytes(done) else ""
    return when (d.status) {
        Status.RUNNING -> listOfNotNull(
            l?.stage, size.takeIf { it.isNotEmpty() && l?.stage == null },
            l?.speed?.takeIf { it > 0 }?.let { "${fmtBytes(it)}/s" },
            l?.takeIf { it.speed > 0 && total != null && total > done }?.let { fmtEta((total!! - done) / it.speed) },
            l?.peers?.takeIf { d.kind == "torrent" && it > 0 }?.let { "$it peers" },
        ).joinToString(" · ").ifEmpty { "Starting…" }
        Status.QUEUED -> "${d.error ?: "Waiting"} · $size".trimEnd(' ', '·')
        Status.WAITING_WIFI -> "Waiting for Wi-Fi"
        Status.PAUSED -> "Paused · $size".trimEnd(' ', '·')
        Status.FAILED -> d.error ?: "Failed"
        Status.DONE -> listOf(fmtBytes(d.total ?: d.done), d.savedUri?.removePrefix("folder:")?.takeIf { it.startsWith("Download") } ?: when (d.kind) { "video" -> "Movies/Gale"; "audio" -> "Music/Gale"; else -> Saver.folderFor(d.name) }).joinToString(" · ")
    }
}

private fun primary(ctx: Context, d: Download) = when (d.status) {
    Status.DONE -> open(ctx, d)
    Status.PAUSED, Status.FAILED -> Gale.resume(d.id)
    else -> Unit
}

private fun open(ctx: Context, d: Download) {
    val uri = d.savedUri?.takeIf { it.startsWith("content:") } ?: run {
        Toast.makeText(ctx, "Saved in ${d.savedUri?.removePrefix("folder:") ?: "Download/Gale"}", Toast.LENGTH_LONG).show(); return
    }
    runCatching {
        ctx.startActivity(Intent(Intent.ACTION_VIEW).setDataAndType(Uri.parse(uri), d.mime ?: Saver.mimeOf(d.name)).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION))
    }.onFailure { Toast.makeText(ctx, "No app on this phone can open ${d.name}", Toast.LENGTH_SHORT).show() }
}

private fun share(ctx: Context, d: Download) {
    val uri = Uri.parse(d.savedUri ?: return)
    ctx.startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).setType(d.mime ?: Saver.mimeOf(d.name)).putExtra(Intent.EXTRA_STREAM, uri).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION), null))
}

@Composable
private fun DeleteDialog(d: Download, onDone: () -> Unit) {
    var alsoFile by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = onDone,
        title = { Text("Delete ${d.name}?") },
        text = {
            if (d.savedUri?.startsWith("content:") == true) Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.clickable { alsoFile = !alsoFile }) {
                Checkbox(alsoFile, { alsoFile = it })
                Text("Also delete the file from the phone")
            } else Text("It is removed from the list. Partly downloaded data is deleted.")
        },
        confirmButton = { TextButton(onClick = { Gale.remove(d.id, alsoFile); onDone() }) { Text("Delete") } },
        dismissButton = { TextButton(onClick = onDone) { Text("Cancel") } },
    )
}

// --- Add -------------------------------------------------------------------------------------------------------------

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddSheet(initial: List<String>, onDismiss: () -> Unit, pickTorrent: () -> Unit, openInBrowser: (String) -> Unit) {
    val ctx = LocalContext.current
    val g = LocalGale.current
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val scope = rememberCoroutineScope()
    // Keyed on the links: a second share while the sheet is open replaces the first one.
    var text by remember(initial) { mutableStateOf(initial.firstOrNull() ?: clipboardLink(ctx).orEmpty()) }
    val extra = initial.drop(1)
    val url = text.trim()
    val kind = Links.kind(url)
    var info by remember { mutableStateOf<MediaInfo?>(null) }
    var probing by remember { mutableStateOf(false) }
    var probeError by remember { mutableStateOf<String?>(null) }
    /** A plain link that turned out to be a web page rather than a file. */
    var webPage by remember { mutableStateOf(false) }
    val blocked = com.faizzyhon.gale.BuildConfig.PLAY && Links.isYouTube(url)
    val needsSignIn = "instagram.com/stories" in url && !Gale.cookies.exists()

    fun close() = scope.launch { sheet.hide() }.invokeOnCompletion { onDismiss() }
    fun add(q: com.faizzyhon.gale.engine.Quality? = null) {
        runCatching { Gale.add(url, q, name = info?.title) }
            .onSuccess { extra.forEach { e -> runCatching { Gale.add(e) } }; close() }
            .onFailure { Toast.makeText(ctx, it.message, Toast.LENGTH_SHORT).show() }
    }

    // Video pages: read the qualities as soon as the link is there, so the list is ready when you look.
    LaunchedEffect(url, kind) {
        info = null; probeError = null; webPage = false
        if (kind == LinkKind.FILE) {
            // Shared article/page links: offer the browser instead of saving the HTML.
            delay(300)
            webPage = runCatching { Gale.http.probe(url).mime }.getOrNull()?.let { it == "text/html" || it == "application/xhtml+xml" } == true
            return@LaunchedEffect
        }
        if (kind != LinkKind.MEDIA || blocked) return@LaunchedEffect
        delay(400)
        probing = true
        runCatching { Media.probe(ctx, url, Gale.cookies.takeIf { it.exists() }) }
            .onSuccess { info = it }
            .onFailure { probeError = Gale.friendly(it) }
        probing = false
    }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheet, containerColor = g.surface) {
        Column(Modifier.padding(horizontal = 20.dp).padding(bottom = 16.dp).navigationBarsPadding().imePadding()) {
            Text("Add download", style = MaterialTheme.typography.titleLarge, color = g.fg)
            Spacer(Modifier.height(12.dp))
            OutlinedTextField(
                value = text, onValueChange = { text = it }, modifier = Modifier.fillMaxWidth(),
                label = { Text("Link or magnet") }, singleLine = true,
                trailingIcon = { IconButton(onClick = { clipboardLink(ctx)?.let { text = it } }) { Icon(Icons.Filled.ContentPaste, "Paste") } },
                isError = url.isNotEmpty() && kind == null,
                supportingText = {
                    Text(
                        when {
                            url.isEmpty() -> "Paste a link to a file, a video page (Instagram, YouTube, TikTok, X…) or a magnet link"
                            kind == null -> "This is not a web address or a magnet link"
                            kind == LinkKind.TORRENT -> "BitTorrent download"
                            kind == LinkKind.MEDIA -> "Video page"
                            else -> "File download"
                        } + if (extra.isNotEmpty()) " · plus ${extra.size} more link(s)" else "",
                    )
                },
            )
            OutlinedButton(onClick = pickTorrent) { Icon(Icons.Filled.UploadFile, null); Spacer(Modifier.width(6.dp)); Text("Open .torrent file") }
            Spacer(Modifier.height(12.dp))

            AnimatedVisibility(needsSignIn) {
                Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).background(g.warn.copy(alpha = 0.12f)).padding(12.dp)) {
                    Text("Instagram stories need you to be signed in.", color = g.fg)
                    TextButton(onClick = { ctx.startActivity(Intent(ctx, InstagramLoginActivity::class.java)) }) { Text("Sign in to Instagram") }
                }
            }

            when {
                blocked -> Text(Links.YOUTUBE_BLOCKED, color = g.bad, fontSize = 13.sp)
                kind == LinkKind.FILE && webPage -> {
                    Text("This link is a web page, not a file. Open it in Gale's browser and tap the file or video you want.", color = g.fg2, fontSize = 13.sp)
                    Spacer(Modifier.height(8.dp))
                    Button(onClick = { openInBrowser(url) }, Modifier.fillMaxWidth(), shape = RoundedCornerShape(4.dp)) { Text("Open in browser") }
                    TextButton(onClick = { add() }, Modifier.fillMaxWidth()) { Text("Save the page anyway") }
                }
                kind == LinkKind.MEDIA && probing -> Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(vertical = 12.dp)) {
                    CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(10.dp))
                    Text("Finding qualities…", color = g.fg2)
                }
                kind == LinkKind.MEDIA && info != null -> {
                    val i = info!!
                    Text(i.title + if (i.items > 1) " · ${i.items} items" else "", color = g.fg, fontWeight = FontWeight.Medium, maxLines = 2, overflow = TextOverflow.Ellipsis)
                    Spacer(Modifier.height(8.dp))
                    for (q in i.qualities) Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(6.dp)).clickable { add(q) }.padding(vertical = 12.dp, horizontal = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(if (q.height == null) Icons.Filled.MusicNote else Icons.Filled.Movie, null, tint = g.accent, modifier = Modifier.size(20.dp))
                        Spacer(Modifier.width(12.dp))
                        Text(q.label, Modifier.weight(1f), color = g.fg)
                        Text(q.size?.let { "~" + fmtBytes(it) } ?: "", color = g.fg3, fontSize = 13.sp)
                    }
                }
                kind == LinkKind.MEDIA && probeError != null -> {
                    Text(probeError!!, color = g.bad, fontSize = 13.sp)
                    Spacer(Modifier.height(8.dp))
                    Button(onClick = { add(com.faizzyhon.gale.engine.Quality("Best", 100_000)) }, Modifier.fillMaxWidth()) { Text("Try downloading the best quality") }
                }
                kind != null -> Button(onClick = { add() }, Modifier.fillMaxWidth(), shape = RoundedCornerShape(4.dp)) { Text(if (extra.isEmpty()) "Download" else "Download ${extra.size + 1} links") }
            }
        }
    }
}

private fun clipboardLink(ctx: Context): String? =
    ctx.getSystemService(ClipboardManager::class.java).primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(ctx)?.toString()
        ?.let { Links.extract(it).firstOrNull() }
