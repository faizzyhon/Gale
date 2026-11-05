# Gale — listing copy (all stores and download sites)

Copy-paste text for every store and download site. Store-specific fields are in the other files in this folder.

## Basics

| Field | Value |
|---|---|
| Product name | Gale Download Manager |
| Short name | Gale |
| Version | 0.5.0 |
| Developer / publisher | Muhammad Faizan |
| Website | https://faizzyhon.com |
| Source code | https://github.com/faizzyhon/Gale |
| Support / bug reports | https://github.com/faizzyhon/Gale/issues |
| Support email | faizzyhon@gmail.com |
| Privacy policy | https://github.com/faizzyhon/Gale/blob/main/PRIVACY.md |
| Terms of use | https://github.com/faizzyhon/Gale/blob/main/TERMS.md |
| License | MIT (free and open source) |
| Price | Free — no ads, no in-app purchases, no account |
| Category | Utilities / Download managers (Chrome Web Store: Productivity → Tools) |
| Platform | Windows 10 (1809+) and Windows 11, 64-bit |
| Languages | English |
| Installer | `Gale_Setup_0.5.0.exe` (or `Gale_0.5.0_x64.msi`), all users (Program Files), asks for administrator approval; updates in place |

## One-line summary (≤ 80 characters)

Free, open-source download manager for Windows — faster, resumable downloads.

## Short description (≤ 132 characters, also the Chrome Web Store summary)

Send downloads, links and videos from Chrome to Gale, the free download manager for Windows.

## Long description

Gale is a free, open-source download manager for Windows 10 and 11. It makes downloads faster and more
reliable, and keeps them organised.

FASTER DOWNLOADS
Gale splits each file into up to 32 parts and downloads them at the same time, so you get the full speed of your
connection. If a server does not support this, Gale falls back to a normal download automatically.

NEVER START OVER
Pause any download and continue later. Downloads that were running when you closed Gale or restarted your PC carry
on by themselves. If your connection drops, Gale retries and picks up where it stopped. Every finished file is
checked before it is marked complete.

TORRENTS
Click a magnet or .torrent link in the browser, paste a magnet link, or open a .torrent file. Torrents download next
to your other files, with pause, resume and the same speed limit. Gale stops sharing when a torrent finishes.

WORKS WITH YOUR BROWSER
The Gale extension for Chrome and Edge sends downloads to Gale automatically. A "Download File Info" window lets you
pick the name, category and folder before a download starts. Right-click any link to download it with Gale, add it to
a queue, or download every link on a page.

SAVE VIDEOS
Hover a video on a web page and click Download to save it. In the app, paste a video page address and choose the
quality you want, from 1080p to audio-only. DRM-protected video is not supported.

STAY ORGANISED
Downloads are sorted into categories (video, music, documents, compressed files, applications) with their own
folders. Use queues to download files one after another, and the scheduler to start downloads at night and shut down
the PC when they finish.

MORE
- Live progress window for every download, with speed graph and per-connection view
- Speed limits and a per-server connection limit
- HTTP, HTTPS and SOCKS5 proxy support; proxy passwords stored in Windows Credential Manager
- Site grabber to download files linked from a website
- Starts with Windows in the tray, so browser downloads and schedules always work
- Optional Microsoft Defender scan of finished downloads
- Light and dark mode

PRIVATE BY DESIGN
No ads, no account, no tracking. Your download list stays on your PC. The source code is public on GitHub.

The browser extension needs the free Gale app for Windows:
https://github.com/faizzyhon/Gale/releases

## Feature list (for sites with bullet fields)

- Multi-connection downloads (up to 32 per file)
- Pause, resume, crash-safe recovery and automatic resume
- Chrome and Edge integration with a "Download File Info" window
- Video downloads in your choice of quality
- Torrent downloads: magnet links and .torrent files
- Categories, queues and a scheduler
- Speed limits, proxy support
- Per-download progress window with live speed graph
- Starts with Windows (can be turned off)
- Free and open source, no ads

## Keywords / tags

download manager, downloader, download accelerator, resume downloads, multi-connection, video downloader,
torrent downloader, magnet links, Chrome download, open source, free, Windows 11

## What's new in 0.5.0 (release notes)

- Torrent downloads: magnet links and .torrent files, from the browser, the clipboard, Add URL or a file on your PC
- Unfinished downloads resume by themselves when Gale starts; downloads that fail because the connection dropped are
  retried (Settings → General)
- The video quality menu in the browser is ready sooner: Gale reads the qualities as soon as a video page opens, or
  when you point at a streamed video
- Saved videos are named after the title only, without the video id
- Videos and audio go to the Video and Music categories
- The tray speed limit menu now shows a limit changed in Settings

## Developer bio (≤ 300 characters)

Muhammad Faizan is a full-stack developer based in Madinah, Saudi Arabia, building free tools for developers and
everyday users. Website: faizzyhon.com — GitHub: github.com/faizzyhon — LinkedIn: linkedin.com/in/faizzyhon

## System requirements

- Windows 10 version 1809 or later, or Windows 11 (64-bit)
- Microsoft Edge WebView2 Runtime (included with Windows 11 and current Windows 10; `Gale_Setup` installs it if missing)
- 400 MB free disk space
- Google Chrome 116+ or Microsoft Edge 116+ for the browser extension (optional)

## Assets

| File | Use |
|---|---|
| `assets/chrome/screenshot-1…5.png` (1280×800) | Chrome Web Store / Edge Add-ons screenshots |
| `assets/chrome/promo-small-440x280.png` | Chrome Web Store small promo tile (required) |
| `assets/chrome/promo-marquee-1400x560.png` | Chrome Web Store marquee (optional) |
| `assets/chrome/icon-128.png` | Store icon 128×128 |
| `assets/microsoft/screenshot-1…5.png` (1920×1080) | Microsoft Store and download sites |
| `assets/microsoft/store-logo-300x300.png` | Microsoft Store logo (1:1) |
| `assets/microsoft/hero-1920x1080.png` | Microsoft Store hero art / banner for download sites |
| `src-tauri/icons/icon.png` (512×512) | Large app icon |
