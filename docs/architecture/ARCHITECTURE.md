# Gale Download Manager — Technical Architecture

Gale is an original Windows 11 download manager (Tauri 2 + Rust + React 19) with a
Chrome MV3 companion extension. This document is the source of truth for the
architecture; deviations must be recorded in `docs/architecture/DECISIONS.md`.

## 1. Requirements analysis (condensed)

| Area | Hard requirement | Where it lives |
|---|---|---|
| Engine | Real multi-connection HTTP(S) range downloads, single-connection fallback, crash-safe resume, never mark incomplete as complete | `crates/gale-core/src/engine` |
| Persistence | SQLite, migrations, FK, indexes, transactions, all UI data from DB | `crates/gale-core/src/db`, `crates/gale-core/migrations` |
| Browser | MV3 extension, native messaging, interception per user rules, context menus, media detection (no DRM bypass) | `browser-extension/`, `src-tauri/src/native_messaging` |
| Scheduling | Multiple queues, priorities, scheduler that survives restart | `crates/gale-core/src/queue`, `.../scheduler` (Phase 6) |
| Windows | Tray, toasts, autostart, Credential Manager, NSIS/MSI installer, clean uninstall, no admin at runtime | `src-tauri/src/system` (Phase 7) |
| Security | Path traversal protection, filename sanitisation, TLS always verified, parameterised SQL, restricted IPC | `crates/gale-core/src/security` |

## 2. Process & component architecture

```
┌──────────────────────────── Gale.exe (Tauri 2, per-user, no elevation) ───────────────────────────┐
│  React 19 UI (WebView2)  ──invoke/events──►  commands/ (thin)  ──►  gale-core                   │
│                                                                    ├─ engine/   (Tokio + reqwest)│
│                                                                    ├─ db/       (SQLx SQLite WAL)│
│                                                                    ├─ security/ (paths, names)   │
│                                                                    └─ categories/ queue/ sched/  │
│  system/ (tray, toasts, autostart, Credential Manager)                                           │
│  ipc server: named pipe \\.\pipe\gale-<user-SID>  (ACL: current user only)                        │
└──────────────────────────────────────────────────────▲───────────────────────────────────────────┘
                                                       │ length-prefixed JSON (same protocol as NM)
┌───────────── gale-nmh.exe (native messaging host) ───┴──┐      stdin/stdout (Chrome NM framing)
│ validates origin ext-ID, size ≤ 1 MiB, JSON schema,      │◄──────────────────────────────────────┐
│ command allow-list; launches Gale.exe if not running     │                                       │
└──────────────────────────────────────────────────────────┘               Chrome MV3 extension ───┘
```

Key decisions:

* **`gale-core` is a plain Rust library with no Tauri dependency.** The engine, DB and
  security code compile and test on any OS in CI, and the Tauri layer stays a thin
  adapter. (Rule: UI separate from engine logic.)
* **The NM host is a separate tiny exe**, not the GUI binary. Chrome spawns a new host
  per connection; a GUI binary would be heavy and would show windows. The host only
  forwards allow-listed commands to the running app over a per-user named pipe.
* **SQLite in WAL mode** with `synchronous=NORMAL`, `foreign_keys=ON`. Segment progress
  is flushed every ~1 s and on every state transition, so a crash loses at most ~1 s of
  progress, never correctness (data already on disk beyond the persisted offset is simply
  re-downloaded).

## 3. Windows-specific limitations & how we handle them

| Limitation | Handling |
|---|---|
| WebView2 runtime must exist | Bundled via Tauri `webviewInstallMode: downloadBootstrapper` (Win 11 ships it, bootstrapper covers edge cases). |
| `MAX_PATH` (260) still bites many APIs | Use `\\?\` verbatim paths for file I/O on long paths; sanitiser caps filename at 200 UTF-16 units. |
| Reserved names (`CON`, `NUL`, `COM1`…), trailing dots/spaces, `<>:"/\|?*` | `security::sanitize_filename` rewrites them (unit-tested). |
| Files locked by AV / indexer during rename | Finalise with retrying rename (5 × 200 ms back-off) before reporting failure. |
| Sparse/pre-allocated files | `set_len` pre-allocation; NTFS zero-fills lazily so this is cheap. |
| Chrome NM manifest lookup | HKCU `Software\Google\Chrome\NativeMessagingHosts\com.gale.nmh` → per-user install, no admin. |
| Chrome cannot hand over every download (blob:, data:, POST forms, some auth flows) | Extension only intercepts http(s) GET downloads it can reproduce; everything else is left to Chrome. Documented in UI. |
| Cookies for authenticated downloads | Sent from the extension per request (`chrome.cookies` for that URL only), held in memory, never written to SQLite or logs. |
| Shutdown-after-completion | `InitiateSystemShutdownEx` via `shutdown /s /t 60` equivalent API, preceded by a cancellable 60 s toast/dialog. |
| SmartScreen on unsigned installers | Release builds must be Authenticode-signed (see risks). |

