# Gale Download Manager

Free, open-source download manager for Windows 10 and 11, with a Chrome/Edge extension.
Developed by **Muhammad Faizan** — [faizzyhon.com](https://faizzyhon.com) · [GitHub](https://github.com/faizzyhon) · [LinkedIn](https://www.linkedin.com/in/faizzyhon/)

Built with a Rust multi-connection engine, SQLite,
Tauri 2 + React 19 UI. Architecture: [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md).

## Install (Windows 11)

1. Run `Gale_Setup_0.5.0.exe` (or `Gale_0.5.0_x64.msi`) and approve the administrator prompt. The setup also installs
   Microsoft Edge WebView2 Runtime if the PC lacks it. Gale installs to `C:\Program Files\Gale` for all users.
   Running a newer (or the same) installer updates Gale in place; an older per-user Gale is removed automatically and your download list is kept.
   The build isn't code-signed yet, so SmartScreen may say "Windows protected your PC": choose **More info → Run anyway**.
2. Start menu → **Gale** → **Install browser extension** and follow the three steps
   (Chrome or Edge: Developer mode → Load unpacked → the folder shown on that page).
3. The extension popup should say **Connected** while Gale is running.

Gale starts with Windows in the tray so browser downloads and schedules always work; turn this off in Settings → General.

Uninstall from Settings → Apps. Your download list/history (`%APPDATA%\com.faizzyhon.gale`) and downloaded files are kept.

## Features

| Area | What's in |
|---|---|
| Engine | 1–32 connections, verified range support with single-connection fallback, work-stealing segments, If-Range/ETag protection, retry with back-off, pause/resume across restarts and crashes, automatic resume on start and retry after network errors, SHA-256 verification, total/per-download speed limits, per-server connection cap, HTTP/HTTPS/SOCKS5 proxy with Credential Manager password |
| App | Download table with all row actions, properties with segment map, categories, search, dashboard, history, batch add, queues (limits, priority, start/pause/stop), scheduler (once/daily/weekdays/weekly, shutdown after with 60 s cancel), site grabber (robots.txt, depth, filters), notifications, settings (General, Downloads, Browser, Network, Security, Advanced), DB backup/restore |
| Windows (UI) | Browser-captured downloads open a small "Download File Info" popup (category, file name, folder, size/resume check; Start, Download later, Cancel). Each started download gets its own progress window: live %, speed, time left, connections, a details panel with a speed graph and per-connection bars, pause/resume/cancel, and a completion card with Open / Open folder. Both can be turned off in Settings → General |
| Windows | Tray + close-to-tray, toasts, start with Windows, single instance, Mark-of-the-Web, optional Defender scan, all-users MSI with in-place updates, Chrome + Edge native messaging registration, clean uninstall |
| Video | YouTube and other video sites, HLS and DASH streams: pick 1080p/720p/360p or audio-only (MP3 or original); separate audio/video merged without re-encoding; verified with FFprobe. Uses bundled yt-dlp + FFmpeg. See `docs/media/VIDEO-DOWNLOADS.md` |
| Torrent | Magnet links and .torrent files (link or local file) through librqbit: DHT, trackers, multi-file torrents in their own folder, pause/resume with piece recheck, speed limit, no seeding after completion |
| Browser | Download interception by type/size/site with bypass & pause, "ask first" mode, context menus (link, queue, media, selected links, all links, copy URL), floating Download button on `<video>`/`<audio>` with a quality menu that is fetched as soon as a video page opens (DRM-protected media refused), magnet and .torrent capture, popup with status/recent/diagnostics, options page |

Not included: DRM-protected video, live streams, playlists, FTP, auto-update, languages other than English. See `docs/security/AUDIT.md` → Known limits.

## Build

```bash
pnpm install && (cd browser-extension && pnpm install)
pnpm tauri dev                          # run the app (Windows, macOS or Linux)
cargo test --workspace                  # engine, DB, queues, protocol, native host
(cd browser-extension && pnpm build)    # → browser-extension/dist (load unpacked)
scripts/installer/build-msi.sh          # Linux: cross-compiles and builds the MSI (cargo-xwin + msitools)
pnpm tauri build                        # on Windows: Tauri's own installers
```

Testing details: `docs/testing/TESTING.md`; Windows sign-off list: `docs/testing/WINDOWS-CHECKLIST.md`.

## Layout

```
crates/gale-core/     engine, media pipeline (yt-dlp/FFmpeg), torrents (librqbit), limits, queues/scheduler, grabber, DB, protocol, IPC, security
crates/gale-nmh/      native messaging host (gale-nmh.exe)
src-tauri/            Tauri app: commands, browser bridge, tray, notifications, Windows integration
src/                  React UI (services/api.ts is the only IPC boundary)
browser-extension/    MV3 extension (TypeScript, esbuild); keys/ holds the signing key that fixes its ID
scripts/installer/    WiX source, native host manifest, extension install guide, build-msi.sh
docs/                 architecture, security audit, testing
```

`browser-extension/keys/extension-key.pem` determines the extension ID that the native host trusts.
Keep it private and don't commit it (it's in `.gitignore`); losing it means a new ID.

## Privacy, terms, license

- [Privacy policy](PRIVACY.md) — Gale keeps everything on your PC and sends nothing to the developer.
- [Terms of use](TERMS.md)
- [MIT License](LICENSE) © 2026 Muhammad Faizan. Third-party components: [THIRD-PARTY-NOTICES.txt](scripts/installer/THIRD-PARTY-NOTICES.txt).
- Store listings and submission notes: [docs/store](docs/store/README.md).
- Website: [`website/`](website/) (built by `python3 scripts/website/build.py`, published to GitHub Pages by `.github/workflows/website.yml`). The PayPal link is `PAYPAL_URL` in `website/assets/site.js`.
