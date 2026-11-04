# Testing

| Suite | Command | What it covers |
|---|---|---|
| Core unit | `cargo test -p gale-core --lib` | filename/URL security, range planning, work stealing, back-off, DB, queues/recurrence, robots.txt, protocol validation, IPC auth, rate/host limiters |
| Core integration | `cargo test -p gale-core --test integration` | local axum servers that support ranges, ignore them, lie about them, drop connections, change files, stream without length, require auth; pause/resume, restart recovery, crash recovery, checksum, cancel/remove, parallel downloads + name collisions, bandwidth and per-host limits, proxy failure, queue concurrency, scheduler, grabber |
| Native host | `cargo test -p gale-nmh` | drives the real `gale-nmh` binary over stdin/stdout with Chrome framing against a test IPC server |
| App | `cargo test -p gale` | settings validation, engine config mapping |
| Extension | `cd browser-extension && pnpm test` | interception decision rules |
| Types/lint | `pnpm typecheck`, `cargo clippy --all-targets`, `cargo xwin clippy --target x86_64-pc-windows-msvc` | strict TS; Windows-only code paths |

## End-to-end runs performed for 0.1.0 (Linux build, real binaries)

- App UI: added a download through the dialog with a SHA-256; file on disk matched; history and dashboard updated.
- Browser bridge: `gale-nmh` ⇄ running app via the real wire protocol — ping, rules, silent add, duplicate refused,
  `file://` rejected, "ask first" opened a prefilled dialog; no cookie value found anywhere on disk afterwards.
- Real Chromium 156 with the built extension: fixed extension ID, popup shows "Connected", **a link click was handed
  to Gale and Chrome kept nothing**, media button injected on a `<video>`, toolbar badge counted the media.

## Needs a Windows 11 machine (not possible in CI here)

Install/uninstall the MSI; tray, close-to-tray, toasts, start with Windows, Credential Manager, Defender scan,
Mark-of-the-Web, pipe-owner check, shutdown countdown; extension in Chrome and Edge on Windows.
Checklist: `docs/testing/WINDOWS-CHECKLIST.md`.
