# Video downloads (HLS, DASH, YouTube and other sites)

Gale has two download pipelines:

| Pipeline | Used for | How |
|---|---|---|
| Direct HTTP engine (`engine.rs`) | Plain files: `.zip`, `.iso`, `.mp4` links… | Gale's own multi-connection range downloader |
| Media pipeline (`media.rs`) | Video pages (YouTube and ~1,800 other sites), HLS `.m3u8`, DASH `.mpd` | yt-dlp extracts and downloads, FFmpeg merges/converts, FFprobe verifies |

```
URL → extract (yt-dlp -J) → pick quality → download fragments (yt-dlp, -N parallel, retries)
    → merge / convert (FFmpeg, stream copy) → verify (FFprobe) → move into the save folder → history
```

Every media download is a normal row in the download list: it can be queued, scheduled, paused,
resumed, cancelled and appears in history. The Type column shows `Video · HLS`, `Video · DASH`,
`Video` or `Audio`; the Status column shows the stage (Reading video info → Downloading →
Merging audio + video / Converting → Verifying).

## Choosing a quality

Add Video (toolbar), the "looks like a video page" link in Add URL, or the browser extension's
**Download in Gale** button opens the video dialog. It lists every resolution the site offers
(e.g. 1080p, 720p, 360p) plus:

- **Audio only (MP3)** — converted with LAME at VBR quality 0.
- **Audio only (original)** — the site's own AAC/Opus track, no re-encoding.

Video is saved as MP4 when the codecs allow it (H.264 is preferred at each resolution so no
re-encoding is needed); otherwise MKV. Separate video and audio tracks are merged with stream copy.

## Supported / not supported

| Works | Doesn't |
|---|---|
| YouTube and other sites yt-dlp supports, public videos | DRM-protected video (Netflix, Prime, Disney+, YouTube Movies…) — Gale refuses, it never asks yt-dlp to include DRM formats |
| Unencrypted HLS and DASH, with separate or combined audio | Live streams (not yet) |
| Up to the highest resolution the site provides | Videos that need you to be signed in (members-only, age-restricted): browser cookies are never written to disk, so yt-dlp can't use them |
| Pause/resume, queue, schedule, proxy, speed limit | Playlists/channels (one video per download for now) |

The site's terms of service apply. Only download content you have the right to save.

## Where the tools are

The installer puts `yt-dlp.exe`, `ffmpeg.exe` and `ffprobe.exe` in `C:\Program Files\Gale\tools`. "Update yt-dlp" in Settings downloads the update into `%LOCALAPPDATA%\com.faizzyhon.gale\tools`, because Program Files is read-only for users; Gale uses whichever copy is newer.
Gale looks there first, then on `PATH` (useful for development on Linux/macOS).

Sites change often and yt-dlp has to keep up: **Settings → Advanced → Update yt-dlp** replaces
the bundled copy in place. If a site that used to work starts failing, update first.

FFmpeg is the LGPL build from BtbN/FFmpeg-Builds (includes the LAME MP3 encoder, no GPL parts).
See `tools\THIRD-PARTY-NOTICES.txt`.

## Safety

- yt-dlp runs with `--no-config` (no user config can inject options) and the URL always follows
  `--`, so a URL can never be read as an option. Only `http`/`https` URLs are accepted.
- Proxy credentials are passed through the child's environment, not its command line.
- Downloads happen in a per-download work folder (`.gale-media-<id>` inside the save folder);
  only a file that FFprobe has verified is moved into the save folder. Cancel kills yt-dlp and any
  FFmpeg it started (whole process tree) and deletes the work folder; pause keeps it so the
  download resumes.

## Tests

`cargo test -p gale-core --test media` generates real HLS (360/720/1080p) and DASH (separate
audio/video) streams with FFmpeg, serves them locally and checks: every resolution, merged
audio+video, MP3 and original audio, failed extraction, a failing FFmpeg, pause/resume,
cancellation (no leftover processes or files) and missing tools. `GALE_NET_TESTS=1` also extracts
a public YouTube video.
