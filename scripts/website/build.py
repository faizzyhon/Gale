#!/usr/bin/env python3
"""Builds the Gale website into website/ (plain HTML, no build step needed to host it).

Run from the repo root: python3 scripts/website/build.py
Header, footer, version and download links live here once; edit the page bodies below.
The PayPal donation link is set in website/assets/site.js (PAYPAL_URL).
"""
import html
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "website"
VERSION = json.loads((ROOT / "package.json").read_text())["version"]
REPO = "https://github.com/faizzyhon/Gale"
REL = f"{REPO}/releases/download/v{VERSION}"
SETUP = f"Gale_Setup_{VERSION}.exe"
MSI = f"Gale_{VERSION}_x64.msi"
DEB = f"Gale_{VERSION}_amd64.deb"
RPM = f"Gale_{VERSION}_x86_64.rpm"
DMG = f"Gale_{VERSION}_universal.dmg"
EXT_ZIP = f"Gale-Chrome-Extension-{VERSION}.zip"
TITLE_SUFFIX = "Gale Download Manager"
# The contact address shown on the About and Disclaimer pages comes from the environment, so it isn't in the repo:
#   GALE_CONTACT_EMAIL=you@example.com python3 scripts/website/build.py
CONTACT_EMAIL = os.environ.get("GALE_CONTACT_EMAIL", "").strip()


def contact():
    return f'<span class="mono">{html.escape(CONTACT_EMAIL)}</span> or ' if CONTACT_EMAIL else ""


def installer_info(name):
    """Size and SHA-256 of a built installer, when dist-installer/ has it."""
    f = ROOT / "dist-installer" / name
    sha = ROOT / "dist-installer" / f"{name}.sha256"
    size = f"{f.stat().st_size / 1_000_000:.0f} MB" if f.exists() else ""
    digest = sha.read_text().split()[0] if sha.exists() else ""
    return size, digest


ICON = {
    "download": '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3v12"/><path d="m7 10 5 5 5-5"/><path d="M5 21h14"/></svg>',
    "book": '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 19.5V5a2 2 0 0 1 2-2h14v16H6a2 2 0 0 0-2 2.5Z"/><path d="M8 7h8"/></svg>',
    "coffee": '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M17 8h1a4 4 0 0 1 0 8h-1"/><path d="M3 8h14v9a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4Z"/><path d="M6 2v2M10 2v2M14 2v2"/></svg>',
    "github": '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 .5a12 12 0 0 0-3.8 23.4c.6.1.8-.3.8-.6v-2.1c-3.3.7-4-1.6-4-1.6-.6-1.4-1.4-1.8-1.4-1.8-1.1-.8.1-.8.1-.8 1.2.1 1.9 1.3 1.9 1.3 1.1 1.9 2.9 1.3 3.6 1 .1-.8.4-1.3.8-1.6-2.7-.3-5.5-1.3-5.5-6 0-1.3.5-2.4 1.2-3.2-.1-.3-.5-1.5.1-3.2 0 0 1-.3 3.3 1.2a11.5 11.5 0 0 1 6 0C17.3 4.3 18.3 4.6 18.3 4.6c.7 1.7.3 2.9.1 3.2.8.8 1.2 1.9 1.2 3.2 0 4.7-2.8 5.7-5.5 6 .4.4.8 1.1.8 2.2v3.3c0 .3.2.7.8.6A12 12 0 0 0 12 .5Z"/></svg>',
    "sun": '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>',
    "menu": '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16"/></svg>',
}

NAV = [("index.html#features", "Features"), ("index.html#speed", "Speed"), ("index.html#download", "Download"), ("index.html#guide", "Guide"), ("index.html#faq", "FAQ"), ("changelog.html", "What's new")]


def donate_btn(cls="btn btn-donate", label="Buy me a coffee"):
    return f'<a class="{cls}" href="support.html" data-donate>{ICON["coffee"]}<span>{label}</span></a>'


def page(file, title, description, body, wide_head=None):
    nav = "".join(
        f'<a href="{href}"{" aria-current=\"page\"" if href == file else ""}>{label}</a>' for href, label in NAV
    )
    full_title = TITLE_SUFFIX if file == "index.html" else f"{title} · {TITLE_SUFFIX}"
    doc = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{html.escape(full_title)}</title>
<meta name="claude:viewport-fit-sides" content="cover">
<meta name="description" content="{html.escape(description)}">
<meta name="author" content="Muhammad Faizan">
<meta property="og:title" content="{html.escape(full_title)}">
<meta property="og:description" content="{html.escape(description)}">
<meta property="og:image" content="assets/img/downloads-list.webp">
<link rel="icon" href="assets/favicon.png">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Familjen+Grotesk:wght@500;600;700&family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Sans:wght@400;500;600&display=swap">
<link rel="stylesheet" href="assets/site.css">
<script src="assets/site.js"></script>
</head>
<body>
<header class="topbar">
  <div class="wrap">
    <a class="brand" href="index.html"><img src="assets/favicon.png" alt="" width="30" height="30">Gale</a>
    <nav class="nav" id="nav" aria-label="Main">{nav}</nav>
    <div class="bar-actions">
      <a class="btn btn-secondary btn-small hide-sm" href="{REPO}">{ICON["github"]}Star</a>
      <a class="btn btn-primary btn-small hide-sm" href="index.html#download">Download</a>
      <button class="icon-btn" id="theme" type="button" aria-label="Switch light or dark theme">{ICON["sun"]}</button>
      <button class="icon-btn menu-toggle" id="menu" type="button" aria-label="Menu" aria-controls="nav" aria-expanded="false">{ICON["menu"]}</button>
    </div>
  </div>
</header>
<main>
{body}
</main>
<footer>
  <div class="wrap">
    <div class="foot">
      <div>
        <a class="brand" href="index.html"><img src="assets/favicon.png" alt="" width="30" height="30">Gale</a>
        <p class="small">A free, open-source download manager for Windows, Linux and macOS. Version {VERSION}.</p>
      </div>
      <div><h4>Product</h4><ul><li><a href="download.html">Download</a></li><li><a href="guide.html">User guide</a></li><li><a href="faq.html">FAQ</a></li><li><a href="changelog.html">What's new</a></li></ul></div>
      <div><h4>Project</h4><ul><li><a href="{REPO}">Source code</a></li><li><a href="{REPO}/releases">Releases</a></li><li><a href="{REPO}/issues">Report a problem</a></li><li><a href="support.html">Support Gale</a></li></ul></div>
      <div><h4>Legal</h4><ul><li><a href="privacy.html">Privacy policy</a></li><li><a href="terms.html">Terms of use</a></li><li><a href="disclaimer.html">Disclaimer</a></li><li><a href="{REPO}/blob/main/LICENSE">MIT License</a></li></ul></div>
      <div><h4>Developer</h4><ul><li><a href="about.html">About</a></li><li><a href="https://faizzyhon.com">faizzyhon.com</a></li><li><a href="https://github.com/faizzyhon">GitHub</a></li><li><a href="https://www.linkedin.com/in/faizzyhon/">LinkedIn</a></li></ul></div>
    </div>
    <div class="legal"><span>© 2026 Muhammad Faizan. Released under the MIT License.</span><span>Gale is independent software and is not affiliated with Google, Microsoft or any website it works with.</span></div>
  </div>
</footer>
</body>
</html>
"""
    (OUT / file).write_text(doc)


def shot(name, alt, w, h, cls="shot", extra=""):
    return f'<img class="{cls}" src="assets/img/{name}.webp" alt="{html.escape(alt)}" width="{w}" height="{h}" loading="lazy"{extra}>'


def fig(name, alt, w, h, caption, narrow=False):
    return f'<figure{" class=\"narrow\"" if narrow else ""}>{shot(name, alt, w, h, cls="")}<figcaption>{caption}</figcaption></figure>'


# --------------------------------------------------------------------------------------------- home
FIREFOX_ZIP = f"Gale-Firefox-Extension-{VERSION}.zip"
PORTABLE = f"Gale-{VERSION}-windows-x64-portable.zip"
BENCH_FILE = Path(__file__).with_name("benchmarks.json")


def bench():
    """Measured results (scripts/website/benchmarks.json, written from docs/benchmarks.md runs)."""
    return json.loads(BENCH_FILE.read_text()) if BENCH_FILE.exists() else None


def copy_block(cmd):
    return f'<div class="cmd"><code>{html.escape(cmd)}</code><button type="button" class="copy" data-copy="{html.escape(cmd)}" aria-label="Copy command">Copy</button></div>'


def bench_section(b):
    if not b:
        return ""
    tools = [("gale", "Gale"), ("aria2c", "aria2c"), ("curl", "curl")]
    panels = []
    for s in b["scenarios"]:
        best = max((r["mibps"] for r in s["results"].values() if r["ok"]), default=1) or 1
        rows = []
        for key, label in tools:
            r = s["results"][key]
            if r["ok"]:
                w = max(1.5, r["mibps"] / best * 100)
                rows.append(f'<div class="bar-row"><span class="who">{label}</span><span class="bar t-{key}"><i style="width:{w:.1f}%"></i></span><span class="val">{r["mibps"]:.1f} MiB/s · {r["seconds"]:.1f} s</span></div>')
            else:
                rows.append(f'<div class="bar-row"><span class="who">{label}</span><span class="bar t-{key} failed"></span><span class="val bad">{html.escape(r["note"])}</span></div>')
        panels.append(f"""
      <article class="bench-card">
        <header><span class="sid">{s["id"]}</span><h3>{html.escape(s["title"])}</h3><span class="verdict">{html.escape(s["verdict"])}</span></header>
        <p class="muted small">{html.escape(s["detail"])}</p>
        {"".join(rows)}
      </article>""")
    table_rows = "".join(
        f'<tr><td>{s["id"]}. {html.escape(s["title"])}</td>' + "".join(
            f'<td>{(f"{s["results"][k]["mibps"]:.1f} MiB/s ({s["results"][k]["seconds"]:.1f} s)") if s["results"][k]["ok"] else html.escape(s["results"][k]["note"])}</td>' for k, _ in tools
        ) + "</tr>" for s in b["scenarios"]
    )
    return f"""
