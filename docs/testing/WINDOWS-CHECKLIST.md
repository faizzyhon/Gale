# Windows 11 acceptance checklist

Run on a clean Windows 11 user account. Tick each line.

## Install
- [ ] Double-click `Gale_0.1.0_x64.msi` → installs without an admin prompt (SmartScreen "More info → Run anyway" is expected until the build is signed).
- [ ] Start menu → Gale folder has **Gale** and **Install browser extension**; desktop has **Gale**.
- [ ] The installer shows the administrator (UAC) prompt; files are in `C:\Program Files\Gale`.
- [ ] With an older per-user Gale installed (0.1–0.3 from `%LOCALAPPDATA%\Programs\Gale`): after installing, the old copy is gone from Settings → Apps, the download list is still there, and "start with Windows" still works after signing out and in.
- [ ] Run the same installer again while Gale is open and downloading: Gale closes, one "Gale Download Manager" entry remains in Settings → Apps, and the download resumes after reopening Gale.
- [ ] Settings → Update yt-dlp works (writes to `%LOCALAPPDATA%\com.faizzyhon.gale\tools`).
- [ ] `reg query HKCU\Software\Google\Chrome\NativeMessagingHosts\com.gale.nmh` points at `…\Gale\com.gale.nmh.json` (same under `Microsoft\Edge`).

## App
- [ ] Add URL → file downloads with 8 connections; Pause → Resume continues from where it stopped.
- [ ] Quit from the tray mid-download, start Gale again → the download is Paused and resumes.
- [ ] Close the window → Gale stays in the tray; left-click the tray icon brings it back.
- [ ] A finished download shows a Windows toast and appears under the bell.
- [ ] File Explorer → downloaded file → Properties shows "This file came from another computer…" (Mark-of-the-Web).
- [ ] Settings → Security → scan with Defender on; download the EICAR test file → toast "Threat found", Defender quarantines it.
- [ ] Settings → Network → proxy with user + password → password appears in Credential Manager → Windows Credentials as "Gale Download Manager".
- [ ] Settings → General → Start with Windows → sign out/in → Gale is in the tray.
- [ ] Queues → schedule "Start Main queue" in 2 minutes with "shut down after" → queue runs → 60 s countdown dialog → Cancel works.
- [ ] Launch Gale twice → the second launch just focuses the first window.

## Browser
- [ ] Follow "Install browser extension" in Chrome → popup says **Connected · v0.1.0**.
- [ ] Click a `.zip` link → Gale takes it; Chrome's download bar shows nothing.
- [ ] Popup → "Let next download through Chrome" → the next file downloads in Chrome.
- [ ] Right-click a link → Download with Gale; select text with several links → Download selected links → Gale shows the review dialog.
- [ ] Page with a `<video src=…mp4>` → hover → Download → file arrives in Gale.
- [ ] Repeat the popup check in Edge.

## Download windows (0.3.0)
- [ ] Click a download link in Chrome with interception set to "ask first" → the "Download File Info" popup appears on top with the right name, category and size; Start download closes it and opens a progress window.
- [ ] Progress window: % , speed and time left move smoothly; Show details grows the window and shows the speed graph and one bar per connection; Pause turns the bar amber; Resume continues.
- [ ] On completion the window shrinks to the completion card; Open and Open folder work; with Settings → "Close the progress window when done" it closes by itself after ~2 s.
- [ ] Windows "reduce animations" on → progress windows still work, without the motion.
- [ ] Dragging the custom title bar moves the window; it is not resizable or maximizable.

## Torrents and resume (0.5.0)
- [ ] Click a magnet link in Chrome → "Download File Info" → Start → the row shows "Getting torrent info", then peers and speed; Windows Firewall may ask once to allow Gale (BitTorrent listens for peers).
- [ ] Add URL → ".torrent file…" → pick a multi-file torrent → files land in their own folder.
- [ ] Pause a torrent, quit Gale, start it again → the torrent checks its files and continues.
- [ ] Start a big HTTP download, end Gale in Task Manager, start Gale → it continues by itself.
- [ ] Turn Wi-Fi off during a download until it fails, turn it on → it starts again within a minute.
- [ ] YouTube watch page: wait 5 s, hover the video, click Download → qualities show without the "Finding qualities…" wait.
- [ ] Settings → Network speed limit 1 MB/s → the tray Speed limit menu ticks "1 MB/s".

## Uninstall
- [ ] Settings → Apps → Gale → Uninstall → program folder, shortcuts and both registry keys are gone; the "start with Windows" entry is removed.
- [ ] Downloaded files and `%APPDATA%\com.faizzyhon.gale` (your list/history) are kept.
