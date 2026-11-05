//! Multi-connection HTTP(S) download engine.
//!
//! Correctness rules (see docs/architecture/ARCHITECTURE.md §4):
//! * ranges are trusted only after a verified `206` + `Content-Range` for `bytes=0-0`;
//! * every ranged response must start exactly where requested and report the probed total,
//!   and is sent with `If-Range`, so bytes from a changed file are never mixed in;
//! * bytes are *reserved* under the lock before being written, so work-stealing splits never overlap;
//! * a file is only reported complete after its size (and optional SHA-256) is verified and it is renamed.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::limits::{HostLimiter, RateLimiter};
use crate::security::{filename_from_content_disposition, filename_from_url, sanitize_filename};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Connections per download, 1..=32.
    pub connections: u8,
    /// Never split work into pieces smaller than this.
    pub min_segment: u64,
    /// Attempts per segment after the first, for transient failures.
    pub max_retries: u32,
    pub retry_base: Duration,
    pub user_agent: String,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub progress_interval: Duration,
    /// Hash the finished file even when no expected checksum is given.
    pub compute_sha256: bool,
    pub proxy: Option<ProxyConfig>,
    /// Per-download cap in bytes/s; 0 = unlimited.
    pub download_limit_bps: u64,
    pub limits: Limits,
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// `http://host:port`, `https://host:port` or `socks5://host:port`.
    pub url: String,
    pub username: Option<String>,
    /// Comes from Windows Credential Manager at runtime; never persisted by the engine.
    pub password: Option<String>,
}

/// Limits shared by every download (global bandwidth, per-host connections) plus this download's own cap.
#[derive(Clone, Default)]
pub struct Limits {
    pub global: Option<Arc<RateLimiter>>,
    pub hosts: Option<Arc<HostLimiter>>,
    local: Option<Arc<RateLimiter>>,
}

impl Limits {
    pub fn new(global: Option<Arc<RateLimiter>>, hosts: Option<Arc<HostLimiter>>) -> Self {
        Self { global, hosts, local: None }
    }
}

impl std::fmt::Debug for Limits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Limits").field("global_bps", &self.global.as_ref().map(|l| l.rate())).finish()
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            connections: 8,
            min_segment: 1024 * 1024,
            max_retries: 6,
            retry_base: Duration::from_millis(500),
            user_agent: format!("Gale/{} (Windows NT 10.0; Win64; x64)", env!("CARGO_PKG_VERSION")),
            connect_timeout: Duration::from_secs(15),
            read_timeout: Duration::from_secs(30),
            progress_interval: Duration::from_millis(250),
            compute_sha256: false,
            proxy: None,
            download_limit_bps: 0,
            limits: Limits::default(),
        }
    }
}

pub fn build_client(cfg: &EngineConfig) -> Result<Client> {
    // TLS verification stays on: there is deliberately no option to disable it.
    let mut b = Client::builder();
    if let Some(p) = &cfg.proxy {
        let mut proxy = reqwest::Proxy::all(&p.url).map_err(|_| Error::InvalidState(format!("invalid proxy address {}", p.url)))?;
        if let Some(u) = &p.username {
            proxy = proxy.basic_auth(u, p.password.as_deref().unwrap_or(""));
        }
        b = b.proxy(proxy);
    }
    Ok(b
        .user_agent(&cfg.user_agent)
        .connect_timeout(cfg.connect_timeout)
        .read_timeout(cfg.read_timeout)
        .pool_max_idle_per_host(32)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?)
}

/// Extra request headers (cookies, auth, referrer) – held in memory only, never persisted or logged.
#[derive(Debug, Clone, Default)]
pub struct RequestHeaders(pub Vec<(String, String)>);