<section id="speed">
  <div class="wrap">
    <div class="section-head">
      <span class="eyebrow">Speed test</span>
      <h2>Measured, not promised.</h2>
      <p class="lead">Gale, aria2c {html.escape(b["aria2"])} and curl {html.escape(b["curl"])} downloading the same files from a local test server. Every file was checked by SHA-256. Higher is better; each panel has its own scale.</p>
    </div>
    <div class="legend"><span><i class="t-gale"></i>Gale</span><span><i class="t-aria2c"></i>aria2c (16 connections)</span><span><i class="t-curl"></i>curl (one connection)</span></div>
    <div class="bench-grid">{"".join(panels)}</div>
    <div class="note-row">
      <p class="note"><b>Machine:</b> {html.escape(b["machine"])}. Measured {html.escape(b["date"])} with <code>scripts/bench/run.sh</code>; method and raw numbers in <a href="{REPO}/blob/main/docs/benchmarks.md">docs/benchmarks.md</a>.</p>
      <p class="note"><b>What this does not show:</b> on a normal home line the line itself is the limit, and all three tools reach it. Extra connections help when a server slows down each connection.</p>
    </div>
    <details class="raw"><summary>All results as a table</summary><div class="table-wrap"><table><tr><th>Test</th><th>Gale</th><th>aria2c</th><th>curl</th></tr>{table_rows}</table></div></details>
  </div>
</section>"""


def os_card(os_id, title, sub, files, note):
    rows = []
    for label, name, rec in files:
        size, _ = installer_info(name)
        rows.append(f'<a class="file-row" href="{REL}/{name}"><span><b>{label}</b>{" <em>Recommended</em>" if rec else ""}<small>{name}{f" · {size}" if size else ""}</small></span>{ICON["download"]}</a>')
    return f"""
      <div class="os-card" data-os-card="{os_id}">
        <span class="yours">Your system</span>
        <h3>{title}</h3>
        <p class="muted small">{sub}</p>
        <div class="file-list">{"".join(rows)}</div>
        <p class="small muted">{note}</p>
      </div>"""


def home():
    b = bench()
    a_gain = ""
    if b:
        a = b["scenarios"][0]["results"]
        if a["gale"]["ok"] and a["curl"]["ok"]:
            a_gain = f'{a["gale"]["mibps"] / a["curl"]["mibps"]:.0f}×'
    limit = b and b["scenarios"][2]["results"]["gale"]
    limit_pct = f'{min(100, limit["mibps"] / 8.0 * 100):.0f}%' if limit and limit["ok"] else ""
    stats = f"""
    <dl class="stats">
      <div><dt>{a_gain or "32"}</dt><dd>{"faster than one connection when the server slows each one down (test A)" if a_gain else "connections per file"}</dd></div>
      <div><dt>32</dt><dd>connections per file, fewer when a server asks for fewer</dd></div>
      <div><dt>{limit_pct or "0"}</dt><dd>{"of the most a 4-connection server allows, found by itself (test C)" if limit_pct else "ads"}</dd></div>
      <div><dt>0</dt><dd>ads, accounts and trackers</dd></div>
    </dl>"""
    guide_tabs = f"""
    <div class="tabs" data-tabs>
      <div class="tab-list" role="tablist" aria-label="Your system">
        <button role="tab" data-tab="windows" aria-selected="true">Windows</button>
        <button role="tab" data-tab="mac" aria-selected="false">macOS</button>
        <button role="tab" data-tab="linux" aria-selected="false">Linux</button>
      </div>
      <div class="tab-panel" data-panel="windows">
        <ol class="steps">
          <li><div><b>Run {SETUP}</b><p class="muted">Approve the administrator prompt once. If SmartScreen says "Windows protected your PC", click <b>More info → Run anyway</b>: the installer isn't code-signed yet.</p></div></li>
          <li><div><b>Gale opens</b><p class="muted">It sits in the tray and starts with Windows, so schedules and the browser extension always work.</p></div></li>
          <li><div><b>Silent install for IT</b>{copy_block(f"msiexec /i {MSI} /qn")}</div></li>
        </ol>
      </div>
      <div class="tab-panel" data-panel="mac" hidden>
        <ol class="steps">
          <li><div><b>Open {DMG} and drag Gale to Applications</b><p class="muted">Works on Apple Silicon and Intel Macs with macOS 11 or later.</p></div></li>
          <li><div><b>First start</b><p class="muted">The app isn't notarized yet, so right-click Gale and choose <b>Open</b> once, or run:</p>{copy_block("xattr -dr com.apple.quarantine /Applications/Gale.app")}</div></li>
          <li><div><b>Video downloads</b><p class="muted">Install FFmpeg; Gale fetches yt-dlp by itself.</p>{copy_block("brew install ffmpeg")}</div></li>
        </ol>
      </div>
      <div class="tab-panel" data-panel="linux" hidden>
        <ol class="steps">
          <li><div><b>Debian, Ubuntu, Mint, Pop!_OS</b>{copy_block(f"sudo apt install ./{DEB}")}</div></li>
          <li><div><b>Fedora, openSUSE, RHEL</b>{copy_block(f"sudo dnf install ./{RPM}")}</div></li>
          <li><div><b>Video downloads</b><p class="muted">FFmpeg comes from your distribution; Gale fetches yt-dlp by itself.</p>{copy_block("sudo apt install ffmpeg")}</div></li>
        </ol>
      </div>
    </div>"""
    ext_tabs = f"""
    <div class="tabs" data-tabs>
      <div class="tab-list" role="tablist" aria-label="Your browser">
        <button role="tab" data-tab="chrome" aria-selected="true">Chrome · Edge · Brave</button>
        <button role="tab" data-tab="firefox" aria-selected="false">Firefox</button>
      </div>
      <div class="tab-panel" data-panel="chrome">
        <ol class="steps">
          <li><div><b>Get the extension</b><p class="muted">On Windows it's already in Gale's folder (Start menu → Gale → <i>Install browser extension</i>). On macOS and Linux unzip <a href="{REL}/{EXT_ZIP}">{EXT_ZIP}</a>.</p></div></li>
          <li><div><b>Load it</b><p class="muted">Open <code>chrome://extensions</code> (or <code>edge://extensions</code>), turn on <b>Developer mode</b>, click <b>Load unpacked</b> and pick the folder.</p></div></li>
          <li><div><b>Check it says Connected</b><p class="muted">Click the Gale icon in the toolbar. Start Gale once before the browser so it can register itself.</p></div></li>
        </ol>
      </div>
      <div class="tab-panel" data-panel="firefox" hidden>
        <ol class="steps">
          <li><div><b>Download {FIREFOX_ZIP}</b><p class="muted">Firefox 128 or later. Start Gale once first: it registers its helper with Firefox.</p></div></li>
          <li><div><b>Load it</b><p class="muted">Open <code>about:debugging#/runtime/this-firefox</code>, click <b>Load Temporary Add-on</b> and pick the zip. Firefox keeps it until it restarts; a signed version for addons.mozilla.org is on the way.</p></div></li>
          <li><div><b>Allow it on all sites</b><p class="muted">In <code>about:addons</code> → Gale → <b>Permissions</b>, turn on access to all websites so it can catch downloads.</p></div></li>
        </ol>
      </div>
    </div>"""
    faq_items = "".join(f"<details><summary>{q}</summary><div><p>{a}</p></div></details>" for q, a in FAQ)
    body = f"""
