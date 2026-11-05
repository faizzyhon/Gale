//! Owns running downloads: persistence of state transitions, resume validation, concurrency limit.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{watch, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::db::{Db, Download, DownloadStatus, NewDownload};
use crate::engine::{self, EngineConfig, Job, Outcome, RequestHeaders, Tick};
use crate::media::{self, MediaChoice, MediaInfo, MediaJob, MediaNet, MediaOutcome, MediaTools};
use crate::security::{sanitize_filename, temp_path_for, unique_path, validate_url};
use crate::{categories, torrent, Error, Result};

/// Implemented by the UI layer (Tauri events) – the manager never knows about windows.
pub trait EventSink: Send + Sync + 'static {
    fn progress(&self, id: i64, tick: &Tick);
    fn changed(&self, download: &Download);
    /// Called once when a download reaches completed or failed.
    fn finished(&self, _download: &Download) {}
    /// A running queue ran out of work. `shutdown` = the user scheduled a shutdown after it.
    fn queue_finished(&self, _queue_id: i64, _shutdown: bool) {}
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddOptions {
    pub url: String,
    pub save_dir: String,
    pub file_name: Option<String>,
    pub connections: Option<u8>,
    pub referrer: Option<String>,
    pub expected_sha256: Option<String>,
    pub start: bool,
    pub source: Option<String>,
    /// Put into this queue instead of starting immediately.
    pub queue_id: Option<i64>,
    /// Set for videos from sites/streams: handled by the media pipeline instead of the HTTP engine.
    pub media: Option<MediaRequest>,
    /// Cookies / auth from the browser. Kept in memory for this session only.
    #[serde(skip)]
    pub headers: RequestHeaders,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaRequest {
    pub choice: MediaChoice,
    /// From the chosen QualityOption: `hls`, `dash`, `video` or `audio`.
    pub kind: String,
    pub title: Option<String>,
    pub expected_bytes: Option<u64>,
}

pub struct Manager {
    db: Db,
    cfg: Mutex<EngineConfig>,
    client: Mutex<reqwest::Client>,
    events: Arc<dyn EventSink>,
    running: Mutex<HashMap<i64, CancellationToken>>,
    headers: Mutex<HashMap<i64, RequestHeaders>>,
    slots: Arc<Semaphore>,
    pump_lock: tokio::sync::Mutex<()>,
    /// Wakes the background loop to pump queues right away (avoids start→pump→start recursion).
    pump_now: tokio::sync::Notify,
    busy_queues: Mutex<HashSet<i64>>,
    shutdown_queues: Mutex<HashSet<i64>>,
    media_tools: Mutex<Option<MediaTools>>,
    /// Started on the first torrent download, so people who never use BitTorrent never open its ports.
    torrent: tokio::sync::OnceCell<Arc<librqbit::Session>>,
    /// Retry downloads that failed because the network went away.
    auto_retry: AtomicBool,
    /// Download id -> (attempts so far, when to try again).
    retries: Mutex<HashMap<i64, (u32, Instant)>>,
}

/// Automatic retries after a network failure: 15 s, 30 s, 1 min, 2 min, 4 min.
const AUTO_RETRIES: u32 = 5;

impl Manager {
    pub fn new(db: Db, cfg: EngineConfig, max_simultaneous: usize, events: Arc<dyn EventSink>) -> Result<Arc<Self>> {
        let client = engine::build_client(&cfg)?;
        Ok(Arc::new(Self {
            db,
            cfg: Mutex::new(cfg),
            client: Mutex::new(client),
            events,
            running: Mutex::new(HashMap::new()),
            headers: Mutex::new(HashMap::new()),
            slots: Arc::new(Semaphore::new(max_simultaneous.max(1))),
            pump_lock: tokio::sync::Mutex::new(()),
            pump_now: tokio::sync::Notify::new(),
            busy_queues: Mutex::new(HashSet::new()),
            shutdown_queues: Mutex::new(HashSet::new()),
            media_tools: Mutex::new(None),
            torrent: tokio::sync::OnceCell::new(),
            auto_retry: AtomicBool::new(false),
            retries: Mutex::new(HashMap::new()),
        }))
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn set_auto_retry(&self, on: bool) {
        self.auto_retry.store(on, Ordering::Relaxed);
    }

    pub fn config(&self) -> EngineConfig {
        self.cfg.lock().unwrap().clone()
    }

    /// Apply new network settings; running transfers keep their current connections.
    pub fn reconfigure(&self, cfg: EngineConfig) -> Result<()> {
        let client = engine::build_client(&cfg)?;
        *self.client.lock().unwrap() = client;
        *self.cfg.lock().unwrap() = cfg;
        Ok(())
    }

    /// yt-dlp/FFmpeg/FFprobe for the media pipeline; None = video downloads unavailable.
    pub fn set_media_tools(&self, tools: Option<MediaTools>) {
        *self.media_tools.lock().unwrap() = tools;
    }

    pub fn media_tools(&self) -> Result<MediaTools> {
        self.media_tools.lock().unwrap().clone().ok_or_else(|| Error::ToolMissing("yt-dlp/FFmpeg".into()))
    }

    /// Proxy, timeout and speed limit from the engine settings, in the form yt-dlp takes.
    fn media_net(&self) -> MediaNet {
        let cfg = self.cfg.lock().unwrap().clone();
        let proxy = cfg.proxy.as_ref().and_then(|p| {
            let mut u = url::Url::parse(&p.url).ok()?;
            if let Some(user) = &p.username {
                u.set_username(user).ok()?;
                u.set_password(p.password.as_deref()).ok()?;
            }
            Some(u.to_string())
        });
        let global = cfg.limits.global.as_ref().map(|l| l.rate()).unwrap_or(0);
        let rate = [global, cfg.download_limit_bps].into_iter().filter(|r| *r > 0).min().unwrap_or(0);
        MediaNet { proxy, timeout_secs: cfg.read_timeout.as_secs(), rate_limit_bps: rate }
    }

    /// Metadata and quality options for a video page or stream URL.
    pub async fn extract_media(&self, url: &str) -> Result<MediaInfo> {
        let tools = self.media_tools()?;
        media::extract(&tools, &validate_url(url)?, &self.media_net()).await
    }

    /// New default for downloads added from now on.
    pub fn set_default_connections(&self, n: u8) {
        self.cfg.lock().unwrap().connections = n.clamp(1, 32);
    }

    /// "Recheck availability": probe the URL again without touching the transfer.
    pub async fn recheck(&self, id: i64) -> Result<engine::ProbeResult> {
        let d = self.db.get_download(id).await?;
        let client = self.client.lock().unwrap().clone();
        let headers = self.headers.lock().unwrap().get(&id).cloned().unwrap_or_default();
        engine::probe(&client, &validate_url(&d.url)?, &headers, None).await
    }

    pub fn is_running(&self, id: i64) -> bool {
        self.running.lock().unwrap().contains_key(&id)
    }

    pub async fn add(self: &Arc<Self>, opts: AddOptions) -> Result<i64> {
        let magnet = torrent::is_magnet(&opts.url);
        let local = torrent::local_torrent(&opts.url);
        let (url, web_url) = if magnet {
            (torrent::validate_magnet(&opts.url)?, None)
        } else if local.is_some() {
            (opts.url.trim().to_string(), None)
        } else {
            let u = validate_url(&opts.url)?;
            (u.to_string(), Some(u))
        };
        if let Some(existing) = self.db.find_unfinished_by_url(&url).await? {
            return Err(Error::InvalidState(format!("this URL is already in the list (download #{existing})")));
        }
        let save_dir = PathBuf::from(&opts.save_dir);
        if !save_dir.is_absolute() {
            return Err(Error::UnsafePath(if opts.save_dir.trim().is_empty() {
                "no download folder chosen".into()
            } else {
                format!("{} (the download folder must be a full path)", opts.save_dir)
            }));
        }
        if let Some(m) = &opts.media {
            if !matches!(m.kind.as_str(), "hls" | "dash" | "video" | "audio") {
                return Err(Error::InvalidState(format!("unknown media kind {}", m.kind)));
            }
        }
        let name = match &opts.media {
            // Provisional name for the list; yt-dlp decides the final one (title + id + container).
            Some(m) => {
                let ext = match m.choice {
                    MediaChoice::Audio { mp3: true } => "mp3",
                    MediaChoice::Audio { mp3: false } => "m4a",
                    MediaChoice::Video { .. } => "mp4",
                };
                sanitize_filename(&format!("{}.{ext}", m.title.as_deref().unwrap_or("video")))
            }
            None if magnet => sanitize_filename(&torrent::magnet_name(&url).unwrap_or_else(|| "torrent".into())),
            None if local.is_some() => sanitize_filename(&local.as_ref().unwrap().file_name().unwrap_or_default().to_string_lossy()),
            None => opts
                .file_name
                .clone()
                .or_else(|| web_url.as_ref().and_then(crate::security::filename_from_url))
                .map(|n| sanitize_filename(&n))
                .unwrap_or_else(|| "download".into()),
        };
        // A .torrent link is opened as a torrent rather than saved as a small file.
        let is_torrent = opts.media.is_none() && (magnet || torrent::is_torrent_name(&name));
        let cats = self.db.list_categories().await?;
        // Read before any await: a guard held across `.await` deadlocks the runtime.
        let connections = opts.connections.unwrap_or_else(|| self.cfg.lock().unwrap().connections) as i64;
        let id = self
            .db
            .insert_download(&NewDownload {
                url,
                referrer: opts.referrer,
                file_name: name.clone(),
                save_dir: opts.save_dir,
                connections,
                // Media goes to Video/Audio by kind; the provisional name's extension says nothing about it.
                category_id: match &opts.media {
                    Some(m) => media_category(&cats, m),
                    None => categories::categorize(&cats, &name).map(|c| c.id),
                },
                expected_sha256: opts.expected_sha256,
                source: opts.source,
                status: Some(if opts.start || opts.queue_id.is_some() { DownloadStatus::Queued } else { DownloadStatus::Paused }),
                kind: if is_torrent { Some("torrent".into()) } else { opts.media.as_ref().map(|m| m.kind.clone()) },
            })
            .await?;
        if let Some(m) = &opts.media {
            let choice = serde_json::to_string(&m.choice).map_err(|e| Error::InvalidState(e.to_string()))?;
            self.db.set_metadata(id, "media_choice", &choice).await?;
            if let Some(b) = m.expected_bytes {
                self.db.set_metadata(id, "expected_bytes", &b.to_string()).await?;
            }
        }
        if let Some(q) = opts.queue_id {
            self.db.enqueue(q, id).await?;
        }
        if !opts.headers.0.is_empty() {
            self.headers.lock().unwrap().insert(id, opts.headers);
        }
        self.notify(id).await;
        if opts.start {
            self.start(id).await?;
        } else if opts.queue_id.is_some() {
            self.pump_now.notify_one();
        }
        Ok(id)
    }

    /// Start or resume. Returns immediately; the transfer runs in the background.
    pub async fn start(self: &Arc<Self>, id: i64) -> Result<()> {
        let d = self.db.get_download(id).await?;
        if d.status == DownloadStatus::Completed {
            return Err(Error::InvalidState("download already completed; use restart".into()));
        }
        let token = CancellationToken::new();
        {
            let mut running = self.running.lock().unwrap();
            if running.contains_key(&id) {
                return Ok(());
            }
            running.insert(id, token.clone());
        }
        self.db.set_status(id, DownloadStatus::Queued, None).await?;
        self.notify(id).await;

        let this = self.clone();
        tokio::spawn(async move {
            let result = this.drive(id, token.clone()).await;
            this.running.lock().unwrap().remove(&id);
            let finish = match result {
                Ok(()) => {
                    this.retries.lock().unwrap().remove(&id);
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!(id, code = e.code(), error = %e, "download failed");
                    if e.is_transient() && this.auto_retry.load(Ordering::Relaxed) {
                        let mut r = this.retries.lock().unwrap();
                        let n = r.get(&id).map_or(0, |v| v.0) + 1;
                        if n <= AUTO_RETRIES {
                            r.insert(id, (n, Instant::now() + Duration::from_secs(15 << (n - 1))));
                        }
                    }
                    this.db.mark_failed(id, &e).await
                }
            };
            if let Err(e) = finish {
                tracing::error!(id, error = %e, "could not persist final download state");
            }
            this.notify(id).await;
            if let Ok(d) = this.db.get_download(id).await {
                if matches!(d.status, DownloadStatus::Completed | DownloadStatus::Failed) {
                    this.events.finished(&d);
                }
            }
            this.pump_now.notify_one();
        });
        Ok(())
    }

    async fn drive(self: &Arc<Self>, id: i64, token: CancellationToken) -> Result<()> {
        let _permit = tokio::select! {
            p = self.slots.clone().acquire_owned() => p.map_err(|_| Error::InvalidState("manager shut down".into()))?,
            _ = token.cancelled() => return self.after_cancel(id).await,
        };
        let d = self.db.get_download(id).await?;
        if d.kind == "torrent" {
            return self.drive_torrent(id, d, token).await;
        }
        if d.kind != "http" {
            return self.drive_media(id, d, token).await;
        }
        self.db.set_status(id, DownloadStatus::Probing, None).await?;
        self.notify(id).await;

        let client = self.client.lock().unwrap().clone();
        let mut cfg = self.cfg.lock().unwrap().clone();
        cfg.connections = d.connections.clamp(1, 32) as u8;
        let headers = self.headers.lock().unwrap().get(&id).cloned().unwrap_or_default();
        let url = validate_url(&d.url)?;
        let probe = tokio::select! {
            p = engine::probe(&client, &url, &headers, Some(&d.file_name)) => p?,
            _ = token.cancelled() => return self.after_cancel(id).await,
        };

        // Resume only if the remote file is provably the same one.
        let mut segments = self.db.load_segments(id).await?;
        let same_remote = d.total_bytes.map(|t| t as u64) == probe.total
            && d.etag.as_deref() == probe.etag.as_deref()
            && d.last_modified.as_deref() == probe.last_modified.as_deref()
            && probe.ranges;
        let (temp_path, final_path) = match (&d.temp_path, &d.final_path) {
            (Some(t), Some(f)) if same_remote && !segments.is_empty() => (PathBuf::from(t), PathBuf::from(f)),
            _ => {
                if !segments.is_empty() {
                    tracing::info!(id, "remote file changed or not resumable; restarting from zero");
                    segments.clear();
                    if let Some(t) = &d.temp_path {
                        remove_if_exists(Path::new(t)).await?;
                    }
                    self.db.reset_progress(id).await?;
                }
                let dir = PathBuf::from(&d.save_dir);
                tokio::fs::create_dir_all(&dir).await?;
                reserve_name(&dir, &probe.file_name).await?
            }
        };
        let cats = self.db.list_categories().await?;
        let file_name = final_path.file_name().unwrap().to_string_lossy().into_owned();
        self.db
            .save_probe(
                id,
                probe.final_url.as_str(),
                &file_name,
                &temp_path.to_string_lossy(),
                &final_path.to_string_lossy(),
                probe.mime.as_deref(),
                probe.total,
                probe.ranges,
                probe.etag.as_deref(),
                probe.last_modified.as_deref(),
                categories::categorize(&cats, &file_name).map(|c| c.id),
            )
            .await?;
        self.db.set_status(id, DownloadStatus::Downloading, None).await?;
        self.notify(id).await;

        // Ticks go to the UI immediately and to SQLite at most once per second.
        let (tx, mut rx) = watch::channel(None::<Tick>);
        let sink = self.events.clone();
        let on_tick: Arc<dyn Fn(Tick) + Send + Sync> = Arc::new(move |t: Tick| {
            sink.progress(id, &t);
            let _ = tx.send(Some(t));
        });
        let persister = {
            let db = self.db.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    match rx.has_changed() {
                        Ok(true) => {}
                        Ok(false) => continue,
                        Err(_) => break, // engine finished
                    }
                    let tick = rx.borrow_and_update().clone();
                    if let Some(t) = tick {
                        if let Err(e) = db.save_progress(id, &t.segments).await {
                            tracing::error!(id, error = %e, "failed to persist progress");
                        }
                    }
                }
            })
        };

        let job = Job { probe, headers, temp_path, final_path, segments, expected_sha256: d.expected_sha256.clone() };
        let outcome = engine::run(&client, &cfg, job, token, on_tick).await;
        let _ = persister.await;

        match outcome {
            Ok(Outcome::Completed { path, bytes, sha256 }) => {
                self.db.mark_completed(id, &path.to_string_lossy(), bytes, sha256.as_deref()).await?;
                self.headers.lock().unwrap().remove(&id);
                Ok(())
            }
            Ok(Outcome::Paused { segments }) => {
                self.db.save_progress(id, &segments).await?;
                self.after_cancel(id).await
            }
            Err(Error::RemoteChanged) => {
                // Never splice two versions of a file: drop progress so the next start is clean.
                let d = self.db.get_download(id).await?;
                if let Some(t) = &d.temp_path {
                    remove_if_exists(Path::new(t)).await?;
                }
                self.db.reset_progress(id).await?;
                Err(Error::RemoteChanged)
            }
            Err(e) => Err(e),
        }
    }

    async fn drive_media(self: &Arc<Self>, id: i64, d: Download, token: CancellationToken) -> Result<()> {
        let tools = self.media_tools()?;
        let choice: MediaChoice = self
            .db
            .get_metadata(id, "media_choice")
            .await?
            .and_then(|c| serde_json::from_str(&c).ok())
            .ok_or_else(|| Error::InvalidState("media download has no quality selection".into()))?;
        let expected_bytes = self.db.get_metadata(id, "expected_bytes").await?.and_then(|b| b.parse().ok());
        let save_dir = PathBuf::from(&d.save_dir);
        // Fragments and .part files live here; kept on pause (yt-dlp resumes them), removed on cancel/success.
        let work_dir = save_dir.join(format!(".gale-media-{id}"));
        sqlx::query("UPDATE downloads SET temp_path = ? WHERE id = ?").bind(work_dir.to_string_lossy().as_ref()).bind(id).execute(self.db.pool()).await?;
        self.db.set_status(id, DownloadStatus::Downloading, None).await?;
        self.notify(id).await;

        let (tx, mut rx) = watch::channel(None::<(u64, Option<u64>)>);
        let persister = {
            let db = self.db.clone();
            tokio::spawn(async move {
                while rx.changed().await.is_ok() {
                    let v = *rx.borrow_and_update();
                    if let Some((done, total)) = v {
                        if let Err(e) = db.set_downloaded(id, done, total).await {
                            tracing::error!(id, error = %e, "failed to persist media progress");
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            })
        };
        let sink = self.events.clone();
        let on_progress = move |p: media::MediaProgress| {
            sink.progress(id, &Tick { written: p.downloaded, total: p.total, bytes_per_sec: p.bytes_per_sec, active_connections: 0, segments: vec![], stage: Some(p.stage) });
            let _ = tx.send(Some((p.downloaded, p.total)));
        };
        let job = MediaJob {
            url: validate_url(&d.url)?,
            choice,
            save_dir,
            work_dir,
            concurrent_fragments: d.connections.clamp(1, 16) as u8,
            expected_bytes,
            net: self.media_net(),
        };
        let outcome = media::download(&tools, &job, &token, &on_progress).await;
        drop(on_progress); // closes the watch channel so the persister exits
        let _ = persister.await;
        match outcome? {
            MediaOutcome::Completed { path, bytes } => {
                self.db.mark_completed(id, &path.to_string_lossy(), bytes, None).await?;
                Ok(())
            }
            MediaOutcome::Paused => self.after_cancel(id).await,
        }
    }

    async fn torrent_session(&self) -> Result<Arc<librqbit::Session>> {
        self.torrent.get_or_try_init(torrent::new_session).await.cloned()
    }

    /// Downloads a .torrent file with the same client, proxy and browser cookies as any other download.
    async fn fetch_torrent_file(&self, id: i64, url: &str) -> Result<bytes::Bytes> {
        let client = self.client.lock().unwrap().clone();
        let headers = self.headers.lock().unwrap().get(&id).cloned().unwrap_or_default();
        let mut resp = client.get(validate_url(url)?).headers(headers.to_map()).send().await?;
        if !resp.status().is_success() {
            return Err(Error::Http { status: resp.status().as_u16() });
        }
        let mut buf = Vec::new();
        while let Some(chunk) = resp.chunk().await? {
            buf.extend_from_slice(&chunk);
            if buf.len() > torrent::MAX_TORRENT_FILE {
                return Err(Error::Torrent("the .torrent file is too large".into()));
            }
        }
        Ok(buf.into())
    }

    async fn drive_torrent(self: &Arc<Self>, id: i64, d: Download, token: CancellationToken) -> Result<()> {
        use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, TorrentStatsState};
        self.db.set_status(id, DownloadStatus::Downloading, None).await?;
        self.notify(id).await;
        let tick = |stage: Option<&'static str>, written: u64, total: Option<u64>, bps: f64, peers: u32| Tick {
            written,
            total,
            bytes_per_sec: bps,
            active_connections: peers,
            segments: vec![],
            stage,
        };
        let total = d.total_bytes.map(|t| t as u64);
        self.events.progress(id, &tick(Some("metadata"), d.downloaded_bytes as u64, total, 0.0, 0));
        let session = self.torrent_session().await?;
        let save_dir = PathBuf::from(&d.save_dir);
        tokio::fs::create_dir_all(&save_dir).await?;

        // The torrent's metadata is kept after the first start, so resuming needs neither the original .torrent
        // link (which may have expired) nor peers to send the metadata of a magnet link again.
        let (meta, folder) = match (self.db.get_metadata(id, "torrent").await?, self.db.get_metadata(id, "torrent_folder").await?) {
            (Some(hex), Some(folder)) => {
                (bytes::Bytes::from(hex::decode(hex).map_err(|_| Error::Torrent("the saved torrent data is damaged".into()))?), folder)
            }
            _ => {
                let add = if torrent::is_magnet(&d.url) {
                    AddTorrent::from_url(torrent::validate_magnet(&d.url)?)
                } else if let Some(path) = torrent::local_torrent(&d.url) {
                    let bytes = tokio::fs::read(&path).await?;
                    if bytes.len() > torrent::MAX_TORRENT_FILE {
                        return Err(Error::Torrent("the .torrent file is too large".into()));
                    }
                    AddTorrent::from_bytes(bytes)
                } else {
                    AddTorrent::from_bytes(self.fetch_torrent_file(id, &d.url).await?)
                };
                let mut list = session.add_torrent(add, Some(AddTorrentOptions { list_only: true, ..Default::default() }));
                // Repeat the stage so a progress window opened meanwhile shows it too.
                let mut every = tokio::time::interval(Duration::from_secs(1));
                let resp = loop {
                    tokio::select! {
                        r = &mut list => break r.map_err(torrent::err)?,
                        _ = every.tick() => self.events.progress(id, &tick(Some("metadata"), 0, None, 0.0, 0)),
                        _ = token.cancelled() => return self.after_cancel(id).await,
                    }
                };
                let AddTorrentResponse::ListOnly(l) = resp else {
                    return Err(Error::Torrent("could not read the torrent".into()));
                };
                // Multi-file torrents get their own folder (librqbit's choice of name); single files go straight in.
                let sub = l.output_folder.strip_prefix(torrent::session_root()).map(|p| p.to_path_buf()).unwrap_or_default();
                let target = match l.info.iter_file_details().next() {
                    Some(f) if sub.as_os_str().is_empty() => f.filename.to_pathbuf(),
                    _ => sub.clone(),
                };
                let name = sanitize_filename(&l.info.name().map(|n| n.into_owned()).unwrap_or_else(|| d.file_name.clone()));
                let bytes_total: u64 = l.info.iter_file_lengths().sum();
                // Never write into a file or folder that is already there: use a new "Name (1)" folder instead.
                let (folder, final_path) = if save_dir.join(&target).exists() {
                    let dir = unique_path(&save_dir, &target.to_string_lossy());
                    let inside = if sub.as_os_str().is_empty() { dir.join(&target) } else { dir.clone() };
                    (dir, inside)
                } else {
                    (save_dir.join(&sub), save_dir.join(&target))
                };
                let cats = self.db.list_categories().await?;
                sqlx::query("UPDATE downloads SET file_name = ?, final_path = ?, total_bytes = ?, category_id = ? WHERE id = ?")
                    .bind(&name)
                    .bind(final_path.to_string_lossy().as_ref())
                    .bind(bytes_total as i64)
                    .bind(categories::categorize(&cats, &name).map(|c| c.id))
                    .bind(id)
                    .execute(self.db.pool())
                    .await?;
                self.db.set_metadata(id, "torrent", &hex::encode(&l.torrent_bytes)).await?;
                let folder = folder.to_string_lossy().into_owned();
                self.db.set_metadata(id, "torrent_folder", &folder).await?;
                self.db.set_metadata(id, "torrent_path", &final_path.to_string_lossy()).await?;
                self.notify(id).await;
                (l.torrent_bytes, folder)
            }
        };

        let rate = self.media_net().rate_limit_bps;
        let opts = AddTorrentOptions {
            output_folder: Some(folder),
            // Keep pieces already on disk: librqbit checks them and downloads only what is missing.
            overwrite: true,
            ratelimits: librqbit::limits::LimitsConfig { download_bps: u32::try_from(rate).ok().and_then(std::num::NonZeroU32::new), upload_bps: None },
            ..Default::default()
        };
        let handle = session
            .add_torrent(AddTorrent::from_bytes(meta), Some(opts))
            .await
            .map_err(torrent::err)?
            .into_handle()
            .ok_or_else(|| Error::Torrent("could not start the torrent".into()))?;
        let tid = handle.id();

        let outcome = loop {
            let s = handle.stats();
            let total = Some(s.total_bytes);
            match &s.state {
                TorrentStatsState::Error => break Err(Error::Torrent(s.error.clone().unwrap_or_else(|| "torrent error".into()))),
                // While checking, librqbit reports bytes *checked*, not bytes it has: keep the last real figure.
                TorrentStatsState::Initializing { .. } => {
                    self.events.progress(id, &tick(Some("checking"), d.downloaded_bytes as u64, total, 0.0, 0));
                }
                _ => {
                    let (bps, peers) = s.live.as_ref().map(|l| (l.download_speed.as_bytes() as f64, l.snapshot.peer_stats.live)).unwrap_or((0.0, 0));
                    self.events.progress(id, &tick(None, s.progress_bytes, total, bps, peers));
                    if let Err(e) = self.db.set_downloaded(id, s.progress_bytes, total).await {
                        tracing::error!(id, error = %e, "failed to persist torrent progress");
                    }
                }
            }
            if s.finished && matches!(s.state, TorrentStatsState::Live | TorrentStatsState::Paused) {
                break Ok(s.total_bytes);
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                _ = token.cancelled() => break Err(Error::InvalidState("stopped".into())),
            }
        };
        // Gale does not seed after a download finishes or while it is paused.
        let cancelled = self.db.get_download(id).await?.status == DownloadStatus::Cancelled;
        if let Err(e) = session.delete(tid.into(), cancelled && outcome.is_err()).await {
            tracing::warn!(id, error = %e, "could not remove torrent from session");
        }
        match outcome {
            Ok(bytes) => {
                let path = self.db.get_metadata(id, "torrent_path").await?.unwrap_or(d.save_dir.clone());
                self.db.mark_completed(id, &path, bytes, None).await?;
                Ok(())
            }
            Err(_) if token.is_cancelled() => self.after_cancel(id).await,
            Err(e) => Err(e),
        }
    }

    /// A cancelled token means pause unless `cancel()` already recorded a terminal status.
    async fn after_cancel(&self, id: i64) -> Result<()> {
        let d = self.db.get_download(id).await?;
        if d.status != DownloadStatus::Cancelled {
            self.db.set_status(id, DownloadStatus::Paused, None).await?;
        }
        Ok(())
    }

    pub async fn pause(&self, id: i64) -> Result<()> {
        let token = self.running.lock().unwrap().get(&id).cloned();
        match token {
            Some(t) => t.cancel(),
            None => {
                let d = self.db.get_download(id).await?;
                if d.status == DownloadStatus::Queued {
                    self.db.set_status(id, DownloadStatus::Paused, None).await?;
                    self.notify(id).await;
                }
            }
        }
        Ok(())
    }

    /// Stop and wait until the background task has released the download.
    pub async fn stop_and_wait(&self, id: i64) -> Result<()> {
        self.pause(id).await?;
        for _ in 0..200 {
            if !self.is_running(id) {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        Err(Error::InvalidState("download did not stop in time".into()))
    }

    /// Cancel: stop transfer and delete the partial temp file (never the completed file).
    /// shortcut: a torrent cancelled while paused keeps its partial files (librqbit deletes them only for a
    /// torrent in its session); delete them by re-adding it paused if people ask.
    pub async fn cancel(&self, id: i64) -> Result<()> {
        if self.db.get_download(id).await?.status == DownloadStatus::Completed {
            return Err(Error::InvalidState("a completed download cannot be cancelled".into()));
        }
        self.db.set_status(id, DownloadStatus::Cancelled, None).await?;
        self.stop_and_wait(id).await?;
        let d = self.db.get_download(id).await?;
        if let Some(t) = &d.temp_path {
            remove_if_exists(Path::new(t)).await?;
        }
        self.db.reset_progress(id).await?;
        self.db.set_status(id, DownloadStatus::Cancelled, None).await?;
        self.notify(id).await;
        Ok(())
    }

    pub async fn restart(self: &Arc<Self>, id: i64) -> Result<()> {
        self.stop_and_wait(id).await?;
        let d = self.db.get_download(id).await?;
        if let Some(t) = &d.temp_path {
            remove_if_exists(Path::new(t)).await?;
        }
        self.db.reset_progress(id).await?;
        sqlx::query("UPDATE downloads SET temp_path = NULL, final_path = NULL, total_bytes = NULL WHERE id = ? AND status != 'completed'")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        if d.status == DownloadStatus::Completed {
            // The completed file stays where it is; restart downloads a fresh copy alongside it.
            sqlx::query("UPDATE downloads SET status = 'paused', temp_path = NULL, final_path = NULL, total_bytes = NULL WHERE id = ?")
                .bind(id)
                .execute(self.db.pool())
                .await?;
        }
        self.start(id).await
    }

    /// Remove from the list. The downloaded file is deleted only when the user explicitly asked.
    pub async fn remove(&self, id: i64, delete_file: bool) -> Result<()> {
        self.stop_and_wait(id).await?;
        let d = self.db.get_download(id).await?;
        if let Some(t) = &d.temp_path {
            remove_if_exists(Path::new(t)).await?;
        }
        if delete_file && d.status == DownloadStatus::Completed {
            if let Some(f) = &d.final_path {
                remove_if_exists(Path::new(f)).await?;
            }
        }
        self.db.delete_download(id).await?;
        self.headers.lock().unwrap().remove(&id);
        Ok(())
    }

    /// Pause everything (app exit). State is already persisted, so this is just a clean stop.
    pub async fn shutdown(&self) {
        let ids: Vec<i64> = self.running.lock().unwrap().keys().copied().collect();
        for id in ids {
            let _ = self.stop_and_wait(id).await;
        }
    }

    async fn notify(&self, id: i64) {
        if let Ok(d) = self.db.get_download(id).await {
            self.events.changed(&d);
        }
    }
}

fn media_category(cats: &[categories::Category], m: &MediaRequest) -> Option<i64> {
    let want = if m.kind == "audio" { "Audio" } else { "Video" };
    cats.iter().find(|c| c.name == want).map(|c| c.id)
}

/// Atomically claim a free name by creating its temp file, so parallel downloads never collide.
async fn reserve_name(dir: &Path, name: &str) -> Result<(PathBuf, PathBuf)> {
    for _ in 0..1000 {
        let final_path = unique_path(dir, name);
        let temp = temp_path_for(&final_path);
        match tokio::fs::OpenOptions::new().write(true).create_new(true).open(&temp).await {
            Ok(_) => return Ok((temp, final_path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(Error::InvalidState(format!("no free file name for {name}")))
}

impl Manager {
    pub async fn start_queue(self: &Arc<Self>, queue_id: i64) -> Result<()> {
        self.db.set_queue_state(queue_id, "running").await?;
        self.pump_queues().await
    }

    /// Stop: active items are paused and nothing new starts.
    pub async fn stop_queue(&self, queue_id: i64) -> Result<()> {
        self.db.set_queue_state(queue_id, "stopped").await?;
        self.shutdown_queues.lock().unwrap().remove(&queue_id);
        self.busy_queues.lock().unwrap().remove(&queue_id);
        for (id, _) in self.db.queue_items(queue_id).await? {
            if self.is_running(id) {
                self.pause(id).await?;
            }
        }
        Ok(())
    }

    /// Pause: let active items finish, start nothing new.
    pub async fn pause_queue(&self, queue_id: i64) -> Result<()> {
        self.db.set_queue_state(queue_id, "paused").await
    }

    /// Shut Windows down when this queue next runs dry (cancelled by stopping the queue).
    pub fn shutdown_after_queue(&self, queue_id: i64, on: bool) {
        let mut s = self.shutdown_queues.lock().unwrap();
        if on { s.insert(queue_id); } else { s.remove(&queue_id); }
    }

    /// Start waiting items in every running queue, up to each queue's limit, in queue order.
    pub async fn pump_queues(self: &Arc<Self>) -> Result<()> {
        let _g = self.pump_lock.lock().await;
        for q in self.db.list_queues().await?.into_iter().filter(|q| q.state == "running") {
            let items = self.db.queue_items(q.id).await?;
            let mut active = items.iter().filter(|(id, _)| self.is_running(*id)).count() as i64;
            let waiting: Vec<i64> = items.iter().filter(|(id, st)| st == "queued" && !self.is_running(*id)).map(|(id, _)| *id).collect();
            for id in waiting.iter().take((q.max_concurrent - active).max(0) as usize) {
                sqlx::query("UPDATE downloads SET connections = MIN(connections, ?) WHERE id = ?")
                    .bind(q.max_connections).bind(id).execute(self.db.pool()).await?;
                if let Err(e) = self.start(*id).await {
                    tracing::warn!(id, error = %e, "queue could not start download");
                }
                active += 1;
            }
            let idle = active == 0 && waiting.is_empty();
            let was_busy = if idle { self.busy_queues.lock().unwrap().remove(&q.id) } else { !self.busy_queues.lock().unwrap().insert(q.id) };
            if idle && was_busy {
                let shutdown = self.shutdown_queues.lock().unwrap().remove(&q.id);
                tracing::info!(queue = q.id, shutdown, "queue finished");
                self.events.queue_finished(q.id, shutdown);
            }
        }
        Ok(())
    }

    /// Run due scheduler jobs. Called every few seconds by `spawn_background`.
    pub async fn run_scheduler(self: &Arc<Self>, now: chrono::DateTime<chrono::Utc>) -> Result<()> {
        for job in self.db.take_due_jobs(now).await? {
            tracing::info!(job = job.id, kind = %job.kind, queue = job.queue_id, "scheduler job due");
            match job.kind.as_str() {
                "start_queue" => {
                    self.shutdown_after_queue(job.queue_id, job.shutdown_after);
                    self.start_queue(job.queue_id).await?;
                }
                _ => {
                    self.stop_queue(job.queue_id).await?;
                    if job.shutdown_after {
                        self.events.queue_finished(job.queue_id, true);
                    }
                }
            }
        }
        Ok(())
    }

    /// Queue pump + scheduler loop for the lifetime of the app.
    /// Starts failed downloads whose automatic retry is due, if they are still failed (not removed or restarted).
    async fn retry_due(self: &Arc<Self>) {
        let now = Instant::now();
        let due: Vec<i64> = {
            let mut r = self.retries.lock().unwrap();
            let ids: Vec<i64> = r.iter().filter(|(_, v)| v.1 <= now).map(|(id, _)| *id).collect();
            for id in &ids {
                // Push far out; the next failure sets a new time, success removes the entry.
                r.get_mut(id).unwrap().1 = now + Duration::from_secs(86_400 * 365);
            }
            ids
        };
        for id in due {
            if matches!(self.db.get_download(id).await, Ok(d) if d.status == DownloadStatus::Failed) {
                tracing::info!(id, "retrying after a network error");
                if let Err(e) = self.start(id).await {
                    tracing::warn!(id, error = %e, "automatic retry could not start");
                }
            }
        }
    }

    pub fn spawn_background(self: &Arc<Self>) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(2));
            loop {
                tokio::select! { _ = tick.tick() => {}, _ = this.pump_now.notified() => {} }
                if let Err(e) = this.run_scheduler(chrono::Utc::now()).await {
                    tracing::error!(error = %e, "scheduler tick failed");
                }
                if let Err(e) = this.pump_queues().await {
                    tracing::error!(error = %e, "queue pump failed");
                }
                this.retry_due().await;
            }
        });
    }
}

async fn remove_if_exists(p: &Path) -> Result<()> {
    if p.is_dir() {
        return Ok(tokio::fs::remove_dir_all(p).await?); // media work folder
    }
    match tokio::fs::remove_file(p).await {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}
