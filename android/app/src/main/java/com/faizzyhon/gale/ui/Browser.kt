package com.faizzyhon.gale.ui

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import android.os.Message
import android.webkit.CookieManager
import android.webkit.URLUtil
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Shield
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.faizzyhon.gale.BuildConfig
import com.faizzyhon.gale.Gale
import com.faizzyhon.gale.engine.AdBlock
import com.faizzyhon.gale.engine.LinkKind
import com.faizzyhon.gale.engine.Links
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.ByteArrayInputStream
import java.net.URLEncoder

private const val HOME = "https://duckduckgo.com/"

/** Loaded once per process on first use (about 76,000 domains). */
private var adBlock: AdBlock? = null

private fun loadAdBlock(ctx: Context): AdBlock =
    adBlock ?: AdBlock(ctx.assets.open("adblock-hosts.txt").bufferedReader().readLines().asSequence()).also { adBlock = it }

/** What the address bar turns into: a web address as typed, or a search. */
fun addressFor(input: String): String {
    val t = input.trim()
    return when {
        t.startsWith("http://", true) || t.startsWith("https://", true) -> t
        ' ' !in t && '.' in t && !t.endsWith('.') -> "https://$t"
        else -> "https://duckduckgo.com/?q=" + URLEncoder.encode(t, "UTF-8")
    }
}