<section class="hero2">
  <div class="wrap">
    <a class="pill" href="#models"><span>New in {VERSION}</span>Download AI models from Hugging Face</a>
    <h1>Fast downloads.<br>Nothing in the way.</h1>
    <p class="lead">Gale is a free, open-source download manager for Windows, macOS and Linux. It splits each file across up to 32 connections, takes over downloads and videos from your browser, and continues after a crash, a restart or an expired link.</p>
    <div class="btn-row">
      <a class="btn btn-primary" id="dl-main" href="#download">{ICON["download"]}<span>Download Gale</span></a>
      <a class="btn btn-secondary" href="#download">All downloads</a>
      <a class="btn btn-secondary" href="{REPO}">{ICON["github"]}Star on GitHub</a>
      {donate_btn("btn btn-secondary", "Donate")}
    </div>
    <p class="hero-meta" id="dl-meta">Version {VERSION} · Windows, macOS, Linux · MIT License</p>
    <ul class="checks"><li>Free and open source</li><li>No ads, no account, no tracking</li><li>Also on Android</li></ul>
    <figure class="hero-shot">{shot("downloads-list", "Gale's main window with downloads in progress and finished", 1280, 800, cls="main-shot", extra=' loading="eager"')}</figure>
    {stats}
  </div>
</section>

<section id="features" class="band">
  <div class="wrap">
    <div class="section-head">
      <span class="eyebrow">Features</span>
      <h2>A complete download manager. Nothing you didn't ask for.</h2>
    </div>

    <div class="feature">
      <div class="feature-copy">
        <span class="eyebrow">Download engine</span>
        <h3>Every connection busy until the last byte</h3>
        <p class="muted">Gale splits the file into parts and keeps all connections working: when one finishes early it takes half of the largest part still left. If a server only allows a few connections, Gale notices, uses that many and remembers it.</p>
        <ul><li>Up to 32 connections per file, written straight into the file</li><li>Resume after pause, crash or restart, checked with ETag and Last-Modified</li><li>Use Ethernet and Wi-Fi together for more total speed</li><li>Expired link? Refresh the address and keep the progress</li></ul>
      </div>
      <div class="sim" aria-label="Simulation of eight connections downloading one file">
        <div class="sim-head"><span>ubuntu-24.04-desktop-amd64.iso</span><b id="strip-read">8 connections · 0%</b></div>
        <div class="strip-bars">{"".join('<span><i style="--p:0%"></i></span>' for _ in range(8))}</div>
        <p class="small muted">Simulation: each bar is one connection fetching its own part of the file.</p>
      </div>
    </div>

    <div class="feature flip">
      <div class="feature-copy">
        <span class="eyebrow">Browser</span>
        <h3>Click a link. Gale has it.</h3>
        <p class="muted">The extension for Chrome, Edge, Brave and Firefox hands downloads to Gale with the name, size and resume support already known. Signed-in downloads keep working: cookies go only to the app on your computer.</p>
        <ul><li>Choose which file types and sizes Gale takes over</li><li>Right-click: download a link, queue it, or grab every link on the page</li><li>Hold Alt to let the browser handle one download itself</li></ul>
      </div>
      <figure>{shot("download-file-info", "The Download File Info window with address, category, file name and folder", 560, 380)}</figure>
    </div>

    <div class="feature">
      <div class="feature-copy">
        <span class="eyebrow">Video</span>
        <h3>Any video, the quality you pick</h3>
        <p class="muted">A Download button appears on the video. Pick the quality, or just the audio as MP3. Gale uses yt-dlp for over a thousand sites, joins video and sound into one file and keeps yt-dlp up to date.</p>
        <ul><li>Subtitles and chapters inside the file</li><li>Optional SponsorBlock to cut sponsor segments</li><li>HLS and DASH streams; DRM-protected video is not supported</li></ul>
      </div>
      <figure>{shot("video-quality", "Quality menu with 1080p, 720p, 480p and audio-only options", 1280, 800)}</figure>
    </div>

    <div class="feature flip" id="models">
      <div class="feature-copy">
        <span class="eyebrow">AI models · new in {VERSION}</span>
        <h3>Hugging Face models, just the files you need</h3>
        <p class="muted">Search for a model or paste its name. Gale shows each GGUF size (Q4_K_M is picked for you) or the weights with their configs, downloads them over many connections and checks every large file against Hugging Face's SHA-256.</p>
        <ul><li>Gated and private models with your access token, kept in the system keychain</li><li>Save straight into LM Studio's models folder</li><li>Mirror support (hf-mirror.com); files you already have are skipped</li></ul>
      </div>
      <figure>{shot("ai-models", "Gale's AI Models page with GGUF sizes of a model and Q4_K_M selected", 1280, 800)}</figure>
    </div>

    <div class="cards3">
      <article class="card"><h3>Queues and scheduler</h3><p class="muted">Run downloads a few at a time, schedule them for the night, and shut down the PC when everything is done.</p></article>
      <article class="card"><h3>Site grabber</h3><p class="muted">Give it a page and the file types you want; it follows links and lists every file to choose from. It respects robots.txt.</p></article>
      <article class="card"><h3>Torrents and magnets</h3><p class="muted">Magnet links and .torrent files open in Gale. Sharing stops when your download is done.</p></article>
      <article class="card"><h3>Light, dark, six accents</h3><p class="muted">Follows your system theme, or pick one. Six accent colours in Settings.</p></article>
      <article class="card"><h3>Out of the way</h3><p class="muted">Tray icon, a small progress window per download, notifications, and an optional clipboard watch for copied links.</p></article>
      <article class="card"><h3>Private by design</h3><p class="muted">No account, no analytics, no server of our own. Passwords and tokens stay in the system keychain. <a href="privacy.html">Privacy policy</a></p></article>
    </div>
  </div>
</section>
{bench_section(b)}
<section id="download" class="band">
  <div class="wrap">
    <div class="section-head">
      <span class="eyebrow">Download</span>
      <h2>Free, for every computer.</h2>
      <p class="lead">Version {VERSION}, straight from GitHub. The file we recommend for your system is marked.</p>
    </div>
    <div class="os-grid">
      {os_card("windows", "Windows", "Windows 10 (1809+) or 11, 64-bit", [("Setup", SETUP, True), ("MSI · all users", MSI, False), ("Portable · no install", PORTABLE, False)], "SmartScreen may warn the first time because the installer isn't code-signed yet: <b>More info → Run anyway</b>.")}
      {os_card("mac", "macOS", "macOS 11 or later · Apple Silicon and Intel", [("Universal", DMG, True)], "Not notarized yet: right-click Gale → <b>Open</b> the first time. Install FFmpeg with Homebrew for video.")}
      {os_card("linux", "Linux", "64-bit · deb or rpm", [("Debian, Ubuntu, Mint", DEB, True), ("Fedora, openSUSE, RHEL", RPM, False)], "Both register the browser helper for Chrome, Chromium, Edge, Brave and Firefox. Install ffmpeg for video.")}
    </div>
    <div class="cards2">
      <div class="card">
        <h3>Browser extension</h3>
        <p class="muted">Install the app first, then the extension. <a href="{REL}/{EXT_ZIP}">Chrome, Edge, Brave</a> · <a href="{REL}/{FIREFOX_ZIP}">Firefox</a> · <a href="#guide">how to install</a></p>
      </div>
      <div class="card">
        <h3>Check your download</h3>
        <p class="muted small">Every file has a <code>.sha256</code> next to it on the release page.</p>
        {copy_block(f"Get-FileHash .\\{SETUP}")}
        {copy_block(f"sha256sum {DEB}")}
      </div>
    </div>
    <p class="muted small">Android: <a href="{REPO}/releases">APK on the release page</a>. Older versions and the source code: <a href="{REPO}/releases">GitHub Releases</a>.</p>
  </div>
</section>

<section id="guide">
  <div class="wrap">
    <div class="section-head">
      <span class="eyebrow">Guide</span>
      <h2>Up and running in two minutes.</h2>
      <p class="lead">Install the app, add the browser extension, done. The <a href="guide.html">full guide</a> covers every feature.</p>
    </div>
    <h3 class="part"><span>Part 1</span>Install the app</h3>
    {guide_tabs}
    <h3 class="part"><span>Part 2</span>Add the browser extension</h3>
    {ext_tabs}
  </div>
</section>

<section id="faq" class="band">
  <div class="wrap faq-grid">
    <div class="section-head">
      <span class="eyebrow">FAQ</span>
      <h2>Questions, answered.</h2>
      <p class="muted">Something else? <a href="{REPO}/issues">Open an issue on GitHub</a>.</p>
    </div>
    <div class="faq">{faq_items}</div>
  </div>
