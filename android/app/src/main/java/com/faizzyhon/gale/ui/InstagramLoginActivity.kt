package com.faizzyhon.gale.ui

import android.annotation.SuppressLint
import android.app.Activity
import android.os.Bundle
import android.webkit.CookieManager
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast
import com.faizzyhon.gale.Gale
import java.io.File

/**
 * Instagram's own sign-in page in a WebView. Stories (and private accounts you follow) need a signed-in session;
 * once Instagram sets its session cookie, the cookies are written to an app-private file that only yt-dlp reads.
 * Gale never sees or stores the password.
 */
class InstagramLoginActivity : Activity() {
    private lateinit var web: WebView

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        CookieManager.getInstance().setAcceptCookie(true)
        web = WebView(this).apply {
            settings.javaScriptEnabled = true // Instagram's sign-in page needs it
            settings.domStorageEnabled = true
            webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView, url: String) {
                    val cookies = CookieManager.getInstance().getCookie("https://www.instagram.com").orEmpty()
                    if ("sessionid=" in cookies && "/accounts/login" !in url) {
                        CookieManager.getInstance().flush()
                        writeCookies(Gale.cookies, cookies)
                        Toast.makeText(this@InstagramLoginActivity, "Signed in to Instagram", Toast.LENGTH_SHORT).show()
                        setResult(RESULT_OK)
                        finish()
                    }
                }
            }
            loadUrl("https://www.instagram.com/accounts/login/")
        }
        setContentView(web)
    }

    override fun onDestroy() {
        web.destroy()
        super.onDestroy()
    }

    companion object {
        /** "a=1; b=2" from the WebView → the Netscape cookies.txt that yt-dlp reads. */
        fun netscape(cookieHeader: String, now: Long = System.currentTimeMillis() / 1000): String = buildString {
            append("# Netscape HTTP Cookie File\n")
            for (part in cookieHeader.split(';')) {
                val name = part.substringBefore('=').trim()
                val value = part.substringAfter('=', "").trim()
                if (name.isEmpty()) continue
                append(".instagram.com\tTRUE\t/\tTRUE\t${now + 365L * 24 * 3600}\t$name\t$value\n")
            }
        }

        fun writeCookies(file: File, cookieHeader: String) {
            file.writeText(netscape(cookieHeader))
        }

        fun signOut() {
            Gale.cookies.delete()
            CookieManager.getInstance().removeAllCookies(null)
            CookieManager.getInstance().flush()
        }
    }
}