/**
 * A small browser for finding files to download. Every file the page offers goes to Gale (with the page's cookies,
 * so signed-in downloads work); on video sites a button opens Gale's quality picker. Ads and trackers are blocked by
 * domain, and pages cannot open pop-ups or other apps unless you tapped something.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
fun BrowserScreen(start: String?, onClose: () -> Unit, onMediaPage: (String) -> Unit) {
    val ctx = LocalContext.current
    val g = LocalGale.current
    val focus = LocalFocusManager.current
    val prefs by Gale.settings.prefs.collectAsStateWithLifecycle()
    var blocker by remember { mutableStateOf(adBlock) }
    var page by remember { mutableStateOf(start ?: HOME) }
    var address by remember { mutableStateOf(page) }
    var progress by remember { mutableIntStateOf(0) }
    var blocked by remember { mutableIntStateOf(0) }
    var canBack by remember { mutableStateOf(false) }
    var canForward by remember { mutableStateOf(false) }
    var web by remember { mutableStateOf<WebView?>(null) }
    val blockAds = prefs.adBlock

    // The list must be in memory before the first page loads, or the first page's ads get through.
    LaunchedEffect(Unit) { if (blocker == null) blocker = withContext(Dispatchers.IO) { loadAdBlock(ctx) } }

    BackHandler { web?.takeIf { it.canGoBack() }?.goBack() ?: onClose() }
    DisposableEffect(Unit) { onDispose { web?.destroy() } }

    fun go(input: String) {
        focus.clearFocus()
        web?.loadUrl(addressFor(input))
    }

    val mediaPage = Links.kind(page) == LinkKind.MEDIA && !(BuildConfig.PLAY && Links.isYouTube(page))

    Column(Modifier.fillMaxSize().background(g.app).statusBarsPadding().navigationBarsPadding().imePadding()) {
        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onClose) { Icon(Icons.Filled.Close, "Close browser") }
            OutlinedTextField(
                value = address, onValueChange = { address = it }, singleLine = true,
                modifier = Modifier.weight(1f), shape = RoundedCornerShape(20.dp),
                textStyle = androidx.compose.ui.text.TextStyle(fontSize = 14.sp),
                placeholder = { Text("Search or type a web address") },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
                keyboardActions = KeyboardActions(onGo = { go(address) }),
            )
            IconButton(onClick = { if (progress in 1..99) web?.stopLoading() else web?.reload() }) {
                Icon(if (progress in 1..99) Icons.Filled.Close else Icons.Filled.Refresh, if (progress in 1..99) "Stop" else "Reload")
            }
        }
        if (progress in 1..99) LinearProgressIndicator(progress = { progress / 100f }, Modifier.fillMaxWidth(), color = g.accent)

        Box(Modifier.weight(1f).fillMaxWidth()) {
            val list = blocker
            if (list == null) CircularProgressIndicator(Modifier.align(Alignment.Center))
            else AndroidView(
                modifier = Modifier.fillMaxSize(),
                factory = { c ->
                    WebView(c).apply {
                        settings.javaScriptEnabled = true
                        settings.domStorageEnabled = true
                        settings.mediaPlaybackRequiresUserGesture = true
                        settings.setSupportMultipleWindows(true)
                        settings.javaScriptCanOpenWindowsAutomatically = false
                        settings.safeBrowsingEnabled = true
                        CookieManager.getInstance().setAcceptCookie(true)

                        webViewClient = object : WebViewClient() {
                            override fun shouldInterceptRequest(view: WebView, req: WebResourceRequest): WebResourceResponse? {
                                if (!blockAds || req.isForMainFrame || !list.blocks(req.url.host)) return null
                                view.post { blocked++ }
                                return WebResourceResponse("text/plain", "utf-8", ByteArrayInputStream(ByteArray(0)))
                            }

                            override fun shouldOverrideUrlLoading(view: WebView, req: WebResourceRequest): Boolean {
                                val u = req.url
                                if (u.scheme == "http" || u.scheme == "https") {
                                    // Redirects to ad domains that the page started by itself.
                                    if (blockAds && !req.hasGesture() && list.blocks(u.host)) { blocked++; return true }
                                    return false
                                }
                                if (req.hasGesture()) openOutside(c, u)?.let { view.loadUrl(it) } // intent:, market:, mailto: — only when tapped
                                return true
                            }

                            override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
                                page = url; address = url; blocked = 0
                            }

                            override fun doUpdateVisitedHistory(view: WebView, url: String, isReload: Boolean) {
                                page = url; address = url
                                canBack = view.canGoBack(); canForward = view.canGoForward()
                            }
                        }

                        webChromeClient = object : WebChromeClient() {
                            override fun onProgressChanged(view: WebView, p: Int) { progress = p }

                            // target="_blank" links you tap open here; pop-ups the page opens by itself are dropped.
                            override fun onCreateWindow(view: WebView, isDialog: Boolean, isUserGesture: Boolean, result: Message): Boolean {
                                if (!isUserGesture) { blocked++; return false }
                                val catcher = WebView(view.context)
                                catcher.webViewClient = object : WebViewClient() {
                                    override fun shouldOverrideUrlLoading(v: WebView, r: WebResourceRequest): Boolean {
                                        view.loadUrl(r.url.toString())
                                        v.destroy()
                                        return true
                                    }
                                }
                                (result.obj as WebView.WebViewTransport).webView = catcher
                                result.sendToTarget()
                                return true
                            }
                        }

                        setDownloadListener { url, ua, disposition, mime, _ ->
                            if (!URLUtil.isNetworkUrl(url)) {
                                Toast.makeText(c, "This file is made by the page itself and can't be downloaded separately", Toast.LENGTH_LONG).show()
                                return@setDownloadListener
                            }
                            val name = URLUtil.guessFileName(url, disposition, mime)
                            // Cookies are not stored with the download; Gale reads them from the browser when it connects.
                            val headers = mapOf("User-Agent" to ua, "Referer" to (this.url ?: url))
                            runCatching { Gale.add(url, name = name, headers = headers) }
                                .onSuccess { Toast.makeText(c, "Downloading $name", Toast.LENGTH_SHORT).show() }
                                .onFailure { Toast.makeText(c, it.message, Toast.LENGTH_LONG).show() }
                        }

                        loadUrl(page)
                        web = this
                    }
                },
            )
            if (mediaPage) ExtendedFloatingActionButton(
                onClick = { onMediaPage(page) },
                icon = { Icon(Icons.Filled.Download, null) },
                text = { Text("Download video") },
                containerColor = g.accent, contentColor = g.accentFg,
                modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
            )
        }

        Row(Modifier.fillMaxWidth().padding(horizontal = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { web?.goBack() }, enabled = canBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, "Back") }
            IconButton(onClick = { web?.goForward() }, enabled = canForward) { Icon(Icons.AutoMirrored.Filled.ArrowForward, "Forward") }
            IconButton(onClick = { web?.loadUrl(HOME) }) { Icon(Icons.Filled.Home, "Home") }
            Row(Modifier.weight(1f), horizontalArrangement = androidx.compose.foundation.layout.Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Filled.Shield, null, tint = if (blockAds) g.accent else g.fg3, modifier = Modifier.padding(end = 6.dp))
                Text(if (blockAds) "$blocked blocked" else "Ad blocking off", color = g.fg2, fontSize = 13.sp, modifier = Modifier.padding(end = 12.dp))
            }
        }
    }
}

/**
 * Hands a non-web link to the app that handles it, without letting the page pick a specific component. Returns the
 * link's web fallback (intent: links often carry one) when no app can open it.
 */
private fun openOutside(ctx: Context, u: Uri): String? {
    val intent = runCatching {
        if (u.scheme == "intent") Intent.parseUri(u.toString(), Intent.URI_INTENT_SCHEME) else Intent(Intent.ACTION_VIEW, u)
    }.getOrNull() ?: return null
    intent.component = null
    intent.selector = null
    intent.addCategory(Intent.CATEGORY_BROWSABLE)
    intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    return if (runCatching { ctx.startActivity(intent) }.isSuccess) null
    else intent.getStringExtra("browser_fallback_url")?.takeIf { URLUtil.isNetworkUrl(it) }
}
