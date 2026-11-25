# Gale for Android

The Android version of Gale: same look, same download engine ideas, built natively with Kotlin and Jetpack Compose.

- Files: multi-connection downloads (up to 16 connections), pause and resume, resume after the app is closed,
  Wi-Fi-only mode. A dropped connection is not a failure: the download waits for the network and continues
- Browser: a small built-in browser (Globe button) for finding files. Tapping a download link hands it to Gale with
  the site's cookies; on video sites a "Download video" button opens the quality picker. Ads and trackers are blocked
  by domain (about 76,000 domains), and pages can't open pop-ups or other apps unless you tapped something
- Videos: Instagram reels, posts and stories (sign in for stories), TikTok, X, Facebook, Vimeo and the other sites
  yt-dlp supports, with a quality picker (4K to 240p, or audio as M4A/MP3). YouTube only in the GitHub build
- Torrents: magnet links and .torrent files (from a link, shared, or opened from the phone); nothing is shared after a
  download finishes
- Share → Gale from any app, or tap Add and paste a link. Shared web pages (not files) open in the browser
- Saved to Movies/Gale, Music/Gale, Pictures/Gale or Download/Gale, where the gallery and file manager see them

## Two builds

| | `full` (APK on GitHub) | `play` (Google Play) |
|---|---|---|
| YouTube downloads | yes | no (Google Play policy) |
| yt-dlp updates | in the app, every 3 days and on demand | only with app updates (Play does not allow apps to download code) |
| Everything else | same | same |

## Build

Requirements: JDK 17+, Android SDK with platform 36.

```
cd android
./gradlew assembleFullRelease      # app/build/outputs/apk/full/release/app-full-<abi>-release.apk
./gradlew bundlePlayRelease        # app/build/outputs/bundle/playRelease/app-play-release.aab (upload to Play)
./gradlew testFullDebugUnitTest    # engine, torrent, link and ad-block tests
./gradlew recordRoborazziFullDebug # UI screenshots in app/build/outputs/roborazzi
```

Release signing reads `android/keystore.properties` (storeFile, storePassword, keyAlias, keyPassword); without it the
release builds are unsigned. Keep the keystore safe: Android only installs updates signed with the same key.

One APK is built per processor type because the bundled Python, FFmpeg and libtorrent are large. Most phones need
`arm64-v8a`; older 32-bit phones need `armeabi-v7a`. Google Play picks the right part of the .aab by itself.

## Libraries and data

[youtubedl-android](https://github.com/JunkFood02/youtubedl-android) (yt-dlp, Python, FFmpeg; GPL-3.0),
[libtorrent4j](https://github.com/aldenml/libtorrent4j) (MIT), OkHttp (Apache-2.0), AndroidX/Compose (Apache-2.0).
Ad and tracker list: [StevenBlack/hosts](https://github.com/StevenBlack/hosts) (MIT), in
`app/src/main/assets/adblock-hosts.txt`.