## 4. Rust download engine interfaces

```rust
// crates/gale-core/src/engine/mod.rs
pub struct EngineConfig { connections: u8 /*1..=32*/, min_segment: u64, max_retries: u32,
                          user_agent: String, connect_timeout: Duration, read_timeout: Duration }

pub struct DownloadRequest { url: Url, headers: Vec<(String,String)> /* cookies, auth – in-memory only */,
                             dest_dir: PathBuf, file_name: Option<String>, expected_sha256: Option<String> }

pub struct ProbeResult { final_url, content_length: Option<u64>, accept_ranges: bool,
                         etag, last_modified, suggested_name: String, content_type }

pub struct SegmentState { index: u32, start: u64, end: u64 /*exclusive*/, written: u64 }
pub struct ResumeState  { probe: ProbeResult, segments: Vec<SegmentState>, temp_path: PathBuf }

pub enum Progress { Started{..}, Tick{ written: u64, total: Option<u64>, bytes_per_sec: f64,
                    active_connections: u32, segments: Vec<SegmentState> }, Finished{ path }, ... }

pub async fn probe(client: &Client, req: &DownloadRequest) -> Result<ProbeResult, EngineError>;
pub async fn run(client, cfg, req, resume: Option<ResumeState>,
                 cancel: CancellationToken, progress: mpsc::Sender<Progress>)
    -> Result<Outcome /* Completed{path, sha256} | Paused(ResumeState) */, EngineError>;
```

Algorithm:

1. **Probe**: `HEAD`, then a verification `GET Range: bytes=0-0`. Ranges are trusted only if
   the server answers **206 with `Content-Range: bytes 0-0/N`** and `N` matches.
   `Accept-Ranges: bytes` alone is not trusted (many CDNs lie).
2. **Plan**: if ranged & length known → `min(connections, ceil(len / min_segment))` equal
   segments; else one streaming segment.
3. **Workers** (one Tokio task per connection): request `Range: bytes=pos-(end-1)` with
   `If-Range: <etag|last-modified>`. A segment response is accepted only if status is 206
   **and** `Content-Range` start equals the requested start **and** the total equals the
   probed length. Anything else → `EngineError::RemoteChanged`, no bytes written. This is the
   guard against mixing incompatible server responses.
4. **Adaptive allocation (work stealing)**: a worker that finishes steals the second half
   of the segment with the most remaining bytes (if ≥ 2 × `min_segment`). Bytes are
   *reserved* under a lock before they are written, so a split point is always past
   anything the victim will write — segments can never overlap.
5. **Retry**: per segment, exponential back-off `500ms × 2^n` capped at 30 s with jitter,
   up to `max_retries`; 4xx (except 408/429) are fatal; 403/410 after success → `UrlExpired`.
6. **Pause/cancel**: `CancellationToken`; workers flush, return; `Outcome::Paused` carries the
   `ResumeState` which is persisted.
7. **Finalise**: verify `sum(written) == content_length`, `fsync`, optional SHA-256, rename
   `name.gale-part` → unique final name. Only after this succeeds is the DB row set to
   `completed`.

## 5. SQLite schema (migration `0001_initial.sql`)

Tables: `download_categories`, `downloads`, `download_segments`, `download_metadata`,
`queues`, `queue_items`, `scheduler_jobs`, `browser_events`, `download_history`,
`application_settings`, `proxy_profiles`, `domain_rules`, `notifications`, plus SQLx's
`_sqlx_migrations` for schema versioning. See the migration file for exact DDL; highlights:

* `downloads.status` is a CHECK-constrained enum: `queued|probing|downloading|paused|completed|failed|cancelled|scheduled`.
* `download_segments (download_id FK ON DELETE CASCADE, idx, start_byte, end_byte, written)`.
* Secrets are never columns: `proxy_profiles.credential_ref` is a Windows Credential
  Manager target name, not a password.
