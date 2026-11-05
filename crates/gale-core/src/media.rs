//! Adaptive-streaming / site-video pipeline, separate from the direct HTTP engine.
//!
//! URL → extract (yt-dlp -J) → choose quality → download (yt-dlp, HLS/DASH fragments, retries)
//!     → merge / convert (FFmpeg, stream copy where possible) → verify (FFprobe).
//!
//! Gale never handles stream signatures or encryption itself: extraction is delegated to yt-dlp,
//! which drops DRM-protected formats unless told otherwise (Gale never tells it otherwise).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::{Error, Result};

const EXE: &str = if cfg!(windows) { ".exe" } else { "" };

// ---------------------------------------------------------------------------------------------
// Tools

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTools {
    pub ytdlp: PathBuf,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl MediaTools {
    /// Bundled `tools/` next to the app first (the installer puts them there), then PATH.
    /// A yt-dlp the user updated (in the writable `user_dir`) is used instead when it is the newer file.
    pub fn locate(bundle_dir: Option<&Path>, user_dir: Option<&Path>) -> Result<Self> {
        let find = |name: &str| -> Result<PathBuf> {
            let file = format!("{name}{EXE}");
            bundle_dir
                .map(|d| d.join("tools").join(&file))
                .filter(|p| p.is_file())
                .or_else(|| which(&file))
                .ok_or_else(|| Error::ToolMissing(name.into()))
        };
        let user_ytdlp = user_dir.map(|d| d.join(format!("yt-dlp{EXE}"))).filter(|p| p.is_file());
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let ytdlp = match (user_ytdlp, find("yt-dlp")) {
            (Some(u), Ok(b)) if modified(&u) >= modified(&b) => u,
            (Some(_), Ok(b)) => b, // a newer Gale shipped a newer yt-dlp than the user's last update
            (Some(u), Err(_)) => u,
            (None, r) => r?,
        };
        Ok(Self { ytdlp, ffmpeg: find("ffmpeg")?, ffprobe: find("ffprobe")? })
    }

    /// Versions of the three tools, for Settings and diagnostics.
    pub async fn versions(&self) -> Result<(String, String)> {
        let yt = run_capture(&self.ytdlp, &["--version"], Duration::from_secs(30)).await?;
        let ff = run_capture(&self.ffmpeg, &["-hide_banner", "-version"], Duration::from_secs(30)).await?;
        Ok((yt.trim().to_string(), ff.lines().next().unwrap_or_default().to_string()))
    }

    /// yt-dlp needs frequent updates as sites change. The installed copy is read-only for users (Program Files),
    /// so `-U` runs on a copy in the writable `user_dir`, which `locate` then prefers. Returns output and new path.
    pub async fn update_ytdlp(&self, user_dir: &Path) -> Result<(String, PathBuf)> {
        let target = user_dir.join(format!("yt-dlp{EXE}"));
        if self.ytdlp != target {
            tokio::fs::create_dir_all(user_dir).await?;
            tokio::fs::copy(&self.ytdlp, &target).await?;
        }
        let out = run_capture(&target, &["--no-config", "-U"], Duration::from_secs(180)).await?;
        Ok((out, target))
    }
}

fn which(file: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(file)).find(|p| p.is_file())
}

fn command(exe: &Path) -> Command {
    let mut c = Command::new(exe);
    c.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    #[cfg(unix)]
    c.process_group(0); // own group, so cancel can stop ffmpeg children too
    c
}

