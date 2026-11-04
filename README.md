# Gale Download Manager

A Windows 11 download manager with a Chrome/Edge extension. Rust multi-connection engine, SQLite,
Tauri 2 + React 19 UI. Architecture: [`docs/architecture/ARCHITECTURE.md`](docs/architecture/ARCHITECTURE.md).

## Install (Windows 11)

1. Run `Gale_0.1.0_x64.msi`. It installs for your user only — no admin prompt.
   The build isn't code-signed yet, so SmartScreen may say "Windows protected your PC": choose **More info → Run anyway**.
2. Start menu → **Gale** → **Install browser extension** and follow the three steps
   (Chrome or Edge: Developer mode → Load unpacked → the folder shown on that page).
3. The extension popup should say **Connected** while Gale is running.

Uninstall from Settings → Apps. Your download list/history (`%APPDATA%\com.faizzyhon.gale`) and downloaded files are kept.

## Features

| Area | What's in |
|---|---|
| Engine | 1–32 connections, verified range support with single-connection fallback, work-stealing segments, If-Range/ETag protection, retry with back-off, pause/resume across restarts and crashes, SHA-256 verification, total/per-download speed limits, per-server connection cap, HTTP/HTTPS/SOCKS5 proxy with Credential Manager password |
| App | Download table with all row actions, properties with segment map, categories, search, dashboard, history, batch add, queues (limits, priority, start/pause/stop), scheduler (once/daily/weekdays/weekly, shutdown after with 60 s cancel), site grabber (robots.txt, depth, filters), notifications, settings (General, Downloads, Browser, Network, Security, Advanced), DB backup/restore |
| Windows | Tray + close-to-tray, toasts, start with Windows, single instance, Mark-of-the-Web, optional Defender scan, per-user MSI, Chrome + Edge native messaging registration, clean uninstall |
| Browser | Download interception by type/size/site with bypass & pause, "ask first" mode, context menus (link, queue, media, selected links, all links, copy URL), floating Download button on `<video>`/`<audio>` (DRM-protected media refused), popup with status/recent/diagnostics, options page |

Not included: HLS/DASH stream downloading, FTP, auto-update, languages other than English. See `docs/security/AUDIT.md` → Known limits.

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
crates/gale-core/     engine, limits, queues/scheduler, grabber, DB (migrations/), protocol, IPC, security
crates/gale-nmh/      native messaging host (gale-nmh.exe)
src-tauri/            Tauri app: commands, browser bridge, tray, notifications, Windows integration
src/                  React UI (services/api.ts is the only IPC boundary)
browser-extension/    MV3 extension (TypeScript, esbuild); keys/ holds the signing key that fixes its ID
scripts/installer/    WiX source, native host manifest, extension install guide, build-msi.sh
docs/                 architecture, security audit, testing
```

`browser-extension/keys/extension-key.pem` determines the extension ID that the native host trusts.
Keep it private and don't commit it (it's in `.gitignore`); losing it means a new ID.