</section>

<section class="closing">
  <div class="wrap">
    <img src="assets/icon-512.png" alt="" width="72" height="72">
    <h2>Fast downloads. Nothing in the way.</h2>
    <p class="muted">Free, open source and ready for your next download.</p>
    <div class="btn-row center">
      <a class="btn btn-primary" href="#download">{ICON["download"]}Download Gale</a>
      <a class="btn btn-secondary" href="{REPO}">{ICON["github"]}Star on GitHub</a>
      {donate_btn("btn btn-secondary", "Donate")}
    </div>
  </div>
</section>
"""
    page("index.html", "Home", "Gale is a free, open-source download manager for Windows, macOS and Linux: multi-connection downloads, resume, browser integration, video, torrents and Hugging Face models.", body)


# ----------------------------------------------------------------------------------------- download
def unix_card(title, name, how):
    size, digest = installer_info(name)
    hash_line = f'<p class="hash">SHA-256: {digest}</p>' if digest else ""
    how = f"<code>{html.escape(how)}</code>" if how.startswith("sudo") else html.escape(how)
    return f"""
    <div class="dl-card">
      <h3>{title}</h3>
      <p class="file">{name}{f" · {size}" if size else ""}</p>
      <p class="muted">{how}</p>
      {hash_line}
      <a class="btn btn-secondary" href="{REL}/{name}">{ICON["download"]}Download</a>
    </div>"""


def download():
    s_size, s_hash = installer_info(SETUP)
    m_size, m_hash = installer_info(MSI)
    hash_line = lambda h: f'<p class="hash">SHA-256: {h}</p>' if h else ""
    body = f"""
<div class="wrap">
  <div class="page-head">
    <span class="eyebrow">Download · version {VERSION}</span>
    <h1>Get Gale</h1>
    <p class="lead">Free for Windows, macOS and Linux. The file we recommend for your system is marked. On Windows one installer sets up Gale, the browser extension and the video tools; installing a newer version updates Gale in place and keeps your downloads list.</p>
  </div>
  <div class="os-grid" style="margin-bottom:56px">
    {os_card("windows", "Windows", "Windows 10 (1809+) or 11, 64-bit", [("Setup", SETUP, True), ("MSI · all users", MSI, False), ("Portable · no install", PORTABLE, False)], "SmartScreen may warn the first time: <b>More info → Run anyway</b>.")}
    {os_card("mac", "macOS", "macOS 11 or later · Apple Silicon and Intel", [("Universal", DMG, True)], "Right-click Gale → <b>Open</b> the first time.")}
    {os_card("linux", "Linux", "64-bit · deb or rpm", [("Debian, Ubuntu, Mint", DEB, True), ("Fedora, openSUSE, RHEL", RPM, False)], "Install ffmpeg for video downloads.")}
  </div>
  <p class="muted">Browser extension: <a href="{REL}/{EXT_ZIP}">Chrome, Edge, Brave</a> · <a href="{REL}/{FIREFOX_ZIP}">Firefox</a> · <a href="index.html#guide">how to install it</a>. Android: <a href="{REPO}/releases">APK on the release page</a>.</p>
  <h2 style="margin-top:56px">Windows files in detail</h2>
  <div class="dl-grid">
    <div class="dl-card primary">
      <span class="tag">Recommended</span>
      <h3>Setup</h3>
      <p class="file">{SETUP}{f" · {s_size}" if s_size else ""}</p>
      <p class="muted">The easiest way. Also installs Microsoft Edge WebView2 if your PC doesn't have it, then opens Gale.</p>
      {hash_line(s_hash)}
      <a class="btn btn-primary" href="{REL}/{SETUP}">{ICON["download"]}Download setup</a>
    </div>
    <div class="dl-card">
      <span class="tag">For IT and scripts</span>
      <h3>MSI package</h3>
      <p class="file">{MSI}{f" · {m_size}" if m_size else ""}</p>
      <p class="muted">The same installation as a Windows Installer package, for silent installs: <code>msiexec /i {MSI} /qn</code></p>
      {hash_line(m_hash)}
      <a class="btn btn-secondary" href="{REL}/{MSI}">{ICON["download"]}Download MSI</a>
    </div>
    <div class="dl-card">
      <span class="tag">Already included</span>
      <h3>Browser extension</h3>
      <p class="file">For Google Chrome and Microsoft Edge</p>
      <p class="muted">The installer puts the extension in Gale's folder. Connecting it takes about a minute; the guide shows each step.</p>
      <a class="btn btn-secondary" href="guide.html#browser">{ICON["book"]}Connect your browser</a>
    </div>
  </div>


  <div class="doc" style="grid-template-columns:minmax(0,1fr);padding-top:56px">
    <article>
      <section>
        <h2>System requirements</h2>
        <div class="table-wrap"><table>
          <tr><th>Windows</th><td>Windows 11, or Windows 10 version 1809 or later, 64-bit</td></tr>
          <tr><th>WebView2</th><td>Built into Windows 11 and current Windows 10. The setup installs it if it's missing.</td></tr>
          <tr><th>Disk space</th><td>About 400 MB</td></tr>
          <tr><th>Browser</th><td>Google Chrome or Microsoft Edge, version 116 or later (optional)</td></tr>
          <tr><th>Administrator</th><td>Needed once to install. Gale itself runs as a normal program.</td></tr>
        </table></div>
      </section>
      <section>
        <h2>"Windows protected your PC"</h2>
        <p>Gale's installer isn't code-signed yet, so Microsoft Defender SmartScreen may warn you the first time. Click <b>More info</b>, check that the file name is <code>{SETUP}</code>, then click <b>Run anyway</b>. You can compare the file with the SHA-256 shown above: in PowerShell run <code>Get-FileHash .\\{SETUP}</code>.</p>
      </section>
      <section>
        <h2>Older versions and source code</h2>
        <p>Every release, with its notes and files, is on <a href="{REPO}/releases">GitHub Releases</a>. The source code is at <a href="{REPO}">github.com/faizzyhon/Gale</a>.</p>
      </section>
    </article>
  </div>
</div>
"""
    page("download.html", "Download", f"Download Gale {VERSION} for Windows, Linux and macOS: setup, MSI, .deb, .rpm, .dmg and browser extension.", body)


# -------------------------------------------------------------------------------------------- guide
GUIDE_TOC = [
    ("install", "Install Gale"), ("first", "Your first download"), ("browser", "Connect your browser"),
    ("from-browser", "Downloads from the browser"), ("progress", "The progress window"), ("video", "Download videos"),
    ("torrent", "Download torrents"), ("manage", "Manage downloads"), ("queues", "Queues and scheduler"), ("grabber", "Site grabber"),
    ("settings", "Settings"), ("shortcuts", "Keyboard shortcuts"), ("trouble", "Troubleshooting"), ("uninstall", "Update and uninstall"),
]


def guide():
    toc = "".join(f'<a href="#{i}">{t}</a>' for i, t in GUIDE_TOC)
    body = f"""
<div class="wrap">
  <div class="page-head">
    <span class="eyebrow">User guide · Gale {VERSION}</span>
    <h1>How to use Gale</h1>
    <p class="lead">From installing to scheduling overnight downloads. Every picture on this page is a screenshot of Gale itself.</p>
  </div>
  <div class="doc">
    <nav class="toc" aria-label="Guide contents"><b>On this page</b>{toc}</nav>
    <article>

<section id="install">
  <h2>Install Gale</h2>
  <ol class="steps">
    <li><div><p>Download <code>{SETUP}</code> from the <a href="download.html">download page</a> and open it.</p></div></li>
    <li><div><p>Windows asks whether to allow the app to make changes. Click <b>Yes</b>. If SmartScreen says "Windows protected your PC", click <b>More info</b>, then <b>Run anyway</b>; the installer isn't code-signed yet.</p></div></li>
    <li><div><p>The setup installs Gale to <code>C:\\Program Files\\Gale</code>, adds it to the Start menu and desktop, and opens it. It takes under a minute.</p>
      {fig("installer", "Gale setup window installing version 0.4.0", 420, 243, "The setup shows its progress and closes by itself when Gale is installed.", narrow=True)}</div></li>
  </ol>
  <p class="note">Already have Gale? Run the new setup. It closes Gale, updates it in place and keeps your downloads list and settings.</p>
</section>