impl RequestHeaders {
    pub(crate) fn to_map(&self) -> HeaderMap {
        let mut m = HeaderMap::new();
        for (k, v) in &self.0 {
            if let (Ok(k), Ok(v)) = (HeaderName::try_from(k.as_str()), HeaderValue::try_from(v.as_str())) {
                m.insert(k, v);
            }
        }
        // Compressed responses make byte ranges meaningless.
        m.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("identity"));
        m
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub final_url: Url,
    pub total: Option<u64>,
    pub ranges: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub file_name: String,
    pub mime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentState {
    pub index: u32,
    pub start: u64,
    /// Exclusive end; `None` when the length is unknown (single streaming segment).
    pub end: Option<u64>,
    pub written: u64,
}

impl SegmentState {
    pub fn is_complete(&self) -> bool {
        self.end.is_some_and(|e| self.start + self.written >= e)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tick {
    pub written: u64,
    pub total: Option<u64>,
    pub bytes_per_sec: f64,
    pub active_connections: u32,
    pub segments: Vec<SegmentState>,
    /// Media pipeline stage (extracting, downloading, merging, converting, verifying); None for direct HTTP.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<&'static str>,
}

#[derive(Debug)]
pub enum Outcome {
    Completed { path: PathBuf, bytes: u64, sha256: Option<String> },
    Paused { segments: Vec<SegmentState> },
}

pub struct Job {
    pub probe: ProbeResult,
    pub headers: RequestHeaders,
    pub temp_path: PathBuf,
    pub final_path: PathBuf,
    /// Resume state from the DB; empty for a fresh start.
    pub segments: Vec<SegmentState>,
    pub expected_sha256: Option<String>,
}

// ---------------------------------------------------------------------------------------------
// Probe

pub async fn probe(client: &Client, url: &Url, headers: &RequestHeaders, name_hint: Option<&str>) -> Result<ProbeResult> {
    let hm = headers.to_map();
    // HEAD is advisory: plenty of servers reject it, so failures fall through to the GET check.
    let head = client.head(url.clone()).headers(hm.clone()).send().await.ok().filter(|r| r.status().is_success());

    let get = client.get(url.clone()).headers(hm).header(header::RANGE, "bytes=0-0").send().await?;
    let status = get.status();
    if !status.is_success() {
        return Err(http_error(status, false));
    }
    let ranged_total = (status == StatusCode::PARTIAL_CONTENT)
        .then(|| content_range(get.headers()))
        .flatten()
        .filter(|cr| cr.start == 0)
        .and_then(|cr| cr.total);
    let ranges = ranged_total.is_some();
    let total = ranged_total
        .or_else(|| (status == StatusCode::OK).then(|| get.content_length()).flatten())
        .or_else(|| head.as_ref().and_then(content_length_header));

    let pick = |name: &HeaderName| -> Option<String> {
        get.headers()
            .get(name)
            .or_else(|| head.as_ref()?.headers().get(name))
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    // Weak ETags are not allowed in If-Range; fall back to Last-Modified for those.
    let etag = pick(&header::ETAG).filter(|e| !e.starts_with("W/"));
    let last_modified = pick(&header::LAST_MODIFIED);
    let mime = pick(&header::CONTENT_TYPE).map(|m| m.split(';').next().unwrap_or("").trim().to_string());
    let final_url = get.url().clone();
    let raw_name = name_hint
        .map(str::to_owned)
        .or_else(|| pick(&header::CONTENT_DISPOSITION).as_deref().and_then(filename_from_content_disposition))
        .or_else(|| filename_from_url(&final_url))
        .unwrap_or_else(|| "download".into());
    drop(get);

    Ok(ProbeResult { final_url, total, ranges, etag, last_modified, file_name: sanitize_filename(&raw_name), mime })
}

fn content_length_header(r: &Response) -> Option<u64> {
    r.headers().get(header::CONTENT_LENGTH)?.to_str().ok()?.parse().ok()
}

#[derive(Debug, PartialEq)]
struct ContentRange {
    start: u64,
    end: u64,
    total: Option<u64>,
}

fn content_range(h: &HeaderMap) -> Option<ContentRange> {
    let v = h.get(header::CONTENT_RANGE)?.to_str().ok()?;
    let rest = v.trim().strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (s, e) = range.split_once('-')?;
    Some(ContentRange {
        start: s.trim().parse().ok()?,
        end: e.trim().parse().ok()?,
        total: if total.trim() == "*" { None } else { Some(total.trim().parse().ok()?) },
    })
}

fn http_error(status: StatusCode, after_success: bool) -> Error {
    match status.as_u16() {
        401 | 403 | 404 | 410 if after_success => Error::UrlExpired { status: status.as_u16() },
        s => Error::Http { status: s },
    }
}

// ---------------------------------------------------------------------------------------------
// Planning

/// Split `total` bytes into at most `connections` equal segments of at least `min_segment` bytes.
pub fn plan_segments(total: Option<u64>, ranges: bool, connections: u8, min_segment: u64) -> Vec<SegmentState> {
    let Some(total) = total.filter(|_| ranges) else {
        return vec![SegmentState { index: 0, start: 0, end: total, written: 0 }];
    };
    if total == 0 {
        return vec![SegmentState { index: 0, start: 0, end: Some(0), written: 0 }];
    }
    let n = (connections.clamp(1, 32) as u64).min(total.div_ceil(min_segment.max(1))).max(1);
    let size = total.div_ceil(n);
    (0..n)
        .map(|i| SegmentState { index: i as u32, start: i * size, end: Some(((i + 1) * size).min(total)), written: 0 })
        .filter(|s| s.end.unwrap() > s.start)
        .collect()
}

/// Exponential back-off with ±25% jitter, capped at 30 s.
pub fn backoff(base: Duration, attempt: u32) -> Duration {
    let exp = base.saturating_mul(1u32 << attempt.min(16)).min(Duration::from_secs(30));
    let jitter = rand::random_range(0.75..1.25);
    exp.mul_f64(jitter)
}

// ---------------------------------------------------------------------------------------------
// Run

struct Slot {
    seg: SegmentState,
    /// Bytes (relative to start) handed out to the writer; always >= written.
    reserved: u64,
    claimed: bool,
}

struct Shared {
    slots: Mutex<Vec<Slot>>,
    active: AtomicU32,
    min_segment: u64,
}

impl Shared {
    fn snapshot(&self) -> Vec<SegmentState> {
        let mut v: Vec<_> = self.slots.lock().unwrap().iter().map(|s| s.seg.clone()).collect();
        v.sort_by_key(|s| s.start);
        v.iter_mut().enumerate().for_each(|(i, s)| s.index = i as u32);
        v
    }

    fn written(&self) -> u64 {
        self.slots.lock().unwrap().iter().map(|s| s.seg.written).sum()
    }

    /// Claim an unstarted segment, or steal the back half of the largest remaining one.
    fn claim(&self, allow_steal: bool) -> Option<usize> {
        let mut slots = self.slots.lock().unwrap();
        if let Some(i) = slots.iter().position(|s| !s.claimed && !s.seg.is_complete()) {
            slots[i].claimed = true;
            return Some(i);
        }
        if !allow_steal {
            return None;
        }
        let (vi, remaining) = slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Some((i, s.seg.end? - (s.seg.start + s.reserved))))
            .max_by_key(|&(_, r)| r)?;
        if remaining < 2 * self.min_segment {
            return None;
        }
        let victim = &mut slots[vi];
        let old_end = victim.seg.end.unwrap();
        let mid = old_end - remaining / 2;
        victim.seg.end = Some(mid);
        let index = slots.len() as u32;
        slots.push(Slot { seg: SegmentState { index, start: mid, end: Some(old_end), written: 0 }, reserved: 0, claimed: true });
        Some(slots.len() - 1)
    }

    fn release(&self, i: usize) {
        self.slots.lock().unwrap()[i].claimed = false;
    }
}

pub async fn run(
    client: &Client,
    cfg: &EngineConfig,
    job: Job,
    cancel: CancellationToken,
    on_tick: Arc<dyn Fn(Tick) + Send + Sync>,
) -> Result<Outcome> {
    let p = &job.probe;
    if let Some(dir) = job.temp_path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }

    // Validate the partial file: missing or truncated temp data means previous progress is unusable.
    let existing_len = tokio::fs::metadata(&job.temp_path).await.map(|m| m.len()).ok();
    // The temp file is pre-allocated to the exact total, so any other length means it was tampered with or lost.
    let resume_ok = p.ranges && !job.segments.is_empty() && p.total.is_some() && existing_len == p.total;
    let segments = if resume_ok { job.segments.clone() } else { plan_segments(p.total, p.ranges, cfg.connections, cfg.min_segment) };

    {
        let f = tokio::fs::OpenOptions::new().create(true).write(true).truncate(!resume_ok).open(&job.temp_path).await?;
        if let Some(total) = p.total {
            if f.metadata().await?.len() < total {
                f.set_len(total).await?; // pre-allocate so segments can write anywhere
            }
        }
    }

    let shared = Arc::new(Shared {
        slots: Mutex::new(segments.into_iter().map(|seg| Slot { reserved: seg.written, seg, claimed: false }).collect()),
        active: AtomicU32::new(0),
        min_segment: cfg.min_segment,
    });
    let workers_cancel = cancel.child_token();

    // Progress reporter: real bytes on disk, EMA-smoothed speed.
    let reporter = {
        let shared = shared.clone();
        let token = workers_cancel.clone();
        let interval = cfg.progress_interval;
        let total = p.total;
        let on_tick = on_tick.clone();
        tokio::spawn(async move {
            let (mut last_bytes, mut last_t, mut ema) = (shared.written(), Instant::now(), 0.0f64);
            loop {
                tokio::select! { _ = token.cancelled() => break, _ = tokio::time::sleep(interval) => {} }
                let (bytes, now) = (shared.written(), Instant::now());
                let inst = (bytes - last_bytes) as f64 / now.duration_since(last_t).as_secs_f64().max(1e-3);
                ema = if ema == 0.0 { inst } else { ema * 0.7 + inst * 0.3 };
                (last_bytes, last_t) = (bytes, now);
                on_tick(Tick {
                    written: bytes,
                    total,
                    bytes_per_sec: ema,
                    active_connections: shared.active.load(Ordering::Relaxed),
                    segments: shared.snapshot(),
                    stage: None,
                });
            }
        })
    };

    let mut cfg = cfg.clone();
    if cfg.download_limit_bps > 0 {
        cfg.limits.local = Some(RateLimiter::new(cfg.download_limit_bps));
    }
    let cfg = &cfg;
    let n_workers = if p.ranges { cfg.connections.clamp(1, 32) as usize } else { 1 };
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..n_workers {
        let (client, cfg, shared, token) = (client.clone(), cfg.clone(), shared.clone(), workers_cancel.clone());
        let (probe, headers, path) = (job.probe.clone(), job.headers.to_map(), job.temp_path.clone());
        tasks.spawn(async move { worker(&client, &cfg, &probe, &headers, &path, &shared, &token).await });
    }

    let mut first_err = None;
    while let Some(res) = tasks.join_next().await {
        let res = res.unwrap_or_else(|e| Err(Error::InvalidState(format!("worker panicked: {e}"))));
        if let Err(e) = res {
            if first_err.is_none() {
                tracing::warn!(error = %e, code = e.code(), "segment failed; stopping download");
                workers_cancel.cancel(); // one fatal segment stops the whole download
                first_err = Some(e);
            }
        }
    }
    workers_cancel.cancel();
    let _ = reporter.await;
    let segments = shared.snapshot();
    on_tick(Tick { written: shared.written(), total: p.total, bytes_per_sec: 0.0, active_connections: 0, segments: segments.clone(), stage: None });

    if let Some(e) = first_err {
        return Err(e);
    }
    if cancel.is_cancelled() {
        return Ok(Outcome::Paused { segments });
    }
    finalize(cfg, &job, &segments).await
}

async fn finalize(cfg: &EngineConfig, job: &Job, segments: &[SegmentState]) -> Result<Outcome> {
    let written: u64 = segments.iter().map(|s| s.written).sum();
    let len = tokio::fs::metadata(&job.temp_path).await?.len();
    if let Some(total) = job.probe.total {
        if written != total || !segments.iter().all(SegmentState::is_complete) {
            return Err(Error::SizeMismatch { got: written, expected: total });
        }
    }
    if len != written {
        return Err(Error::SizeMismatch { got: len, expected: written });
    }

    let sha256 = if cfg.compute_sha256 || job.expected_sha256.is_some() { Some(sha256_file(&job.temp_path).await?) } else { None };
    if let (Some(exp), Some(got)) = (&job.expected_sha256, &sha256) {
        if !exp.eq_ignore_ascii_case(got) {
            return Err(Error::ChecksumMismatch { expected: exp.clone(), got: got.clone() });
        }
    }
    // FlushFileBuffers needs write access on Windows.
    tokio::fs::OpenOptions::new().write(true).open(&job.temp_path).await?.sync_all().await?;

    let dest = if job.final_path.exists() {
        let dir = job.final_path.parent().unwrap_or(Path::new("."));
        let name = job.final_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        crate::security::unique_path(dir, &name)
    } else {
        job.final_path.clone()
    };
    rename_with_retry(&job.temp_path, &dest).await?;
    Ok(Outcome::Completed { path: dest, bytes: written, sha256 })
}

/// Antivirus scanners and the indexer briefly lock fresh files on Windows.
async fn rename_with_retry(from: &Path, to: &Path) -> Result<()> {
    let mut attempt = 0;
    loop {
        match tokio::fs::rename(from, to).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt < 5 && e.kind() == std::io::ErrorKind::PermissionDenied => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(200 * attempt)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

pub async fn sha256_file(path: &Path) -> Result<String> {
    let mut f = tokio::fs::File::open(path).await?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

async fn worker(
    client: &Client,
    cfg: &EngineConfig,
    p: &ProbeResult,
    headers: &HeaderMap,
    path: &Path,
    shared: &Shared,
    cancel: &CancellationToken,
) -> Result<()> {
    while let Some(i) = shared.claim(p.ranges) {
        let mut attempt = 0;
        loop {
            if cancel.is_cancelled() {
                shared.release(i);
                return Ok(());
            }
            match fetch_segment(client, cfg, p, headers, path, shared, i, cancel).await {
                Ok(()) => break,
                Err(e) if e.is_transient() && attempt < cfg.max_retries => {
                    let wait = backoff(cfg.retry_base, attempt);
                    tracing::debug!(segment = i, attempt, ?wait, error = %e, "retrying segment");
                    attempt += 1;
                    tokio::select! { _ = cancel.cancelled() => {}, _ = tokio::time::sleep(wait) => {} }
                }
                Err(e) => {
                    shared.release(i);
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn fetch_segment(
    client: &Client,
    cfg: &EngineConfig,
    p: &ProbeResult,
    headers: &HeaderMap,
    path: &Path,
    shared: &Shared,
    i: usize,
    cancel: &CancellationToken,
) -> Result<()> {
    let (start, pos, end) = {
        let mut slots = shared.slots.lock().unwrap();
        let s = &mut slots[i];
        if !p.ranges {
            s.seg.written = 0; // without range support every attempt restarts from byte 0
        }
        s.reserved = s.seg.written; // anything reserved but unwritten by a failed attempt is re-fetched
        (s.seg.start, s.seg.start + s.seg.written, s.seg.end)
    };
    if end.is_some_and(|e| pos >= e) {
        return Ok(());
    }

    let mut req = client.get(p.final_url.clone()).headers(headers.clone());
    if p.ranges {
        let range = match end {
            Some(e) => format!("bytes={pos}-{}", e - 1),
            None => format!("bytes={pos}-"),
        };
        req = req.header(header::RANGE, range);
        if let Some(v) = p.etag.as_ref().or(p.last_modified.as_ref()) {
            req = req.header(header::IF_RANGE, v);
        }
    }

    let host = p.final_url.host_str().unwrap_or_default().to_string();
    let _host_permit = match &cfg.limits.hosts {
        Some(h) => tokio::select! { p = h.acquire(&host) => p, _ = cancel.cancelled() => return Ok(()) },
        None => None,
    };
    shared.active.fetch_add(1, Ordering::Relaxed);
    let _guard = ActiveGuard(&shared.active);
    let resp = tokio::select! {
        r = req.send() => r?,
        _ = cancel.cancelled() => return Ok(()),
    };
    let status = resp.status();
    if p.ranges {
        match status {
            StatusCode::PARTIAL_CONTENT => {
                let cr = content_range(resp.headers()).ok_or(Error::RangeMismatch)?;
                // Shorter or longer bodies are fine (we cap writes at `end` and retry short ones);
                // a different start or total means bytes from another representation.
                if cr.start != pos || cr.total != p.total {
                    return Err(Error::RangeMismatch);
                }
            }
            // 200 to a ranged request with If-Range means the validator no longer matches.
            StatusCode::OK => return Err(Error::RemoteChanged),
            StatusCode::RANGE_NOT_SATISFIABLE => return Err(Error::RemoteChanged),
            s => return Err(http_error(s, true)),
        }
    } else if !status.is_success() {
        return Err(http_error(status, true));
    }

    let mut file = tokio::fs::OpenOptions::new().write(true).open(path).await?;
    if p.total.is_none() {
        file.set_len(0).await?; // unknown length: drop bytes from any earlier attempt
    }
    file.seek(std::io::SeekFrom::Start(pos)).await?;
    let mut stream = resp.bytes_stream();

    loop {
        let chunk = tokio::select! {
            c = stream.next() => c,
            _ = cancel.cancelled() => { file.flush().await?; return Ok(()); }
        };
        let Some(chunk) = chunk else { break };
        let chunk = chunk?;
        let take = {
            let mut slots = shared.slots.lock().unwrap();
            let s = &mut slots[i];
            let allowed = s.seg.end.map_or(u64::MAX, |e| e.saturating_sub(start + s.reserved));
            let take = (chunk.len() as u64).min(allowed);
            s.reserved += take;
            take
        };
        if take > 0 {
            file.write_all(&chunk[..take as usize]).await?;
            for l in [&cfg.limits.global, &cfg.limits.local].into_iter().flatten() {
                l.consume(take).await;
            }
            shared.slots.lock().unwrap()[i].seg.written += take;
        }
        if take < chunk.len() as u64 {
            break; // reached our (possibly shrunk by a steal) end
        }
        let s = &shared.slots.lock().unwrap()[i];
        if s.seg.is_complete() {
            break;
        }
    }
    file.flush().await?;

    let s = &shared.slots.lock().unwrap()[i];
    if s.seg.end.is_some() && !s.seg.is_complete() {
        // Connection closed early; transient so the worker retries from the new position.
        return Err(Error::Io(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "connection closed before segment end")));
    }
    Ok(())
}

struct ActiveGuard<'a>(&'a AtomicU32);
impl Drop for ActiveGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_equal_segments_covering_file() {
        let segs = plan_segments(Some(10 * 1024 * 1024 + 7), true, 8, 1024 * 1024);
        assert_eq!(segs.len(), 8);
        assert_eq!(segs[0].start, 0);
        for w in segs.windows(2) {
            assert_eq!(w[0].end, Some(w[1].start), "contiguous");
        }
        assert_eq!(segs.last().unwrap().end, Some(10 * 1024 * 1024 + 7));
    }

    #[test]
    fn small_files_use_fewer_segments() {
        assert_eq!(plan_segments(Some(1500), true, 32, 1024).len(), 2);
        assert_eq!(plan_segments(Some(10), true, 8, 1024).len(), 1);
        assert_eq!(plan_segments(Some(0), true, 8, 1024).len(), 1);
    }

    #[test]
    fn no_ranges_means_single_segment() {
        let s = plan_segments(Some(100_000_000), false, 16, 1024);
        assert_eq!(s, vec![SegmentState { index: 0, start: 0, end: Some(100_000_000), written: 0 }]);
        assert_eq!(plan_segments(None, true, 16, 1024)[0].end, None);
    }

    #[test]
    fn content_range_parsing() {
        let mut h = HeaderMap::new();
        h.insert(header::CONTENT_RANGE, HeaderValue::from_static("bytes 0-0/1234"));
        assert_eq!(content_range(&h), Some(ContentRange { start: 0, end: 0, total: Some(1234) }));
        h.insert(header::CONTENT_RANGE, HeaderValue::from_static("bytes 10-19/*"));
        assert_eq!(content_range(&h), Some(ContentRange { start: 10, end: 19, total: None }));
        h.insert(header::CONTENT_RANGE, HeaderValue::from_static("garbage"));
        assert_eq!(content_range(&h), None);
    }

    #[test]
    fn backoff_grows_and_caps() {
        let b = Duration::from_millis(500);
        assert!(backoff(b, 0) <= Duration::from_millis(625));
        assert!(backoff(b, 3) >= Duration::from_millis(3000));
        assert!(backoff(b, 40) <= Duration::from_millis(37_500));
    }

    #[test]
    fn steal_splits_without_overlap() {
        let shared = Shared {
            slots: Mutex::new(vec![Slot {
                seg: SegmentState { index: 0, start: 0, end: Some(100), written: 10 },
                reserved: 20,
                claimed: true,
            }]),
            active: AtomicU32::new(0),
            min_segment: 10,
        };
        let i = shared.claim(true).unwrap();
        let slots = shared.slots.lock().unwrap();
        assert_eq!(slots[0].seg.end, Some(60)); // 20 reserved, 80 remaining, steal back 40
        assert_eq!((slots[i].seg.start, slots[i].seg.end), (60, Some(100)));
        assert!(slots[0].reserved <= 60);
    }

    #[test]
    fn steal_refuses_tiny_remainders() {
        let shared = Shared {
            slots: Mutex::new(vec![Slot { seg: SegmentState { index: 0, start: 0, end: Some(100), written: 85 }, reserved: 85, claimed: true }]),
            active: AtomicU32::new(0),
            min_segment: 10,
        };
        assert!(shared.claim(true).is_none());
    }
}
