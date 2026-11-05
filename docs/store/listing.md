# Gale — listing copy (all stores and download sites)

Copy-paste text for every store and download site. Store-specific fields are in the other files in this folder.

## Basics

| Field | Value |
|---|---|
| Product name | Gale Download Manager |
| Short name | Gale |
| Version | 0.4.0 |
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
| Installer | `Gale_Setup_0.4.0.exe` (or `Gale_0.4.0_x64.msi`), all users (Program Files), asks for administrator approval; updates in place |

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
Pause any download and continue later, even after closing Gale or restarting your PC. If your connection drops,
Gale retries and picks up where it stopped. Every finished file is checked before it is marked complete.

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
- Pause, resume and crash-safe recovery
- Chrome and Edge integration with a "Download File Info" window
- Video downloads in your choice of quality
- Categories, queues and a scheduler
- Speed limits, proxy support
- Per-download progress window with live speed graph
- Starts with Windows (can be turned off)
- Free and open source, no ads

## Keywords / tags

download manager, downloader, download accelerator, resume downloads, multi-connection, video downloader,
Chrome download, open source, free, Windows 11

## What's new in 0.4.0 (release notes)

- New "Download File Info" window for downloads caught from the browser: choose name, category and folder first
- New progress window for each download, with live speed graph, per-connection view, pause/resume and a completion card
- Smoother animations throughout the app
- Gale now starts with Windows by default (in the tray); turn it off in Settings → General
- Quality menu on videos in the browser: pick a quality and the download starts without opening Gale
- Speed limit presets in the tray menu
- Press Ctrl+V in Gale, or drag links onto it, to add copied links
- New one-file setup, `Gale_Setup_0.4.0.exe`, which also installs Microsoft Edge WebView2 Runtime when a PC lacks it
- The installer now installs for all users in Program Files (administrator approval), updates an existing install in place, and replaces an older per-user install while keeping your downloads list
- New About page with developer, privacy and license information
- Browser extension: works in tabs that were open before an update, and the video button works on more players

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
