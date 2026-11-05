# Privacy Policy — Gale Download Manager

**Effective date:** 5 November 2026
**Applies to:** the Gale Download Manager app for Windows and the Gale browser extension for Chrome and Edge
**Developer:** Muhammad Faizan — [faizzyhon.com](https://faizzyhon.com) — faizzyhon@gmail.com

## Summary

Gale works entirely on your computer. It has no accounts, no ads, no analytics and no telemetry. The developer
does not run any server for Gale and does not receive, collect, sell or share any of your data.

## What the app stores on your computer

To work, Gale keeps the following in a local database and settings file in your Windows user profile
(`%APPDATA%\com.faizzyhon.gale`):

- your download list and history: web address, file name, size, save folder, status, dates
- your settings, categories, queues, schedules and site rules
- a short log of what the browser extension sent to Gale (shown in Settings → Browser)
- in-app notifications

Diagnostic logs are written to `%LOCALAPPDATA%\com.faizzyhon.gale\logs`. They contain download ids, error codes
and error messages only, never cookies or request headers.

If you set a proxy password, it is saved in **Windows Credential Manager**, not in the database or settings file.

None of this leaves your computer unless you copy it yourself (for example with Settings → Advanced → Back up).

## What the browser extension does with your data

The extension only talks to the Gale app on the same computer, through Chrome's native messaging. It does not send
anything to the developer or to any other server.

- **Downloads you start:** the address, file name, size, type and referring page of downloads that match your
  interception rules are passed to Gale so Gale can download them.
- **Cookies:** if "Send the site's cookies with downloads" is on (it is by default), the cookies for that one
  download address are passed to Gale so downloads from sites you are signed in to work. Gale holds them in memory
  for that download only; they are never written to disk or logs. You can turn this off in the extension options.
- **Media on pages:** to show the Download button on videos, the extension notes the addresses of video and audio
  streams a page loads. This list stays in the extension's memory for that tab and is cleared when the tab closes.
- **Video qualities:** so the quality menu opens without waiting, the extension passes the address of a video page
  to Gale when the page opens on a video site (such as YouTube or Vimeo), or when you point at a streamed video on
  another site. Gale then reads the available qualities from that site, the same site your browser is already
  showing. The answer is kept in the extension's session memory for 15 minutes and cleared when the browser closes.
- **Magnet links:** when you click a magnet link, its address and the page address are passed to Gale.
- **Links you choose:** "Download all links" and "Download selected links" read the links on the current page only
  when you pick that menu item.
- **Settings:** the extension's own settings are saved with Chrome's storage. If you use Chrome Sync, Chrome syncs them
  with your Google account like any other extension setting.

## Network connections

Gale connects only to:

- the servers you download files from (including video sites, when you download a video or the extension asks for
  a video's qualities)
- for torrents: the torrent's trackers, other BitTorrent peers and the BitTorrent DHT network. Peers see your IP
  address and which torrent you are downloading, and receive pieces of it from Gale while it downloads; this is how
  BitTorrent works. Gale stops sharing when the download finishes or is paused
- your proxy server, if you set one
- GitHub, only when you click "Update yt-dlp" in Settings, to fetch the latest yt-dlp release

Gale does not check for updates by itself and does not contact the developer.

## Your control

- Remove any download from the list at any time (right-click → Remove).
- Turn off browser interception, cookies or the media button in the extension options.
- Uninstalling Gale removes the program and its browser and startup registrations. Your downloaded files and your
  list in `%APPDATA%\com.faizzyhon.gale` are kept so you do not lose them; delete that folder to remove everything.

## Children

Gale is a general-purpose utility and is not directed at children. It collects no personal information from anyone.

## Changes

If this policy changes, the new version will be published at
<https://github.com/faizzyhon/Gale/blob/main/PRIVACY.md> with a new effective date.

## Contact

Questions about privacy: faizzyhon@gmail.com, or open an issue at
<https://github.com/faizzyhon/Gale/issues>.