async fn run_capture(exe: &Path, args: &[&str], timeout: Duration) -> Result<String> {
    let out = tokio::time::timeout(timeout, command(exe).args(args).output())
        .await
        .map_err(|_| Error::Media(format!("{} timed out", exe.display())))??;
    if !out.status.success() {
        return Err(Error::Media(last_error(&String::from_utf8_lossy(&out.stderr))));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// yt-dlp's own message, without its "ERROR: [extractor] id:" prefix noise.
fn last_error(stderr: &str) -> String {
    stderr
        .lines()
        .rev()
        .find(|l| l.starts_with("ERROR:"))
        .map(|l| l.trim_start_matches("ERROR:").trim().to_string())
        .unwrap_or_else(|| stderr.lines().last().unwrap_or("media tool failed").trim().to_string())
}

/// Stops yt-dlp and any FFmpeg it started.
fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let sys = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let _ = std::process::Command::new(Path::new(&sys).join(r"System32\taskkill.exe"))
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(0x0800_0000)
            .status();
    }
    #[cfg(unix)]
    // SAFETY: plain syscall; negative pid targets the process group created with process_group(0).
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

// ---------------------------------------------------------------------------------------------
// Extraction

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaFormat {
    pub format_id: String,
    pub ext: String,
    pub protocol: String,
    pub has_video: bool,
    pub has_audio: bool,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub size: Option<u64>,
    pub tbr: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MediaChoice {
    /// Best video up to this height (None = best available) with its best audio, as MP4 when possible.
    Video {
        #[serde(rename = "maxHeight")]
        max_height: Option<u32>,
    },
    /// Audio only: MP3 conversion, or the original codec (M4A/Opus) with no quality loss.
    Audio { mp3: bool },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityOption {
    pub label: String,
    pub choice: MediaChoice,
    /// `hls`, `dash`, `video` (single file) or `audio` — stored as the download's kind.
    pub kind: String,
    pub estimated_size: Option<u64>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub id: String,
    pub title: String,
    pub extractor: String,
    pub webpage_url: String,
    pub duration: Option<f64>,
    pub is_live: bool,
    pub formats: Vec<MediaFormat>,
    pub options: Vec<QualityOption>,
    /// Formats existed but every one was DRM-protected.
    pub drm_only: bool,
}

#[derive(Deserialize)]
struct RawInfo {
    id: Option<String>,
    title: Option<String>,
    extractor: Option<String>,
    webpage_url: Option<String>,
    duration: Option<f64>,
    is_live: Option<bool>,
    #[serde(default)]
    formats: Vec<RawFormat>,
    // Single-format extractors (e.g. a plain .mp4) put the format at the top level.
    format_id: Option<String>,
    ext: Option<String>,
    protocol: Option<String>,
    vcodec: Option<String>,
    acodec: Option<String>,
    height: Option<u32>,
}

#[derive(Deserialize, Clone)]
struct RawFormat {
    format_id: String,
    ext: Option<String>,
    protocol: Option<String>,
    vcodec: Option<String>,
    acodec: Option<String>,
    height: Option<u32>,
    fps: Option<f64>,
    filesize: Option<u64>,
    filesize_approx: Option<u64>,
    tbr: Option<f64>,
    #[serde(default)]
    has_drm: Option<serde_json::Value>,
}

fn present(c: &Option<String>) -> Option<String> {
    c.clone().filter(|v| v != "none")
}

pub fn parse_info(json: &str) -> Result<MediaInfo> {
    let raw: RawInfo = serde_json::from_str(json).map_err(|e| Error::Media(format!("unexpected extractor output: {e}")))?;
    let mut raw_formats = raw.formats.clone();
    if raw_formats.is_empty() {
        if let Some(id) = &raw.format_id {
            raw_formats.push(RawFormat {
                format_id: id.clone(),
                ext: raw.ext.clone(),
                protocol: raw.protocol.clone(),
                vcodec: raw.vcodec.clone(),
                acodec: raw.acodec.clone(),
                height: raw.height,
                fps: None,
                filesize: None,
                filesize_approx: None,
                tbr: None,
                has_drm: None,
            });
        }
    }
    let total = raw_formats.len();
    let formats: Vec<MediaFormat> = raw_formats
        .into_iter()
        .filter(|f| !matches!(f.has_drm, Some(serde_json::Value::Bool(true))))
        .filter(|f| !matches!(f.protocol.as_deref(), Some("mhtml"))) // storyboard images
        .map(|f| {
            let (v, a) = (present(&f.vcodec), present(&f.acodec));
            // "none" = track absent; missing = not reported (HLS variants, plain files): assume present
            // unless the other track is known, so muxed streams aren't mistaken for audio-only.
            let has_video = v.is_some() || (f.vcodec.is_none() && (f.height.is_some() || a.is_none()));
            let has_audio = a.is_some() || (f.acodec.is_none() && v.is_none());
            MediaFormat {
                format_id: f.format_id,
                ext: f.ext.unwrap_or_default(),
                protocol: f.protocol.unwrap_or_else(|| "https".into()),
                has_video,
                has_audio,
                height: f.height,
                fps: f.fps,
                vcodec: v,
                acodec: a,
                size: f.filesize.or(f.filesize_approx),
                tbr: f.tbr,
            }
        })
        .collect();
    let options = quality_options(&formats);
    Ok(MediaInfo {
        id: raw.id.unwrap_or_default(),
        title: raw.title.unwrap_or_else(|| "video".into()),
        extractor: raw.extractor.unwrap_or_default(),
        webpage_url: raw.webpage_url.unwrap_or_default(),
        duration: raw.duration,
        is_live: raw.is_live.unwrap_or(false),
        drm_only: total > 0 && formats.is_empty(),
        formats,
        options,
    })
}

fn kind_of(protocol: &str) -> &'static str {
    if protocol.starts_with("m3u8") {
        "hls"
    } else if protocol.contains("dash") {
        "dash"
    } else {
        "video"
    }
}

/// What the user can pick: one entry per available height, plus audio-only options.
pub fn quality_options(formats: &[MediaFormat]) -> Vec<QualityOption> {
    let best_audio = formats
        .iter()
        .filter(|f| f.has_audio && !f.has_video)
        .max_by(|a, b| a.tbr.unwrap_or(0.0).total_cmp(&b.tbr.unwrap_or(0.0)));
    let mut heights: Vec<u32> = formats.iter().filter(|f| f.has_video).filter_map(|f| f.height).collect();
    heights.sort_unstable_by(|a, b| b.cmp(a));
    heights.dedup();

    let mut out = Vec::new();
    for h in heights {
        // Prefer a format that will go into MP4 without re-encoding (H.264), then the highest bitrate.
        let v = formats
            .iter()
            .filter(|f| f.has_video && f.height == Some(h))
            .max_by(|a, b| {
                let avc = |f: &MediaFormat| f.vcodec.as_deref().is_some_and(|c| c.starts_with("avc"));
                (avc(a), a.tbr.unwrap_or(0.0)).partial_cmp(&(avc(b), b.tbr.unwrap_or(0.0))).unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("height came from these formats");
        let separate = !v.has_audio && best_audio.is_some();
        let size = match (v.size, separate.then_some(best_audio).flatten()) {
            (Some(vs), Some(a)) => a.size.map(|s| s + vs),
            (vs, _) => vs,
        };
        let kind = kind_of(&v.protocol);
        out.push(QualityOption {
            label: format!("{h}p"),
            choice: MediaChoice::Video { max_height: Some(h) },
            kind: kind.into(),
            estimated_size: size,
            detail: format!(
                "{}{}",
                match kind {
                    "hls" => "HLS stream",
                    "dash" => "DASH stream",
                    _ => "Single file",
                },
                if separate { " · video + audio merged" } else { "" }
            ),
        });
    }
    if let Some(f) = formats.iter().rev().find(|f| f.has_video).filter(|_| out.is_empty()) {
        out.push(QualityOption {
            label: "Best available".into(),
            choice: MediaChoice::Video { max_height: None },
            kind: kind_of(&f.protocol).into(),
            estimated_size: f.size,
            detail: "Quality not reported by the site".into(),
        });
    }
    if formats.iter().any(|f| f.has_audio) {
        let size = best_audio.and_then(|a| a.size);
        out.push(QualityOption { label: "Audio only (MP3)".into(), choice: MediaChoice::Audio { mp3: true }, kind: "audio".into(), estimated_size: size, detail: "Converted to MP3".into() });
        out.push(QualityOption {
            label: "Audio only (original)".into(),
            choice: MediaChoice::Audio { mp3: false },
            kind: "audio".into(),
            estimated_size: size,
            detail: "M4A/Opus as published, no re-encoding".into(),
        });
    }
    out
}

/// Network settings applied to yt-dlp (it doesn't share the HTTP engine's client).
#[derive(Debug, Clone, Default)]
pub struct MediaNet {
    /// Full proxy URL incl. credentials; passed via environment, never on the command line.
    pub proxy: Option<String>,
    pub timeout_secs: u64,
    pub rate_limit_bps: u64,
}

impl MediaNet {
    fn apply(&self, c: &mut Command) {
        if let Some(p) = &self.proxy {
            for k in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"] {
                c.env(k, p);
            }
        }
    }
}

pub async fn extract(tools: &MediaTools, url: &url::Url, net: &MediaNet) -> Result<MediaInfo> {
    let mut c = command(&tools.ytdlp);
    net.apply(&mut c);
    c.args(["--no-config", "--no-playlist", "--no-warnings", "-J", "--socket-timeout"])
        .arg(net.timeout_secs.max(5).to_string())
        .arg("--")
        .arg(url.as_str()); // after "--": a URL can never be read as an option
    let out = tokio::time::timeout(Duration::from_secs(120), c.output())
        .await
        .map_err(|_| Error::Media("the site took too long to answer".into()))??;
    if !out.status.success() {
        return Err(Error::Media(last_error(&String::from_utf8_lossy(&out.stderr))));
    }
    let info = parse_info(&String::from_utf8_lossy(&out.stdout))?;
    if info.drm_only {
        return Err(Error::Media("this video is DRM-protected and can't be downloaded".into()));
    }
    if info.is_live {
        return Err(Error::Media("live streams aren't supported yet".into()));
    }
    if info.options.is_empty() {
        return Err(Error::Media("no downloadable formats were found".into()));
    }
    Ok(info)
}

// ---------------------------------------------------------------------------------------------
// Download

pub struct MediaJob {
    pub url: url::Url,
    pub choice: MediaChoice,
    pub save_dir: PathBuf,
    /// Per-download working folder for fragments and .part files (kept on pause, removed on cancel).
    pub work_dir: PathBuf,
    pub concurrent_fragments: u8,
    pub expected_bytes: Option<u64>,
    pub net: MediaNet,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaProgress {
    pub stage: &'static str,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub bytes_per_sec: f64,
}

#[derive(Debug)]
pub enum MediaOutcome {
    Completed { path: PathBuf, bytes: u64 },
    Paused,
}

pub fn selector_args(choice: MediaChoice) -> Vec<String> {
    let s = |v: &str| v.to_string();
    match choice {
        MediaChoice::Video { max_height } => {
            let f = match max_height {
                Some(h) => format!("bv*[height<={h}]+ba/b[height<={h}]"),
                None => s("bv*+ba/b"),
            };
            // Prefer H.264/AAC at a given resolution so MP4 needs no re-encode; copy streams otherwise.
            vec![s("-f"), f, s("-S"), s("vcodec:h264,acodec:m4a"), s("--merge-output-format"), s("mp4/mkv"), s("--remux-video"), s("mp4/mkv")]
        }
        MediaChoice::Audio { mp3: true } => vec![s("-f"), s("ba/b"), s("-x"), s("--audio-format"), s("mp3"), s("--audio-quality"), s("0")],
        MediaChoice::Audio { mp3: false } => vec![s("-f"), s("ba/b"), s("-x")],
    }
}

const DL: &str = "GALEDL";
const PP: &str = "GALEPP";
const FILE: &str = "GALEFILE";

#[derive(Debug, PartialEq)]
enum Line {
    Download { format: String, downloaded: u64, total: Option<u64>, speed: f64 },
    Post(String),
    File(PathBuf),
    Other,
}

fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0)
}

fn parse_line(l: &str) -> Line {
    let l = l.trim_end_matches(['\r', '\n']);
    if let Some(rest) = l.find(DL).map(|i| &l[i + DL.len() + 1..]) {
        let p: Vec<&str> = rest.split('|').collect();
        if p.len() >= 5 {
            return Line::Download {
                format: p[0].to_string(),
                downloaded: num(p[1]).unwrap_or(0.0) as u64,
                total: num(p[2]).or_else(|| num(p[3])).map(|v| v as u64),
                speed: num(p[4]).unwrap_or(0.0),
            };
        }
    }
    if let Some(rest) = l.find(PP).map(|i| &l[i + PP.len() + 1..]) {
        return Line::Post(rest.split('|').next().unwrap_or_default().to_string());
    }
    if let Some(rest) = l.strip_prefix(FILE).map(str::trim) {
        return Line::File(PathBuf::from(rest));
    }
    Line::Other
}

fn stage_for(postprocessor: &str) -> &'static str {
    match postprocessor {
        "Merger" => "merging",
        p if p.contains("ExtractAudio") || p.contains("Convertor") || p.contains("Remuxer") => "converting",
        _ => "processing",
    }
}

pub async fn download(
    tools: &MediaTools,
    job: &MediaJob,
    cancel: &CancellationToken,
    on_progress: &(dyn Fn(MediaProgress) + Send + Sync),
) -> Result<MediaOutcome> {
    tokio::fs::create_dir_all(&job.save_dir).await?;
    tokio::fs::create_dir_all(&job.work_dir).await?;
    let mut c = command(&tools.ytdlp);
    job.net.apply(&mut c);
    c.args(["--no-config", "--no-playlist", "--no-warnings", "--newline", "--progress", "--no-simulate", "--continue", "--no-mtime", "--windows-filenames"])
        .args(["--retries", "10", "--fragment-retries", "10", "--socket-timeout"])
        .arg(job.net.timeout_secs.max(5).to_string())
        .arg("-N")
        .arg(job.concurrent_fragments.clamp(1, 16).to_string())
        .arg("--ffmpeg-location")
        .arg(&tools.ffmpeg)
        // Everything happens in the work folder; only the verified result is moved to save_dir.
        .arg("-P")
        .arg(format!("home:{}", job.work_dir.display()))
        .args(["-o", "%(title).150B [%(id)s].%(ext)s"])
        .args(["--progress-template", &format!("download:{DL}|%(info.format_id)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s")])
        .args(["--progress-template", &format!("postprocess:{PP}|%(progress.postprocessor)s|%(progress.status)s")])
        .args(["--print", &format!("after_move:{FILE} %(filepath)s")]);
    if job.net.rate_limit_bps > 0 {
        c.arg("--limit-rate").arg(job.net.rate_limit_bps.to_string());
    }
    c.args(selector_args(job.choice)).arg("--").arg(job.url.as_str());
    c.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = c.spawn().map_err(|e| Error::ToolMissing(format!("yt-dlp ({e})")))?;
    let pid = child.id().unwrap_or(0);
    let mut out = BufReader::new(child.stdout.take().expect("piped")).lines();
    let mut err = BufReader::new(child.stderr.take().expect("piped")).lines();

    on_progress(MediaProgress { stage: "extracting", downloaded: 0, total: job.expected_bytes, bytes_per_sec: 0.0 });
    let mut per_format: HashMap<String, (u64, Option<u64>)> = HashMap::new();
    let mut file: Option<PathBuf> = None;
    let mut errors = String::new();
    let (mut out_done, mut err_done) = (false, false);

    while !(out_done && err_done) {
        let line = tokio::select! {
            _ = cancel.cancelled() => {
                kill_tree(pid);
                let _ = child.wait().await;
                return Ok(MediaOutcome::Paused);
            }
            l = out.next_line(), if !out_done => match l? { Some(l) => l, None => { out_done = true; continue } },
            l = err.next_line(), if !err_done => match l? {
                Some(l) => { if l.starts_with("ERROR:") { errors.push_str(&l); errors.push('\n'); } l }
                None => { err_done = true; continue }
            },
        };
        match parse_line(&line) {
            Line::Download { format, downloaded, total, speed } => {
                per_format.insert(format, (downloaded, total));
                let done: u64 = per_format.values().map(|v| v.0).sum();
                let seen_total: u64 = per_format.values().filter_map(|v| v.1).sum();
                let total = job.expected_bytes.map_or(seen_total, |e| e.max(seen_total));
                on_progress(MediaProgress { stage: "downloading", downloaded: done, total: (total > 0).then_some(total), bytes_per_sec: speed });
            }
            Line::Post(pp) => {
                let done: u64 = per_format.values().map(|v| v.0).sum();
                on_progress(MediaProgress { stage: stage_for(&pp), downloaded: done, total: Some(done), bytes_per_sec: 0.0 });
            }
            Line::File(p) => file = Some(p),
            Line::Other => {}
        }
    }
    let status = child.wait().await?;
    if !status.success() {
        return Err(Error::Media(if errors.is_empty() { format!("yt-dlp exited with {status}") } else { last_error(&errors) }));
    }
    let path = file.filter(|p| p.is_file()).ok_or_else(|| Error::Media("the media tool finished without producing a file".into()))?;

    let bytes = tokio::fs::metadata(&path).await?.len();
    on_progress(MediaProgress { stage: "verifying", downloaded: bytes, total: Some(bytes), bytes_per_sec: 0.0 });
    verify(tools, &path, job.choice).await?;
    let name = path.file_name().map(|n| crate::security::sanitize_filename(&n.to_string_lossy())).unwrap_or_else(|| "video".into());
    let dest = crate::security::unique_path(&job.save_dir, &name);
    tokio::fs::rename(&path, &dest).await?; // work folder is inside save_dir: same volume, atomic
    let _ = tokio::fs::remove_dir_all(&job.work_dir).await;
    Ok(MediaOutcome::Completed { path: dest, bytes })
}

/// Streams FFprobe finds in a file: (codec_type, codec_name).
pub async fn probe_streams(tools: &MediaTools, path: &Path) -> Result<Vec<(String, String)>> {
    let mut c = command(&tools.ffprobe);
    c.args(["-v", "error", "-show_entries", "stream=codec_type,codec_name", "-of", "csv=p=0"]).arg(path);
    let out = tokio::time::timeout(Duration::from_secs(60), c.output()).await.map_err(|_| Error::Media("ffprobe timed out".into()))??;
    if !out.status.success() {
        return Err(Error::Media(format!("the downloaded file is not valid media: {}", last_error(&String::from_utf8_lossy(&out.stderr)))));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split_once(',').map(|(n, t)| (t.trim().to_string(), n.trim().to_string())))
        .collect())
}