* Indexes on `downloads(status)`, `downloads(category_id)`, `downloads(created_at)`,
  `download_history(completed_at)`, `browser_events(created_at)`.
* Backup/restore uses `VACUUM INTO` (consistent online snapshot).

## 6. Native messaging protocol (v1)

Framing: Chrome NM (4-byte little-endian length + UTF-8 JSON), max 1 MiB inbound (Chrome's
own limit to host is 4 GiB; we cap far lower). Same framing on the named pipe.

```jsonc
// extension → host
{ "v":1, "id":"<uuid>", "cmd":"add_download",
  "payload": { "url":"https://…", "referrer":"…", "filename":"…", "fileSize":123,
               "mime":"…", "cookies":"a=b; c=d", "userAgent":"…", "queue": null, "silent": false } }
{ "v":1, "id":"…", "cmd":"add_batch",  "payload": { "items":[{ "url":"…" }], "referrer":"…" } }
{ "v":1, "id":"…", "cmd":"ping" }
{ "v":1, "id":"…", "cmd":"get_rules" }          // interception settings owned by the app
{ "v":1, "id":"…", "cmd":"recent", "payload":{ "limit":10 } }
{ "v":1, "id":"…", "cmd":"show_app" }
// host → extension
{ "v":1, "id":"…", "ok":true,  "result":{…} }
{ "v":1, "id":"…", "ok":false, "error":{ "code":"invalid_url|not_running|rejected|internal", "message":"…" } }
```

Security: Chrome passes the calling origin (`chrome-extension://<id>/`) as argv[1]; the
host rejects anything not in `allowed_origins`. Commands outside the allow-list are
rejected; only `http`/`https`/`ftp(s)` URLs are accepted; no command can name a file path
or a program to run. The pipe is created with a DACL granting only the current user SID.

## 7. Frontend architecture

```
src/
  app/App.tsx                — providers, router (state-based, no URL routing needed)
  layouts/Shell.tsx          — sidebar + toolbar + content
  components/                — DownloadTable, AddUrlDialog, PropertiesDialog, StatusPill, ProgressBar…
  pages/                     — Downloads (filtered views), Dashboard, History, Settings/*
  stores/downloads.ts        — Zustand store, hydrated from `list_downloads`, patched by `download://progress` events
  services/api.ts            — the ONLY place that calls `invoke` / `listen`
  types/                     — mirrors Rust serde types
```

Progress events are throttled in Rust (≤ 4 Hz per download) and applied as patches, so a
table of thousands of rows stays smooth.

## 8. Roadmap

| Phase | Scope | Exit criteria |
|---|---|---|
| 1 | Workspace, Tauri shell, navigation, DB + migrations, models | `cargo test`, `tsc`, `vite build`, `cargo check` on app crate |
| 2 | Engine: probe, multi-conn, fallback, stealing, pause/resume, retry, finalise | Integration tests vs local range / no-range / flaky / changed-ETag servers |
| 3 | Real download table, actions, properties, categories, search, history, settings | All row actions wired to commands; settings persist |
| 4 | MV3 extension, NM host, interception rules, context menus, selected links | Extension loads; NM round-trip test |
| 5 | Media detection (direct media + unencrypted HLS/DASH manifests), floating button | Test pages detected |
| 6 | Queues, scheduler, site grabber (robots.txt, depth, rate limit), proxy/bandwidth | Schedules survive restart |
| 7 | Tray, toasts, autostart, Credential Manager, NSIS + MSI, updater | Install/uninstall on Win 11 |
| 8 | Security audit, `cargo audit`, `npm audit`, full test pass, release build | Acceptance list in the brief |

## 9. Risks & dependencies

* **Code signing** — unsigned MSI/NSIS triggers SmartScreen; needs an OV/EV certificate.
* **Chrome Web Store review** — `downloads` + `nativeMessaging` + `<all_urls>` get scrutiny; ship
  as unpacked/enterprise-policy install first.
* **Interception races** — `chrome.downloads.onDeterminingFilename` lets us cancel before data is
  written to disk, but Chrome may already have fetched bytes; duplicate transfer is accepted cost.
* **Servers lying about ranges** — mitigated by 0-0 verification and per-response Content-Range checks.
* **Windows-only code is not exercised on Linux CI** — Phase 7/8 require a Windows 11 runner
  (GitHub Actions `windows-2025`).
* Pinned stack: Tauri 2.x, reqwest 0.12 (rustls), SQLx 0.8 (sqlite), Tokio 1.x, React 19, Vite, Tailwind 4.