<section id="first">
  <h2>Your first download</h2>
  <p>When you open Gale for the first time the list is empty. The buttons at the top add downloads; the sidebar filters them by status and category.</p>
  {fig("first-launch", "Gale's main window with an empty download list", 1280, 800, "Gale on first launch.")}
  <ol class="steps">
    <li><div><p>Copy the address of a file, click <b>Add URL</b> (or press <kbd>Ctrl</kbd> + <kbd>N</kbd>) and paste it into <b>Address</b>. Quicker: press <kbd>Ctrl</kbd> + <kbd>V</kbd> anywhere in Gale's window, or drag a link from your browser onto Gale, and the dialog opens with the link filled in. Several links at once open <b>Add Batch</b>.</p></div></li>
    <li><div><p>Check the folder under <b>Save to</b>. Leave <b>File name</b> empty to use the name the server gives. Eight connections is a good default.</p></div></li>
    <li><div><p>If the site publishes a SHA-256 checksum, paste it in. Gale then marks the file complete only if it matches.</p></div></li>
    <li><div><p>Click <b>Start download</b>, or <b>Add paused</b> to start it later.</p>
      {fig("add-download", "Add download dialog with address, save folder, file name, connections, queue and SHA-256 fields", 1280, 800, "The Add download dialog.")}</div></li>
  </ol>
</section>

<section id="browser">
  <h2>Connect your browser</h2>
  <p>The extension sends downloads, links and videos from Chrome or Edge to Gale. It comes with Gale; you only switch it on once.</p>
  <ol class="steps">
    <li><div><p>Open the Start menu, open the <b>Gale</b> folder and click <b>Install browser extension</b>. The page that opens shows the extension's folder and a <b>Copy</b> button. The folder is <code>C:\\Program Files\\Gale\\browser-extension</code>.</p></div></li>
    <li><div><p>In Chrome go to <code>chrome://extensions</code> (in Edge, <code>edge://extensions</code>) and turn on <b>Developer mode</b>.</p>
      {fig("chrome-developer-mode", "Chrome extensions page with Developer mode switched on and the Load unpacked button", 1280, 110, "Developer mode is the switch at the top right. Load unpacked appears below the search box.")}</div></li>
    <li><div><p>Click <b>Load unpacked</b> and choose the <code>browser-extension</code> folder. Gale Download Manager appears in the list.</p>
      {fig("chrome-extension-card", "Gale Download Manager 0.4.0 card on the Chrome extensions page", 470, 240, "The extension after loading.", narrow=True)}</div></li>
    <li><div><p>Click the puzzle-piece icon in the toolbar and pin <b>Gale</b>. Click the Gale icon: it should say <b>Connected</b> while Gale is running.</p>
      {fig("extension-popup", "Gale extension popup showing Connected, version 0.4.0, and recent downloads", 380, 520, "The toolbar popup shows the connection, recent downloads, and quick switches.", narrow=True)}</div></li>
  </ol>
  <p class="note">Keep the folder where it is. The browser loads the extension from it, and Gale updates it when you install a new version.</p>
</section>

<section id="from-browser">
  <h2>Downloads from the browser</h2>
  <p>With the extension connected, click a download link as usual. Instead of Chrome's download bar you get Gale's <b>Download File Info</b> window.</p>
  {fig("download-file-info", "Download File Info window with address, category, save-as name and folder", 560, 380, "Download File Info: Gale has already checked the size and whether the download can resume.", narrow=True)}
  <ul>
    <li><b>Start download</b> begins at once and opens the progress window.</li>
    <li><b>Download later</b> adds it to your main queue.</li>
    <li><b>Cancel</b> drops it. Nothing is downloaded.</li>
    <li>Changing <b>Category</b> moves the file to that category's folder when "Sort downloads into category folders" is on.</li>
  </ul>
  <p>Right-click for more: on a link, <b>Download with Gale</b>, <b>Add to Gale queue</b> or <b>Copy download URL</b>; on selected text, <b>Download selected links with Gale</b>; on the page, <b>Download all links on this page with Gale</b> or <b>Download video on this page with Gale</b>.</p>
  <p>To choose which downloads Gale takes over, open the extension's options (right-click the Gale icon, then <b>Options</b>). You can limit it to certain file types or sizes, exclude sites, or turn off the Download File Info window so downloads start straight away.</p>
  {fig("extension-options", "Gale browser settings page with interception, file types, privacy and media options", 885, 841, "The extension's options page.", narrow=True)}
</section>

<section id="progress">
  <h2>The progress window</h2>
  <p>Each download gets its own small window. It shows the percentage, speed, time left and how many connections are working.</p>
  {fig("progress-window", "Progress window at 53% with speed graph and eight connection bars", 520, 520, "Show details opens the speed graph for the last minute and one bar per connection.", narrow=True)}
  <ul>
    <li><b>Pause</b> stops the download and keeps what's done. <b>Resume</b> continues from there, even days later.</li>
    <li><b>Cancel</b> stops it and deletes the unfinished part. Finished files are never deleted.</li>
    <li>You can close the window; the download keeps going in the main window.</li>
  </ul>
  <p>When the download finishes, the window shows where the file was saved.</p>
  {fig("download-complete", "Download complete card with Open folder, Close and Open buttons", 520, 330, "Open runs the file, Open folder shows it in File Explorer.", narrow=True)}
  <p class="note">Don't want these windows? Turn off "Show a progress window when a download starts" in Settings → General. You can also have them close automatically when a download finishes.</p>
</section>

<section id="video">
  <h2>Download videos</h2>
  <p>There are two ways to save a video.</p>
  <h3>From the web page</h3>
  <ol class="steps">
    <li><div><p>Hover over a video. A blue <b>Download ▾</b> button appears at its top right.</p></div></li>
    <li><div><p>Click it. Gale reads the video and lists the qualities with their estimated size: 1080p, 720p, 360p, and audio only as MP3 or in its original format.</p></div></li>
    <li><div><p>Click a quality. The download starts at once and its progress window opens; there's nothing to confirm in Gale.</p>
      {fig("video-button", "Quality menu over a video on a web page listing 1080p, 720p, 360p and audio-only options", 1280, 800, "The quality menu on a web page. Gale needs to be running; it starts in the tray with Windows.")}</div></li>
  </ol>
  <p>Plain video files (an .mp4 link, for example) show a single <b>Download</b> row instead. If Gale can't read a page, the menu offers <b>Open in Gale</b> so you can try in the app.</p>
  <h3>From Gale</h3>
  <ol class="steps">
    <li><div><p>Copy the address of the video page and click <b>Add Video</b>.</p></div></li>
    <li><div><p>Paste it and click <b>Get formats</b>. Gale reads the page and lists the qualities.</p></div></li>
    <li><div><p>Pick a quality, or audio only as MP3 or in its original format, check the folder and click <b>Download</b>.</p>
      {fig("video-quality", "Download video dialog with 1080p, 720p, 360p and audio-only choices", 1280, 800, "Gale downloads the video and sound separately and joins them into one file without re-encoding.")}</div></li>
  </ol>
  <p>Video downloads show their stage in the list: reading video info, downloading, merging, then verifying.</p>
  <p class="note warn">Only download videos you have the right to save. DRM-protected video (most paid streaming services) cannot be downloaded, and Gale doesn't try.</p>
</section>

<section id="torrent">
  <h2>Download torrents</h2>
  <p>Gale downloads magnet links and .torrent files with BitTorrent, next to your other downloads.</p>
  <ol class="steps">
    <li><div><p>In the browser, click a magnet link or a .torrent link. The extension sends it to Gale, which asks where to save it. When Gale isn't running, the link opens the way it normally would.</p></div></li>
    <li><div><p>In Gale, click <b>Add URL</b> and paste a magnet link, or click <b>.torrent file…</b> to pick a file on your PC.</p></div></li>
    <li><div><p>Gale first gets the torrent's details from other peers (for magnet links), then downloads. The list shows how many peers are connected. A torrent with several files gets its own folder.</p></div></li>
  </ol>
  <p>Pause and resume work like any other download, also after a restart: Gale checks the pieces already on disk and fetches only what's missing. Gale shares pieces with other peers while it downloads, which is how BitTorrent works, and stops sharing when the download finishes or is paused. The speed limit applies to torrents too.</p>
  <p>Turn off <b>Open magnet links and .torrent files in Gale</b> in the extension's options to keep using another torrent app from the browser.</p>
  <p class="note warn">Only download torrents you have the right to. Sharing copyrighted files without permission is illegal in most countries, and peers can see your IP address.</p>
</section>

