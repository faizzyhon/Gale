# Security audit — Gale 0.1.0 (2026-11-04)

Scope: Rust core, Tauri app, native messaging host, Chrome extension, installer.

## Threat model in one paragraph

Untrusted input enters from (1) web servers (headers, filenames, bodies), (2) web pages
(through the extension), and (3) other local processes (through the IPC pipe). Gale must
never let any of these write outside the chosen folder, run a program, read local files,
or receive the user's cookies. It runs as the signed-in user, never elevated.

## Controls and where they are tested

| Risk | Control | Test |
|---|---|---|
| Path traversal / reserved names in remote filenames | `security::sanitize_filename` keeps only the last component, strips `<>:"\|?*`, controls, bidi overrides, trailing dots, prefixes `CON`/`NUL`…, caps length | `security::tests::*` |
| Disguised extensions (`invoice‮fdp.exe`) | bidi override characters replaced | `bidi_override_is_neutralised` |
| Non-http URLs (`file:`, `javascript:`) from any source | `validate_url` allow-list at every entry point | `url_validation`, `invalid_urls_duplicates_and_auth_failures`, `rejects_unknown_or_dangerous_input` |
| Corrupt files from mixed server versions | `If-Range` + per-response `Content-Range` start/total check; restart on ETag/size change | `if_range_mismatch_is_detected_by_engine`, `changed_remote_file_restarts_instead_of_splicing` |
| Incomplete file reported as complete | size check, optional SHA-256, rename only after both pass | `checksum_verification`, `finalize` |
| TLS downgrade | rustls with certificate verification; no setting to disable it | code review (`build_client`) |
| SQL injection | every query uses bound parameters (`sqlx::query(..).bind`) | code review (`grep format!` in db.rs shows only the constant `NOW` expression) |
| Secrets at rest | Cookies/auth headers live only in memory (`Manager.headers`, `AppState.pending`); proxy password in Windows Credential Manager (`keyring`) | `invalid_urls_duplicates_and_auth_failures` asserts no `Bearer` in DB; end-to-end run found the cookie value in no file under the app folder |
| Header injection via extension fields | CR/LF/NUL rejected in referrer, cookies, user agent, filename | `rejects_unknown_or_dangerous_input` |
| Websites driving the desktop app | Content scripts can only message the extension's own worker; the worker only sends five fixed commands; the host accepts only the extension ID derived from our key | `unknown_extension_is_refused`, `relays_valid_messages_and_rejects_bad_ones` |
| Arbitrary command execution via messaging | Protocol is a closed enum (`deny_unknown_fields` on payloads); no command carries a path or program | `rejects_unknown_or_dangerous_input` |
| Other local processes using the pipe | Per-user pipe, `reject_remote_clients`, `first_pipe_instance`; every message carries a 256-bit token from `%LOCALAPPDATA%` (user-only ACL), compared in constant time | `authenticated_roundtrip` |
| Pipe squatting (fake app stealing cookies) | Host checks the pipe server's process image equals its sibling `gale.exe` (`GetNamedPipeServerProcessId` + `QueryFullProcessImageNameW`) | Windows-only; compiles under `cargo xwin clippy` |
| Oversized messages | 1 MiB cap both directions, length validated before allocation | `framing_roundtrip_and_limits` |
| Downloaded malware | Mark-of-the-Web (`Zone.Identifier`, ZoneId=3) so SmartScreen/Protected View apply; optional Defender scan; user-configurable blocked extensions | manual on Windows |
| Deleting user files | Only on explicit confirmation (`remove(id, delete_file=true)` from the dialog); cancel removes only Gale's own `.gale-part` | `cancel_removes_partial_but_remove_keeps_completed_file` |
| Website grabber abuse | robots.txt honoured, same-site default, ≥200 ms between pages, ≤500 pages, depth ≤5, never signs in | `grabber_crawls_politely` |
| DRM / protected media | yt-dlp never gets `--allow-unplayable-formats`, so DRM formats are dropped; Gale also filters `has_drm` and reports DRM-only videos | `drm_only_and_single_file_pages`, `options_from_youtube_like_json` |
| Media tool invocation | `--no-config`, URL after `--`, http(s) only, fixed argument list, proxy via environment; output only moved out of the work folder after FFprobe verification; cancel kills the process tree | `tests/media.rs` |
| Privilege | MSI needs admin only to install into Program Files; the app always runs unelevated. Installer custom actions run fixed commands only (`taskkill` for gale.exe, `gale.exe --migrate` / `--uninstall-cleanup`); shutdown/Defender run fixed executables with fixed arguments | code review (`system.rs`, `startup.rs`, `gale.wxs`) |
| Logs leaking data | JSON logs record ids, codes and error text only; header values never logged | code review |

## Dependency scans

- `pnpm audit` (app and extension): **no known vulnerabilities**.
- `cargo audit`:
  - `rsa` RUSTSEC-2023-0071 — appears in `Cargo.lock` only as an optional dependency of SQLx's MySQL
    driver. `cargo tree -i rsa` is empty: it is **not compiled into Gale**.
  - "Unmaintained" notices for `zbus*`, `zvariant*`, `glib` (Linux GTK/D-Bus stack, not part of the
    Windows build) and `proc-macro-error` (build-time macro). No action needed for the Windows release.

## Known limits / follow-ups

1. Release binaries are **not code-signed** — SmartScreen will warn on first run until signed with an
   OV/EV certificate.
2. The extension is installed unpacked (Developer mode). Chrome on Windows only auto-installs
   Web Store or policy-managed extensions; publishing to the Web Store removes this step.
3. Cookies for a browser download are lost if Gale restarts before that download finishes; the
   download then fails with "link expired" if the site requires them.