async fn verify(tools: &MediaTools, path: &Path, choice: MediaChoice) -> Result<()> {
    let streams = probe_streams(tools, path).await?;
    let has = |t: &str| streams.iter().any(|(ty, _)| ty == t);
    let ok = match choice {
        MediaChoice::Video { .. } => has("video"),
        MediaChoice::Audio { mp3 } => has("audio") && !has("video") && (!mp3 || streams.iter().any(|(_, n)| n == "mp3")),
    };
    if !ok {
        return Err(Error::Media(format!("the finished file has unexpected streams: {streams:?}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_tools_are_preferred() {
        let d = std::env::temp_dir().join(format!("gale-tools-{}", std::process::id()));
        std::fs::create_dir_all(d.join("tools")).unwrap();
        for t in ["yt-dlp", "ffmpeg", "ffprobe"] {
            std::fs::write(d.join("tools").join(format!("{t}{EXE}")), b"").unwrap();
        }
        let found = MediaTools::locate(Some(&d), None).unwrap();
        assert_eq!(found.ytdlp, d.join("tools").join(format!("yt-dlp{EXE}")));
        assert_eq!(found.ffprobe, d.join("tools").join(format!("ffprobe{EXE}")));
        std::fs::remove_file(d.join("tools").join(format!("ffmpeg{EXE}"))).unwrap();
        // Falls back to PATH for the missing one, or reports exactly which tool is missing.
        match MediaTools::locate(Some(&d), None) {
            Ok(t) => assert_ne!(t.ffmpeg.parent(), Some(d.join("tools").as_path())),
            Err(e) => assert!(e.to_string().starts_with("ffmpeg"), "{e}"),
        }
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn newer_user_ytdlp_wins() {
        use std::time::{Duration, SystemTime};
        let d = std::env::temp_dir().join(format!("gale-tools-user-{}", std::process::id()));
        let (bundle, user) = (d.join("tools"), d.join("user"));
        std::fs::create_dir_all(&bundle).unwrap();
        std::fs::create_dir_all(&user).unwrap();
        for t in ["yt-dlp", "ffmpeg", "ffprobe"] {
            std::fs::write(bundle.join(format!("{t}{EXE}")), b"").unwrap();
        }
        let user_yt = user.join(format!("yt-dlp{EXE}"));
        let set = |p: &Path, age: u64| {
            std::fs::File::options().write(true).open(p).unwrap().set_modified(SystemTime::now() - Duration::from_secs(age)).unwrap()
        };
        std::fs::write(&user_yt, b"").unwrap();
        set(&bundle.join(format!("yt-dlp{EXE}")), 3600);
        set(&user_yt, 60);
        assert_eq!(MediaTools::locate(Some(&d), Some(&user)).unwrap().ytdlp, user_yt);
        set(&user_yt, 7200); // Gale was updated with a newer bundled yt-dlp since
        assert_eq!(MediaTools::locate(Some(&d), Some(&user)).unwrap().ytdlp, bundle.join(format!("yt-dlp{EXE}")));
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn progress_lines() {
        assert_eq!(
            parse_line("GALEDL|137|1048576|4194304|NA|524288.5"),
            Line::Download { format: "137".into(), downloaded: 1_048_576, total: Some(4_194_304), speed: 524_288.5 }
        );
        assert_eq!(
            parse_line("[download] GALEDL|251|10|NA|200.4|NA"),
            Line::Download { format: "251".into(), downloaded: 10, total: Some(200), speed: 0.0 }
        );
        assert_eq!(parse_line("GALEPP|Merger|started"), Line::Post("Merger".into()));
        assert_eq!(parse_line("GALEFILE C:\\Users\\x\\Videos\\a [id].mp4"), Line::File(PathBuf::from("C:\\Users\\x\\Videos\\a [id].mp4")));
        assert_eq!(parse_line("[youtube] abc: Downloading webpage"), Line::Other);
        assert_eq!(stage_for("FFmpegExtractAudio"), "converting");
    }

    #[test]
    fn selectors_never_allow_drm_and_keep_quality() {
        let v = selector_args(MediaChoice::Video { max_height: Some(720) }).join(" ");
        assert!(v.contains("bv*[height<=720]+ba/b[height<=720]") && v.contains("mp4/mkv"));
        assert!(!v.contains("allow-unplayable"));
        assert!(selector_args(MediaChoice::Audio { mp3: true }).join(" ").contains("--audio-format mp3"));
        assert!(!selector_args(MediaChoice::Audio { mp3: false }).join(" ").contains("--audio-format"));
    }

    #[test]
    fn options_from_youtube_like_json() {
        let json = r#"{"id":"x","title":"T","extractor":"youtube","duration":19,"formats":[
          {"format_id":"233","ext":"mp4","protocol":"m3u8_native","vcodec":"none","acodec":null},
          {"format_id":"140","ext":"m4a","protocol":"https","vcodec":"none","acodec":"mp4a.40.2","filesize":300,"tbr":129},
          {"format_id":"251","ext":"webm","protocol":"https","vcodec":"none","acodec":"opus","filesize":250,"tbr":106},
          {"format_id":"134","ext":"mp4","protocol":"https","vcodec":"avc1.4D400C","acodec":"none","height":360,"filesize":1000,"tbr":136},
          {"format_id":"243","ext":"webm","protocol":"https","vcodec":"vp9","acodec":"none","height":360,"filesize":900,"tbr":150},
          {"format_id":"137","ext":"mp4","protocol":"http_dash_segments","vcodec":"avc1.640028","acodec":"none","height":1080,"tbr":4000},
          {"format_id":"sb0","ext":"mhtml","protocol":"mhtml","vcodec":"none","acodec":"none"},
          {"format_id":"drm1","ext":"mp4","protocol":"https","vcodec":"avc1","acodec":"none","height":2160,"has_drm":true}]}"#;
        let info = parse_info(json).unwrap();
        assert!(!info.drm_only);
        let labels: Vec<_> = info.options.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(labels, ["1080p", "360p", "Audio only (MP3)", "Audio only (original)"], "2160p DRM-only height excluded");
        assert_eq!(info.options[0].kind, "dash");
        assert_eq!(info.options[1].kind, "video");
        assert_eq!(info.options[1].estimated_size, Some(1300), "H.264 360p + best audio");
        assert!(info.options[1].detail.contains("merged"));
    }

    #[test]
    fn drm_only_and_single_file_pages() {
        let drm = parse_info(r#"{"id":"d","title":"D","formats":[{"format_id":"a","vcodec":"avc1","acodec":"mp4a","height":720,"has_drm":true}]}"#).unwrap();
        assert!(drm.drm_only && drm.options.is_empty());
        let single = parse_info(r#"{"id":"s","title":"S","format_id":"0","ext":"mp4","protocol":"https"}"#).unwrap();
        assert!(single.options.iter().any(|o| o.choice == MediaChoice::Video { max_height: None }));
    }
}