<section id="manage">
  <h2>Manage downloads</h2>
  <p>The main list shows every download with its progress, size, speed, time left and status. Use the sidebar to see only what's in progress, finished, queued or failed, or one category such as Music or Compressed Files. The search box filters by name.</p>
  {fig("downloads-list", "Main download list with downloading, queued and completed files", 1280, 800, "Three downloads running, three finished. Speeds and times update every second.")}
  <p>Select a download to use the toolbar above the list, or right-click it for everything you can do with it.</p>
  {fig("right-click-menu", "Right-click menu with Open, Open containing folder, Pause, Restart, Cancel, Copy download URL, Recheck availability, Add to queue, Remove and Properties", 1280, 800, "The right-click menu.")}
  <ul>
    <li><b>Recheck availability</b> asks the server whether the file still exists and can resume.</li>
    <li><b>Remove</b> takes it off the list. Gale asks before deleting a downloaded file, and only deletes it if you tick that box.</li>
    <li><b>Properties</b> shows the address, size, checksum and a map of which parts are done.</li>
  </ul>
  <p>The <b>Dashboard</b> sums up your downloads, and <b>Download History</b> keeps a record of what finished, failed or was cancelled, with how long each took.</p>
  {fig("dashboard", "Dashboard with totals, a 14-day chart, downloads by category and recently downloaded files", 1280, 800, "The Dashboard.")}
  {fig("history", "Download history table with result, size, duration, finish time and address", 1280, 800, "Download History keeps finished, failed and cancelled downloads.")}
</section>

<section id="queues">
  <h2>Queues and scheduler</h2>
  <p>A queue runs its downloads a few at a time instead of all at once. Gale starts with a <b>Main queue</b>; add more with <b>New queue</b>.</p>
  {fig("queues", "Queues and Scheduler page with the main queue and an empty schedule list", 1280, 800, "Each queue shows how many downloads run at once and how many are done.")}
  <ol class="steps">
    <li><div><p>Add downloads to a queue: choose the queue in the Add download dialog, use <b>Download later</b> in the browser, or right-click a download and pick <b>Add to</b> a queue.</p></div></li>
    <li><div><p>Click <b>Edit</b> on a queue to set how many downloads run at the same time and how many connections they may use.</p></div></li>
    <li><div><p>Click <b>Add schedule</b> to start or stop a queue at a set time: once, every day, on weekdays or weekly. You can also have Gale shut down the PC when the queue finishes; a 60-second countdown lets you cancel.</p></div></li>
  </ol>
  <p>Need the bandwidth back for a while? Right-click Gale's tray icon and choose <b>Speed limit</b>: Unlimited, 256 KB/s, 1 MB/s or 5 MB/s. It applies to running downloads at once. Other values can be set in Settings → Network.</p>
  <p class="note">Schedules need Gale running. It starts with Windows in the tray by default, so they run even if you never open the window.</p>
</section>

<section id="grabber">
  <h2>Site grabber</h2>
  <p>The site grabber finds files linked from a web page, such as every PDF in a course page or every image in a gallery, and lets you choose which to download.</p>
  {fig("site-grabber", "Site Grabber page with address field, file type checkboxes, link depth and page limit", 1280, 800, "Choose the file types, how many links deep to follow and whether to stay on the same site.")}
  <ol class="steps">
    <li><div><p>Paste the page address and tick the file types you want. Add other extensions under <b>More extensions</b>.</p></div></li>
    <li><div><p>Keep <b>Link depth</b> at 1 to look only at that page, or raise it to follow links. <b>URL must contain</b> and <b>must not contain</b> narrow the results.</p></div></li>
    <li><div><p>Click <b>Find files</b>, tick the files to keep and click <b>Download</b>.</p></div></li>
  </ol>
  <p>The grabber respects each site's robots.txt, waits between pages, and never signs in or gets around access controls.</p>
</section>

<section id="settings">
  <h2>Settings</h2>
  <p>Open Settings from the bottom of the sidebar or the gear at the top right. Changes apply when you click <b>Save</b>.</p>
  {fig("settings", "General settings: theme, default download folder, category folders, notifications, start with Windows, progress windows and tray", 1280, 800, "Settings → General.")}
  <div class="table-wrap"><table>
    <tr><th>Tab</th><th>What's there</th></tr>
    <tr><td>General</td><td>Light or dark theme, default download folder, category folders, notifications, start with Windows, progress windows, keep running in the tray</td></tr>
    <tr><td>Downloads</td><td>Connections per download (1 to 32), how many downloads run at once, automatic retries</td></tr>
    <tr><td>Browser</td><td>Whether the extension is connected, sites and file types the browser should keep</td></tr>
    <tr><td>Network</td><td>Proxy (HTTP, HTTPS or SOCKS5), timeouts, total and per-download speed limits, connections per server, a connection test</td></tr>
    <tr><td>Security</td><td>Blocked file types, scanning finished downloads with Microsoft Defender</td></tr>
    <tr><td>Advanced</td><td>Video tools and <b>Update yt-dlp</b>, database backup and restore, logs, reset</td></tr>
  </table></div>
</section>

<section id="shortcuts">
  <h2>Keyboard shortcuts</h2>
  <div class="table-wrap"><table>
    <tr><th>Keys</th><th>Action</th></tr>
    <tr><td><kbd>Ctrl</kbd> + <kbd>N</kbd></td><td>Add a download</td></tr>
    <tr><td><kbd>Ctrl</kbd> + <kbd>V</kbd></td><td>Add the link you copied (several links open Add Batch)</td></tr>
    <tr><td><kbd>Space</kbd></td><td>Pause or resume the selected download</td></tr>
    <tr><td><kbd>Enter</kbd></td><td>Open the selected file when finished, otherwise show its properties</td></tr>
    <tr><td><kbd>Delete</kbd></td><td>Remove the selected download (asks first)</td></tr>
    <tr><td><kbd>Esc</kbd></td><td>Close a dialog</td></tr>
  </table></div>
</section>

<section id="trouble">
  <h2>Troubleshooting</h2>
  <div class="faq">
    <details><summary>The extension says "Gale is not installed"</summary><div><p>Install Gale with the setup, then restart the browser. The installer registers the connection the extension needs.</p></div></details>
    <details><summary>The extension says Gale is not running</summary><div><p>Start Gale from the Start menu. After that it sits in the tray (next to the clock). To have it ready every time, keep "Start Gale when I sign in to Windows" on in Settings → General.</p></div></details>
    <details><summary>"The Gale connection is owned by another program"</summary><div><p>Two copies of Gale are installed or running. Quit Gale from its tray icon (or end every <code>gale.exe</code> in Task Manager), run the latest setup, and start Gale again. The message names both programs so you can see which one is in the way.</p></div></details>
    <details><summary>A video won't download</summary><div><p>Video sites change often. Open Settings → Advanced and click <b>Update yt-dlp</b>, then try again. DRM-protected video can't be downloaded.</p></div></details>
    <details><summary>A download uses only one connection or starts over after a pause</summary><div><p>Some servers don't allow downloading a file in parts. Gale detects this and uses a single connection; a paused download from such a server has to restart. There's nothing to fix on your side.</p></div></details>
    <details><summary>Downloads from the browser aren't going to Gale</summary><div><p>Click the Gale toolbar icon and check that <b>Send downloads to Gale</b> is on and that it isn't paused. In the extension's options, check the file types, minimum size and excluded sites. Gale also leaves alone the sites and types listed in Settings → Browser.</p></div></details>
    <details><summary>Downloads are slower than expected</summary><div><p>Check for speed limits in Settings → Network. Raising connections per download (Settings → Downloads) helps on fast lines; some servers limit connections, which Gale respects.</p></div></details>
    <details><summary>Where are my settings and download list?</summary><div><p>In <code>%APPDATA%\\com.faizzyhon.gale</code>. Back them up from Settings → Advanced.</p></div></details>
  </div>
  <p>Still stuck? <a href="{REPO}/issues">Report the problem on GitHub</a> with what you did and what Gale showed.</p>
</section>

<section id="uninstall">
  <h2>Update and uninstall</h2>
  <p><b>To update</b>, download the latest setup and run it. Gale closes, updates in place and keeps your list and settings.</p>
  <p><b>To uninstall</b>, open Windows Settings → Apps → Installed apps, find <b>Gale Download Manager</b> and choose Uninstall. This removes the program, its Start menu entries, the browser connection and the start-with-Windows entry. Your downloaded files and your list in <code>%APPDATA%\\com.faizzyhon.gale</code> are kept; delete that folder to remove everything. Remove the extension from <code>chrome://extensions</code> as well.</p>
</section>

    </article>
  </div>
