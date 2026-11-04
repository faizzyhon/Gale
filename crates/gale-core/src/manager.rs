//! Owns running downloads: persistence of state transitions, resume validation, concurrency limit.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{watch, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::db::{Db, Download, DownloadStatus, NewDownload};
use crate::engine::{self, EngineConfig, Job, Outcome, RequestHeaders, Tick};
use crate::security::{sanitize_filename, temp_path_for, unique_path, validate_url};
use crate::{categories, Error, Result};

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
    /// Cookies / auth from the browser. Kept in memory for this session only.
    #[serde(skip)]
    pub headers: RequestHeaders,
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
}

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
        }))
    }

    pub fn db(&self) -> &Db {
        &self.db
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
        let url = validate_url(&opts.url)?;
        if let Some(existing) = self.db.find_unfinished_by_url(url.as_str()).await? {
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
        let name = opts
            .file_name
            .clone()
            .or_else(|| crate::security::filename_from_url(&url))
            .map(|n| sanitize_filename(&n))
            .unwrap_or_else(|| "download".into());
        let cats = self.db.list_categories().await?;
        // Read before any await: a guard held across `.await` deadlocks the runtime.
        let connections = opts.connections.unwrap_or_else(|| self.cfg.lock().unwrap().connections) as i64;
        let id = self
            .db
            .insert_download(&NewDownload {
                url: url.to_string(),
                referrer: opts.referrer,
                file_name: name.clone(),
                save_dir: opts.save_dir,
                connections,
                category_id: categories::categorize(&cats, &name).map(|c| c.id),
                expected_sha256: opts.expected_sha256,
                source: opts.source,
                status: Some(if opts.start || opts.queue_id.is_some() { DownloadStatus::Queued } else { DownloadStatus::Paused }),
            })
            .await?;
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
                Ok(()) => Ok(()),
                Err(e) => {
                    tracing::warn!(id, code = e.code(), error = %e, "download failed");
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
            }
        });
    }
}

async fn remove_if_exists(p: &Path) -> Result<()> {
    match tokio::fs::remove_file(p).await {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}
