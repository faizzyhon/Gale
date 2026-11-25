# Google Play submission (Android)

Upload the **play** build: `android/app/build/outputs/bundle/playRelease/app-play-release.aab`. The `full` APKs are
for GitHub only. Differences from the GitHub build: no YouTube downloads, and yt-dlp is not updated from inside the
app (Play does not allow apps to download code; yt-dlp updates arrive with app updates).

## Before the first release

- **New personal developer accounts** must run a closed test with at least 12 testers for 14 days before they can
  publish to production (Play Console → Testing → Closed testing). Organisation accounts are exempt.
- **App signing**: accept Play App Signing. `android/gale-release.jks` becomes the *upload key*; keep it and
  `keystore.properties` safe.
- Package name: `com.faizzyhon.gale` (cannot be changed later).

## Store listing

**App name** (30): `Gale Download Manager`

**Short description** (80): `Fast downloads with resume, torrents, video links and a browser that blocks ads`

**Full description**:

```
Gale is a free, open-source download manager. No ads, no account, no tracking.

Files
• Several connections per file for more speed
• Pause and resume, also after the app or the phone was closed
• A dropped connection is not a failure: Gale waits for the network and continues
• Wi-Fi only mode

Built-in browser
• Find files on any site and tap to download them with Gale
• Blocks ads and trackers (about 76,000 known ad domains)
• Stops pop-ups and pages that try to open other apps

Videos
• Share a link from Instagram, TikTok, X, Facebook, Vimeo and many more
• Pick the quality, or save just the audio as M4A or MP3

Torrents
• Magnet links and .torrent files
• Nothing is uploaded after your download finishes

Files are saved to Download/Gale, Movies/Gale, Music/Gale and Pictures/Gale, where your gallery and file manager
find them.

Only download content you own or have permission to download.

Source code: https://github.com/faizzyhon/Gale
```

**Category**: Tools. **Tags**: Download manager, Browser, Productivity.

**Graphics**: the icon and screenshots from `D:\Gale\0.5.0\android` and `D:\Gale\0.5.0\store-images` (feature graphic
1024×500 is required; phone screenshots 2–8).

## App content

| Question | Answer |
|---|---|
| Privacy policy | the same URL as the Microsoft Store listing (PRIVACY.md on the website) |
| Ads | No, the app does not contain ads |
| App access | All functionality is available without special access |
| Content rating (IARC) | Category "Utility, Productivity, Communication or other". Answer **Yes** to "Does the app allow users to access unrestricted internet content?" (built-in browser); everything else No |
| Target audience | 18 and over (a general browser makes younger audiences subject to the Families policy) |
| News app | No |
| Government app | No |
| Financial features | None |
| Health | None |

### Data safety

| Question | Answer |
|---|---|
| Does your app collect or share any of the required user data types? | **No** |
| Is all of the user data collected by your app encrypted in transit? | (not asked when nothing is collected) |
| Do you provide a way for users to request that their data is deleted? | Not applicable; Settings → Browser → Clear cookies and site data, and Instagram sign-out, remove everything stored on the phone |

Gale has no server and no analytics. Downloads, the Instagram session and browser cookies stay on the phone.

### Foreground service declaration

Play Console → App content → Foreground service permissions → **Data sync** (`FOREGROUND_SERVICE_DATA_SYNC`).

- *Describe the task*: "Downloads files the user started (a link they added, shared or tapped in the app's browser).
  The service runs only while a user-started download is running or waiting for the network to return, and shows a
  notification with progress and a Pause button."
- *Why it can't be deferred*: "The user is waiting for the file; stopping when the app leaves the screen would break
  the download."
- *Video*: record the phone screen: add a link, go to the home screen, pull down the notification showing progress.

## Policy notes

- **YouTube**: Play does not allow apps that download from YouTube. The play build refuses YouTube links in the Add
  sheet, the share sheet and the browser.
- **Downloaded code**: the play build never updates yt-dlp by itself (Device and Network Abuse policy).
- **Ad blocking**: Gale blocks ads only inside its own browser, not in other apps; this is allowed.
- **Torrents**: allowed; the description asks users to download only content they have rights to.
- **16 KB pages**: all native libraries are 16 KB aligned (required for apps targeting Android 15+).