</div>
"""
    page("guide.html", "User guide", "Step-by-step guide to Gale: installing, downloading, browser extension, videos, queues, scheduler, settings and troubleshooting.", body)


# ---------------------------------------------------------------------------------------------- faq
FAQ = [
    ("Is Gale really free?", "Yes. Gale is free for personal and commercial use, with no ads, no trial and no paid version. It's open source under the MIT License. Donations are welcome but never required."),
    ("Is Gale safe?", "The whole source code is public on GitHub, so anyone can check what it does. Gale doesn't collect data, has no account and contacts only the servers you download from. The installer isn't code-signed yet, which is why SmartScreen may warn about it; you can compare the file's SHA-256 with the one on the download page."),
    ("How is it faster than downloading in the browser?", "Gale opens several connections to the server and downloads different parts of the file at the same time. Many servers limit the speed of each connection, so several together use more of your bandwidth. If a server allows only one connection, Gale uses one."),
    ("Can it resume a download that stopped?", "Yes, if the server supports it, which most do. Paused or interrupted downloads continue where they stopped, even after closing Gale or restarting the PC. Gale checks that the file on the server hasn't changed before continuing."),
    ("Which browsers does it work with?", "Chrome, Edge and Brave (version 116 or later) and Firefox 128 or later, on Windows, macOS and Linux. Other Chromium browsers that read Chrome's settings, such as Vivaldi and Opera, usually work too but aren't tested."),
    ("Can it download AI models from Hugging Face?", "Yes, from version 0.7.0. Open AI Models, search or paste a model name, and pick a GGUF size or the weights. Gale checks every large file against the SHA-256 Hugging Face lists, can save straight into LM Studio's models folder, and uses your access token (kept in the system keychain) for gated models."),
    ("Can I download YouTube videos?", "Gale can save videos from YouTube and most other video sites in the quality you choose. Downloading may be against a site's terms, and some content is protected by copyright: only download what you have the right to. DRM-protected video (Netflix, most paid services) can't be downloaded."),
    ("Can Gale download torrents?", "Yes. Click a magnet or .torrent link in the browser, paste a magnet link into Add URL, or open a .torrent file from your PC. Gale stops sharing a torrent once it has finished downloading. Peers in a torrent can see your IP address, and you are responsible for what you download and share."),
    ("Do downloads continue after a restart?", "Yes. Downloads that were running when Gale closed or the PC restarted start again by themselves when Gale starts, and a download that failed because the connection dropped is retried a few times. Turn this off in Settings → General."),
    ("Does Gale run on macOS or Linux?", "Yes, from version 0.6.0: a .deb for Debian and Ubuntu, an .rpm for Fedora and openSUSE, and a .dmg for macOS 11 or later, on the download page. Windows-only parts (Defender scan, the setup's WebView2 install, automatic install of updates) are not in those versions."),
    ("Can Gale use two internet connections at once?", "Yes. In Settings → Network pick two or more networks, for example Ethernet and Wi-Fi. Each download's connections are spread over them, so the speeds add up when the server allows several connections. If one network drops, its connections move to the others."),
    ("A download stopped because its link expired. Do I have to start again?", "No. Right-click it and choose Refresh download address. Paste a new link, or let Gale open the website so you can start the download again there; Gale checks it's the same file and goes on from where it stopped."),
    ("Does it need administrator rights?", "Only once, to install into Program Files for all users. Gale itself runs as a normal program."),
    ("Does Gale update itself?", "On Windows, yes: from version 0.6.0 Gale tells you when a new version is out, and Install replaces the old version and restarts Gale (your list is kept). On macOS and Linux it opens the release page; install the new package the same way as the first time."),
    ("Is there a portable version?", "Yes, for Windows: the portable ZIP runs without installing. It registers its browser helper for your user and doesn't add itself to Windows startup unless you turn that on."),
    ("Where does Gale save files?", "In Downloads\\Gale by default. Change it in Settings → General, or per download in the Add download dialog. With category folders on, videos, music and documents go into their own subfolders."),
    ("How can I support Gale?", 'Star the project on GitHub, tell a friend, report problems you find, or <a href="support.html">buy the developer a coffee</a>.'),
]


def faq():
    items = "".join(f"<details><summary>{q}</summary><div><p>{a}</p></div></details>" for q, a in FAQ)
    body = f"""
<div class="wrap">
  <div class="page-head"><span class="eyebrow">FAQ</span><h1>Questions people ask</h1><p class="lead">Something missing? Check the <a href="guide.html">user guide</a> or <a href="{REPO}/issues">ask on GitHub</a>.</p></div>
  <div class="faq" style="padding-bottom:72px">{items}</div>
</div>
"""
    page("faq.html", "FAQ", "Answers to common questions about Gale Download Manager.", body)


# ---------------------------------------------------------------------------------------- changelog
LOG = [
    ("0.7.0", "November 2026", [
        "AI Models: search Hugging Face or paste a model, pick a GGUF size or the weights, and download with SHA-256 checks; gated models with your token; save into LM Studio's folder",
        "Servers that allow only a few connections are detected: Gale uses what the server accepts and remembers it",
        "Videos: subtitles and chapters inside the file, and optional SponsorBlock",
        "Firefox extension, and the browser helper registers with Firefox on Windows, macOS and Linux",
        "Portable ZIP for Windows",
        "Clipboard watch (off by default) and six accent colours",
        "Cookies and sign-in headers are no longer sent along when a download redirects to another site",
    ]),
    ("0.6.0", "November 2026", [
        "Gale for Linux (.deb and .rpm) and macOS (universal .dmg)",
        "Refresh a download whose link stopped working: paste a new address or get a fresh one from the website, and the download goes on from where it stopped",
        "Use several networks at once: pick Ethernet, Wi-Fi and a second Wi-Fi adapter in Settings → Network and the connections of each download are spread over them",
        "Gale tells you when a new version is out; on Windows, Install replaces the old version and restarts Gale",
        "The browser helper only talks to the Gale program of the signed-in user",
    ]),
    ("0.5.0", "November 2026", [
        "Torrent downloads: magnet links and .torrent files, from the browser, the clipboard or a file on your PC",
        "Unfinished downloads resume by themselves when Gale starts, and downloads that failed because the connection dropped are retried",
        "The video quality menu is ready sooner: Gale reads the qualities when a video page opens or when you point at a video",
        "Saved videos are named after the title only, without the video id",
        "Videos and audio go to the Video and Music categories",
        "The tray speed limit menu shows the limit set in Settings",
    ]),
    ("0.4.0", "November 2026", [
        "New one-file setup that also installs Microsoft Edge WebView2 when it's missing, then opens Gale",
        "Installs for all users in Program Files with one administrator prompt",
        "Installing a new version updates Gale in place, closes a running copy first and keeps your list",
        "Replaces older per-user installs automatically and fixes browser connection problems they could cause",
        "Quality menu on videos in the browser: pick 1080p, 720p, 360p or audio and the download starts, without opening Gale",
        "Speed limit presets in the tray menu",
        "Press Ctrl+V in Gale, or drag links onto it, to add copied links",
        "Video tools can now be updated without administrator rights",
    ]),
    ("0.3.0", "November 2026", [
        "Download File Info window for downloads caught from the browser",
        "A progress window for every download, with speed graph, per-connection view and a completion card",
        "Smoother animations across the app",
        "About page, privacy policy and terms of use",
        "Starts with Windows in the tray by default",
        "Browser extension keeps working after it's updated, and the video button works on more players",
    ]),
    ("0.2.0", "November 2026", [
        "Video downloads from YouTube and most video sites, plus HLS and DASH streams",
        "Choose 1080p, 720p, 360p or audio only (MP3 or original); video and sound joined without re-encoding",
        "Download button on videos in the browser",
    ]),
    ("0.1.0", "November 2026", [
        "First release: multi-connection downloads with resume and verification",
        "Chrome and Edge extension, queues, scheduler, site grabber, speed limits and proxy support",
    ]),
]


def changelog():
    items = "".join(
        f'<article><div><div class="ver">{v}</div><time>{d}</time></div><ul>{"".join(f"<li>{x}</li>" for x in xs)}</ul></article>'
        for v, d, xs in LOG
    )
    body = f"""
<div class="wrap">
  <div class="page-head"><span class="eyebrow">What's new</span><h1>Release notes</h1><p class="lead">Every release with its files is also on <a href="{REPO}/releases">GitHub Releases</a>.</p></div>
  <div class="log" style="padding-bottom:72px">{items}</div>
</div>
"""
    page("changelog.html", "What's new", "Release notes for every version of Gale Download Manager.", body)


# -------------------------------------------------------------------------------------------- about
def about():
    body = f"""
