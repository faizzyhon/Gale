# Windows 11 acceptance checklist

Run on a clean Windows 11 user account. Tick each line.

## Install
- [ ] Double-click `Gale_0.1.0_x64.msi` → installs without an admin prompt (SmartScreen "More info → Run anyway" is expected until the build is signed).
- [ ] Start menu → Gale folder has **Gale** and **Install browser extension**; desktop has **Gale**.
- [ ] Files are in `%LOCALAPPDATA%\Programs\Gale`.
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

## Uninstall
- [ ] Settings → Apps → Gale → Uninstall → program folder, shortcuts and both registry keys are gone; the "start with Windows" entry is removed.
- [ ] Downloaded files and `%APPDATA%\com.faizzyhon.gale` (your list/history) are kept.