<div class="wrap">
  <div class="page-head"><span class="eyebrow">About</span><h1>About Gale</h1></div>
  <div class="prose" style="padding-bottom:72px">
    <p class="lead">Gale is a free download manager for Windows, made for people who want fast, reliable downloads without paying for a licence or putting up with ads.</p>
    <p>Many download managers for Windows are paid or come with ads. Gale was started as a free alternative that anyone can use, read and improve. It does the things people rely on a download manager for: faster downloads over several connections, resuming after interruptions, taking over browser downloads, saving videos, and running queues on a schedule.</p>
    <p>Gale is built with Rust for the download engine, Tauri for the Windows app, and React for the interface. Video downloads use the open-source tools yt-dlp and FFmpeg.</p>

    <h2>Developer</h2>
    <div class="dev">
      <div class="avatar" aria-hidden="true">MF</div>
      <div style="display:flex;flex-direction:column;gap:10px;min-width:0">
        <h3>Muhammad Faizan</h3>
        <p class="muted">Full-stack developer based in Madinah, Saudi Arabia. He builds free tools for developers and everyday users, and Gale is his open-source download manager.</p>
        <div class="btn-row">
          <a class="btn btn-secondary btn-small" href="https://faizzyhon.com">faizzyhon.com</a>
          <a class="btn btn-secondary btn-small" href="https://github.com/faizzyhon">{ICON["github"]}GitHub</a>
          <a class="btn btn-secondary btn-small" href="https://www.linkedin.com/in/faizzyhon/">LinkedIn</a>
        </div>
      </div>
    </div>

    <h2>Open source</h2>
    <p>Gale is released under the <a href="{REPO}/blob/main/LICENSE">MIT License</a>. The code, issue tracker and releases are on <a href="{REPO}">GitHub</a>. Bug reports and pull requests are welcome.</p>

    <h2>Credits</h2>
    <p>Gale stands on the work of many open-source projects, including Rust, Tokio, reqwest, SQLx, Tauri, React and Framer Motion. Torrents use <a href="https://github.com/ikatson/rqbit">librqbit</a> (Apache 2.0). It ships <a href="https://github.com/yt-dlp/yt-dlp">yt-dlp</a> (Unlicense) and <a href="https://ffmpeg.org">FFmpeg</a> (LGPL); their licences are installed with Gale and listed in <a href="{REPO}/blob/main/scripts/installer/THIRD-PARTY-NOTICES.txt">THIRD-PARTY-NOTICES</a>.</p>

    <h2>Contact</h2>
    <p>For bugs and feature requests, please <a href="{REPO}/issues">open an issue on GitHub</a> so others can find the answer too. For anything else: {contact()}<a href="https://faizzyhon.com">faizzyhon.com</a>.</p>
  </div>
</div>
"""
    page("about.html", "About", "About Gale Download Manager and its developer, Muhammad Faizan.", body)


# ------------------------------------------------------------------------------------------ support
def support():
    body = f"""
<div class="wrap">
  <div class="page-head"><span class="eyebrow">Support Gale</span><h1>Buy me a coffee</h1>
    <p class="lead">Gale is free and has no ads. If it saves you time, a small donation helps cover code signing, testing machines and the hours that go into each release.</p></div>
  <div class="prose" style="padding-bottom:72px">
    <div class="coffee">
      <div><h2>Donate with PayPal</h2><p>Any amount helps. PayPal accepts cards too, no account needed in most countries.</p></div>
      <div class="btn-row">{donate_btn("btn btn-donate", "Donate with PayPal")}</div>
    </div>
    <h2>Other ways to help</h2>
    <ul>
      <li><a href="{REPO}">Star Gale on GitHub</a> so more people find it.</li>
      <li><a href="{REPO}/issues">Report bugs</a> with the steps to reproduce them.</li>
      <li>Share Gale with friends who still pay for a download manager.</li>
      <li>Developers: pull requests are welcome.</li>
    </ul>
    <p class="muted">Donations are gifts. They don't buy a licence, support contract or extra features, and they aren't refundable. Thank you.</p>
  </div>
</div>
"""
    page("support.html", "Support Gale", "Support Gale Download Manager with a PayPal donation.", body)


# ---------------------------------------------------------------------------------------------- legal
def md_to_html(md):
    """Tiny converter for PRIVACY.md / TERMS.md: headings, paragraphs, bullet lists, bold, code, links."""
    import re

    def inline(t):
        t = html.escape(t, quote=False)
        t = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", t)
        t = re.sub(r"`(.+?)`", r"<code>\1</code>", t)
        t = re.sub(r"\[(.+?)\]\((.+?)\)", lambda m: f'<a href="{fix_link(m.group(2))}">{m.group(1)}</a>', t)
        t = re.sub(r"&lt;(https?://[^&]+)&gt;", r'<a href="\1">\1</a>', t)
        return t

    def fix_link(href):
        return {"PRIVACY.md": "privacy.html", "TERMS.md": "terms.html", "LICENSE": f"{REPO}/blob/main/LICENSE"}.get(
            href, href if href.startswith("http") else f"{REPO}/blob/main/{href}"
        )

    out, para, items = [], [], []

    def flush():
        if para:
            out.append(f"<p>{inline(' '.join(para))}</p>")
            para.clear()
        if items:
            out.append("<ul>" + "".join(f"<li>{inline(i)}</li>" for i in items) + "</ul>")
            items.clear()

    for line in md.splitlines():
        s = line.strip()
        if s.startswith("# "):
            flush()
        elif s.startswith("## "):
            flush()
            out.append(f"<h2>{inline(s[3:])}</h2>")
        elif s.startswith("- "):
            if para:
                flush()
            items.append(s[2:])
        elif not s:
            flush()
        elif items:
            items[-1] += " " + s
        else:
            para.append(s)
    flush()
    return "\n".join(out)


def legal(file, md_name, title, desc):
    md = (ROOT / md_name).read_text()
    body = f"""
<div class="wrap">
  <div class="page-head"><span class="eyebrow">Legal</span><h1>{title}</h1></div>
  <div class="prose" style="padding-bottom:72px">{md_to_html(md)}</div>
</div>
"""
    page(file, title, desc, body)


def disclaimer():
    body = f"""
<div class="wrap">
  <div class="page-head"><span class="eyebrow">Legal</span><h1>Disclaimer</h1><p class="muted">Effective 5 November 2026</p></div>
  <div class="prose" style="padding-bottom:72px">
    <h2>Use at your own risk</h2>
    <p>Gale is provided free of charge and "as is", without warranty of any kind. The developer does not guarantee that Gale is free of errors, that downloads will succeed, or that it suits a particular purpose. Keep backups of important files, and scan files you download from the internet.</p>
    <h2>Your responsibility for what you download</h2>
    <p>Gale is a tool for downloading files you are allowed to download. You are solely responsible for what you download and how you use it, including respecting copyright, licences and the terms of the websites you visit. Downloading videos or other content may be prohibited by a website's terms or by the law where you live. The developer does not encourage or condone copyright infringement.</p>
    <h2>No circumvention</h2>
    <p>Gale does not bypass DRM, paywalls, logins or other access controls, and refuses DRM-protected media. Do not use Gale, or modified versions of it, to get around such protections.</p>
    <h2>Third-party websites and trademarks</h2>
    <p>Gale is independent software. It is not affiliated with, endorsed by or sponsored by Google, YouTube, Microsoft or any website or service it can download from. All product names, logos and trademarks belong to their owners and are used only to describe compatibility.</p>
    <h2>Third-party components</h2>
    <p>Gale includes open-source components, such as yt-dlp and FFmpeg, that are covered by their own licences and provided by their own authors. See <a href="{REPO}/blob/main/scripts/installer/THIRD-PARTY-NOTICES.txt">THIRD-PARTY-NOTICES</a>.</p>
    <h2>Limitation of liability</h2>
    <p>To the fullest extent allowed by law, the developer is not liable for any loss or damage arising from the use of, or inability to use, Gale or this website, including lost data, lost profits or damage to your device. See the <a href="terms.html">terms of use</a> for the full terms.</p>
    <h2>Website content</h2>
    <p>Information on this website is provided for general guidance and may change without notice. Screenshots show Gale {VERSION}; other versions may look different.</p>
    <h2>Contact</h2>
    <p>{contact()}<a href="{REPO}/issues">GitHub issues</a>.</p>
  </div>
</div>
"""
    page("disclaimer.html", "Disclaimer", "Disclaimer for Gale Download Manager and this website.", body)


def not_found():
    body = """
<div class="wrap"><div class="page-head" style="padding-block:96px">
  <span class="eyebrow">404</span><h1>This page isn't here.</h1>
  <p class="lead">The link may be old. Try the home page or the user guide.</p>
  <div class="btn-row"><a class="btn btn-primary" href="index.html">Go to the home page</a><a class="btn btn-secondary" href="guide.html">Open the guide</a></div>
</div></div>
"""
    page("404.html", "Page not found", "Page not found.", body)


if __name__ == "__main__":
    OUT.mkdir(exist_ok=True)
    home(); download(); guide(); faq(); changelog(); about(); support(); disclaimer(); not_found()
    legal("privacy.html", "PRIVACY.md", "Privacy policy", "How Gale Download Manager handles your data: everything stays on your PC.")
    legal("terms.html", "TERMS.md", "Terms of use", "Terms of use for Gale Download Manager.")
    (OUT / "robots.txt").write_text("User-agent: *\nAllow: /\n")
    print("Built", len(list(OUT.glob("*.html"))), "pages into", os.path.relpath(OUT, ROOT))
