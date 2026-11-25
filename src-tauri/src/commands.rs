//! Thin IPC adapters. File-system actions take a download id, never a path from the webview
//! (the only exceptions are paths the user picked in a native file dialog for backup/restore).

use std::path::{Path, PathBuf};
use std::time::Instant;

use gale_core::categories::{self, Category};
use gale_core::db::{DashboardStats, Db, Download, DownloadStatus, HistoryEntry, Notification};
use gale_core::engine::{self, ProbeResult, RequestHeaders, SegmentState};
use gale_core::grabber::{self, GrabOptions, GrabResult};
use gale_core::manager::AddOptions;
use gale_core::media::MediaInfo;
use gale_core::queue::{Queue, QueueInput, SchedulerJob};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;
use tokio_util::sync::CancellationToken;

use crate::settings::{self, AppSettings};
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl From<gale_core::Error> for CommandError {
    fn from(e: gale_core::Error) -> Self {
        Self { code: e.code().into(), message: e.to_string() }
    }
}

fn err(code: &str, message: impl Into<String>) -> CommandError {
    CommandError { code: code.into(), message: message.into() }
}

type Res<T> = Result<T, CommandError>;

// ---------------------------------------------------------------------------------------------
// Downloads

/// Applies Security → blocked file types and General → category folders, then adds.
pub async fn add_with_policy(st: &AppState, mut opts: AddOptions) -> Res<i64> {
    let s = st.settings();
    use gale_core::torrent;
    let name = if torrent::is_magnet(&opts.url) {
        torrent::magnet_name(&torrent::validate_magnet(&opts.url)?).unwrap_or_default()
    } else if let Some(p) = torrent::local_torrent(&opts.url) {
        p.file_name().unwrap_or_default().to_string_lossy().into_owned()
    } else {
        let url = gale_core::security::validate_url(&opts.url)?;
        opts.file_name.clone().or_else(|| gale_core::security::filename_from_url(&url)).unwrap_or_default()
    };
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if !ext.is_empty() && s.blocked_extensions.contains(&ext) {
        return Err(err("blocked", format!(".{ext} files are blocked in Settings → Security")));
    }
    if s.category_folders && Path::new(&opts.save_dir) == Path::new(&s.download_dir) {
        let cats = st.manager.db().list_categories().await?;
        if let Some(c) = categories::categorize(&cats, &name) {
            opts.save_dir = Path::new(&s.download_dir).join(&c.subfolder).to_string_lossy().into_owned();
        }
    }
    Ok(st.manager.add(opts).await?)
}

#[tauri::command]
pub async fn list_downloads(state: State<'_, AppState>) -> Res<Vec<Download>> {
    Ok(state.manager.db().list_downloads().await?)
}

/// `pending_id` links a browser download confirmed in the Add dialog to its cookies (held in Rust).
#[tauri::command]
pub async fn add_download(app: AppHandle, state: State<'_, AppState>, mut opts: AddOptions, pending_id: Option<String>) -> Res<i64> {
    if let Some(p) = pending_id {
        opts.headers = state.take_pending(&p).unwrap_or_default();
        opts.source = Some("browser".into());
    }
    let starts_now = opts.start && opts.queue_id.is_none();
    let id = add_with_policy(&state, opts).await?;
    if starts_now {
        crate::windows::want_progress_window(&app, id).await;
    }
    Ok(id)
}

#[tauri::command]
pub async fn get_download(state: State<'_, AppState>, id: i64) -> Res<Download> {
    Ok(state.manager.db().get_download(id).await?)
}

/// What the browser sent for a download awaiting confirmation (no cookies/headers).
#[tauri::command]
pub fn get_pending(state: State<'_, AppState>, pending_id: String) -> Res<serde_json::Value> {
    state.pending.lock().unwrap().get(&pending_id).map(|p| p.info.clone()).ok_or_else(|| err("expired", "this download request has expired"))
}

/// Size / resumability for the pop-up, fetched with the browser's cookies (they stay in Rust).
#[tauri::command]
pub async fn probe_pending(state: State<'_, AppState>, pending_id: String) -> Res<ProbeResult> {
    let (url, headers) = {
        let p = state.pending.lock().unwrap();
        let p = p.get(&pending_id).ok_or_else(|| err("expired", "this download request has expired"))?;
        (p.info["url"].as_str().unwrap_or_default().to_string(), p.headers.clone())
    };
    if gale_core::torrent::is_magnet(&url) {
        // Nothing to ask a server; the size is known once peers send the torrent's metadata.
        let m = gale_core::torrent::validate_magnet(&url)?;
        let name = gale_core::torrent::magnet_name(&m).unwrap_or_else(|| "torrent".into());
        let final_url = url::Url::parse(&m).map_err(|_| err("invalid_url", "invalid magnet link"))?;
        return Ok(ProbeResult { final_url, total: None, ranges: true, etag: None, last_modified: None, file_name: name, mime: None });
    }
    let u = gale_core::security::validate_url(&url)?;
    let client = engine::build_client(&state.manager.config())?;
    let name = state.pending.lock().unwrap().get(&pending_id).and_then(|p| p.info["fileName"].as_str().map(str::to_owned));
    Ok(engine::probe(&client, &u, &headers, name.as_deref()).await?)
}

#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    crate::show_main(&app);
}

const REPO: &str = "https://github.com/faizzyhon/Gale";

/// About-page links. The page passes a name, never a URL, so it cannot open arbitrary addresses.
pub fn link_url(name: &str) -> Option<String> {
    Some(match name {
        "website" => "https://faizzyhon.com".into(),
        "github" => "https://github.com/faizzyhon".into(),
        "linkedin" => "https://www.linkedin.com/in/faizzyhon/".into(),
        "repo" => REPO.into(),
        "issues" => format!("{REPO}/issues"),
        "releases" => format!("{REPO}/releases"),
        "privacy" => format!("{REPO}/blob/main/PRIVACY.md"),
        "terms" => format!("{REPO}/blob/main/TERMS.md"),
        "license" => format!("{REPO}/blob/main/LICENSE"),
        "notices" => format!("{REPO}/blob/main/scripts/installer/THIRD-PARTY-NOTICES.txt"),
        _ => return None,
    })
}

#[tauri::command]
pub fn open_link(app: AppHandle, name: String) -> Result<(), CommandError> {
    let url = link_url(&name).ok_or_else(|| err("invalid", "unknown link"))?;
    app.opener().open_url(url, None::<&str>).map_err(|e| err("open_failed", e.to_string()))
}

/// Opens a release page or installer link, only from Gale's own GitHub repository.
#[tauri::command]
pub fn open_release_url(app: AppHandle, url: String) -> Result<(), CommandError> {
    if !url.starts_with("https://github.com/faizzyhon/Gale/") || url.contains("..") {
        return Err(err("invalid", "not a Gale release link"));
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| err("open_failed", e.to_string()))
}

#[tauri::command]
pub async fn check_update() -> Result<crate::update::UpdateInfo, CommandError> {
    crate::update::check().await.map_err(|e| err("update", e))
}

/// Downloads and checks the installer, starts it, and closes Gale so it can be replaced.
#[tauri::command]
pub async fn install_update(app: AppHandle, installer: String) -> Result<(), CommandError> {
    if crate::startup::is_packaged() {
        return Err(err("update", "the Microsoft Store updates this copy of Gale"));
    }
    let msi = crate::update::download(&installer).await.map_err(|e| err("update", e))?;
    crate::update::launch(&msi).map_err(|e| err("update", e))?;
    let st = app.state::<AppState>();
    st.manager.shutdown().await; // save progress before the installer closes Gale
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn list_networks() -> Vec<gale_core::engine::Network> {
    gale_core::engine::list_networks()
}

/// Points a broken download at a new address it was given (pasted by the user). Progress is kept.
#[tauri::command]
pub async fn refresh_url(state: State<'_, AppState>, id: i64, url: String) -> Result<(), CommandError> {
    state.manager.refresh_url(id, &url, None, true).await?;
    Ok(())
}

/// Opens the page the download came from; the next download the browser extension catches with the same size
/// becomes this download's new address (with the browser's fresh cookies). Waits 5 minutes at most.
#[tauri::command]
pub async fn refresh_from_browser(app: AppHandle, state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    let d = state.manager.db().get_download(id).await?;
    let page = d.referrer.as_deref().filter(|r| gale_core::security::validate_url(r).is_ok()).unwrap_or(&d.url).to_string();
    *state.refreshing.lock().unwrap() = Some((id, std::time::Instant::now()));
    app.opener().open_url(page, None::<&str>).map_err(|e| err("open_failed", e.to_string()))
}

#[cfg(test)]
mod link_tests {
    #[test]
    fn only_known_links_open() {
        assert_eq!(super::link_url("website").as_deref(), Some("https://faizzyhon.com"));
        assert!(super::link_url("https://evil.example").is_none());
        assert!(super::link_url("").is_none());
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    url: String,
    id: Option<i64>,
    error: Option<String>,
}

/// One result per URL so the UI can show exactly which lines were rejected.
#[tauri::command]
pub async fn add_batch(state: State<'_, AppState>, urls: Vec<String>, save_dir: String, start: bool, queue_id: Option<i64>, source: Option<String>) -> Res<Vec<BatchResult>> {
    if urls.len() > 5000 {
        return Err(err("too_many", "at most 5000 URLs per batch"));
    }
    let mut out = Vec::with_capacity(urls.len());
    for url in urls.into_iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()) {
        let opts = AddOptions {
            url: url.clone(),
            save_dir: save_dir.clone(),
            start: start && queue_id.is_none(),
            queue_id,
            source: Some(source.clone().filter(|s| matches!(s.as_str(), "grabber" | "browser")).unwrap_or_else(|| "batch".into())),
            ..Default::default()
        };
        out.push(match add_with_policy(&state, opts).await {
            Ok(id) => BatchResult { url, id: Some(id), error: None },
            Err(e) => BatchResult { url, id: None, error: Some(e.message) },
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn resume_download(app: AppHandle, state: State<'_, AppState>, id: i64) -> Res<()> {
    state.manager.start(id).await?;
    crate::windows::want_progress_window(&app, id).await;
    Ok(())
}

#[tauri::command]
pub async fn pause_download(state: State<'_, AppState>, id: i64) -> Res<()> {
    Ok(state.manager.pause(id).await?)
}

#[tauri::command]
pub async fn cancel_download(state: State<'_, AppState>, id: i64) -> Res<()> {
    Ok(state.manager.cancel(id).await?)
}

#[tauri::command]
pub async fn restart_download(app: AppHandle, state: State<'_, AppState>, id: i64) -> Res<()> {
    state.manager.restart(id).await?;
    crate::windows::want_progress_window(&app, id).await;
    Ok(())
}

/// `delete_file` must come from an explicit confirmation in the UI.
#[tauri::command]
pub async fn remove_download(state: State<'_, AppState>, id: i64, delete_file: bool) -> Res<()> {
    Ok(state.manager.remove(id, delete_file).await?)
}

#[tauri::command]
pub async fn recheck_download(state: State<'_, AppState>, id: i64) -> Res<ProbeResult> {
    Ok(state.manager.recheck(id).await?)
}

async fn completed_path(state: &State<'_, AppState>, id: i64) -> Res<String> {
    let d = state.manager.db().get_download(id).await?;
    match (d.status, d.final_path) {
        (DownloadStatus::Completed, Some(p)) if Path::new(&p).exists() => Ok(p),
        (DownloadStatus::Completed, _) => Err(err("missing_file", "the file was moved or deleted")),
        _ => Err(err("not_completed", "the download has not finished")),
    }
}

#[tauri::command]
pub async fn open_download(app: AppHandle, state: State<'_, AppState>, id: i64) -> Res<()> {
    let p = completed_path(&state, id).await?;
    app.opener().open_path(p, None::<&str>).map_err(|e| err("open_failed", e.to_string()))
}

/// Opens the folder with the file selected; for unfinished downloads opens the save folder.
#[tauri::command]
pub async fn reveal_download(app: AppHandle, state: State<'_, AppState>, id: i64) -> Res<()> {
    let r = match completed_path(&state, id).await {
        Ok(p) => app.opener().reveal_item_in_dir(p),
        Err(_) => {
            let d = state.manager.db().get_download(id).await?;
            app.opener().open_path(d.save_dir, None::<&str>)
        }
    };
    r.map_err(|e| err("open_failed", e.to_string()))
}

#[tauri::command]
pub async fn get_download_segments(state: State<'_, AppState>, id: i64) -> Res<Vec<SegmentState>> {
    Ok(state.manager.db().load_segments(id).await?)
}

#[tauri::command]
pub async fn dashboard(state: State<'_, AppState>) -> Res<DashboardStats> {
    Ok(state.manager.db().dashboard().await?)
}

#[tauri::command]
pub async fn list_categories(state: State<'_, AppState>) -> Res<Vec<Category>> {
    Ok(state.manager.db().list_categories().await?)
}

#[tauri::command]
pub async fn list_history(state: State<'_, AppState>, limit: Option<i64>) -> Res<Vec<HistoryEntry>> {
    Ok(state.manager.db().list_history(limit.unwrap_or(500).clamp(1, 10_000)).await?)
}

// ---------------------------------------------------------------------------------------------
// Settings

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    #[serde(flatten)]
    settings: AppSettings,
    has_proxy_password: bool,
    has_hf_token: bool,
    /// Store (MSIX) install: "start with Windows" is set in Windows Settings → Apps → Startup instead.
    packaged: bool,
}

fn view(s: AppSettings) -> SettingsView {
    SettingsView {
        settings: s,
        has_proxy_password: settings::has_proxy_password(),
        has_hf_token: settings::hf_token().is_some(),
        packaged: crate::startup::is_packaged(),
    }
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Res<SettingsView> {
    Ok(view(state.settings()))
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: AppSettings) -> Res<SettingsView> {
    let s = settings.validated().map_err(|m| err("invalid_settings", m))?;
    let old = state.settings();
    state.manager.reconfigure(s.engine_config(&state.global, &state.hosts))?;
    state.manager.set_auto_retry(s.auto_resume);
    if s.global_limit_kbps != old.global_limit_kbps {
        crate::sync_tray_limit(&app, s.global_limit_kbps);
    }
    if s.start_with_windows != old.start_with_windows && !crate::startup::is_packaged() {
        let al = app.autolaunch();
        let r = if s.start_with_windows { al.enable() } else { al.disable() };
        r.map_err(|e| err("autostart", format!("could not change startup setting: {e}")))?;
        if s.start_with_windows {
            crate::startup::write_run_entry();
        }
    }
    state.manager.db().set_setting(settings::KEY, &s).await?;
    state.set_settings(s.clone());
    Ok(view(s))
}

/// Password goes straight to Windows Credential Manager; it never touches SQLite or logs.
#[tauri::command]
pub async fn set_proxy_password(state: State<'_, AppState>, password: Option<String>) -> Res<()> {
    settings::set_proxy_password(password.as_deref()).map_err(|e| err("credentials", e))?;
    let s = state.settings();
    state.manager.reconfigure(s.engine_config(&state.global, &state.hosts))?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTest {
    ok: bool,
    millis: u64,
    resumable: bool,
    size: Option<u64>,
    message: String,
}

/// Network diagnostics: probe a URL through the current proxy/timeout settings.
#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>, url: String) -> Res<ConnectionTest> {
    let u = gale_core::security::validate_url(&url)?;
    let client = engine::build_client(&state.manager.config())?;
    let t = Instant::now();
    let r = engine::probe(&client, &u, &RequestHeaders::default(), None).await;
    let millis = t.elapsed().as_millis() as u64;
    Ok(match r {
        Ok(p) => ConnectionTest { ok: true, millis, resumable: p.ranges, size: p.total, message: format!("Reached {}", p.final_url.host_str().unwrap_or("server")) },
        Err(e) => ConnectionTest { ok: false, millis, resumable: false, size: None, message: e.to_string() },
    })
}

#[tauri::command]
pub async fn reset_settings(state: State<'_, AppState>) -> Res<SettingsView> {
    let keep_dir = state.settings().download_dir;
    let s = AppSettings { download_dir: keep_dir, ..Default::default() };
    state.manager.reconfigure(s.engine_config(&state.global, &state.hosts))?;
    state.manager.db().set_setting(settings::KEY, &s).await?;
    state.set_settings(s.clone());
    Ok(view(s))
}

#[tauri::command]
pub async fn backup_database(state: State<'_, AppState>, dest: String) -> Res<()> {
    let p = PathBuf::from(&dest);
    if !p.is_absolute() {
        return Err(err("invalid_path", "choose a location with the dialog"));
    }
    Ok(state.manager.db().backup_to(&p).await?)
}

/// Validated now, swapped in at next start (the live database can't be replaced while open).
#[tauri::command]
pub async fn restore_database(app: AppHandle, source: String) -> Res<()> {
    let src = PathBuf::from(&source);
    Db::check_backup(&src).await?;
    let target = app.path().app_data_dir().map_err(|e| err("io", e.to_string()))?.join("gale.db.restore");
    tokio::fs::copy(&src, &target).await.map_err(|e| err("io", e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn open_logs(app: AppHandle) -> Res<()> {
    let dir = app.path().app_log_dir().map_err(|e| err("io", e.to_string()))?;
    std::fs::create_dir_all(&dir).map_err(|e| err("io", e.to_string()))?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| err("open_failed", e.to_string()))
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.state::<AppState>().quitting.store(true, std::sync::atomic::Ordering::SeqCst);
    app.exit(0);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    last_contact: Option<String>,
    intercepted: i64,
}

#[tauri::command]
pub async fn browser_status(state: State<'_, AppState>) -> Res<BrowserStatus> {
    let (last_contact, intercepted) = state.manager.db().browser_activity().await?;
    Ok(BrowserStatus { last_contact, intercepted })
}

// ---------------------------------------------------------------------------------------------
// Queues and scheduler

#[tauri::command]
pub async fn list_queues(state: State<'_, AppState>) -> Res<Vec<Queue>> {
    Ok(state.manager.db().list_queues().await?)
}

#[tauri::command]
pub async fn save_queue(state: State<'_, AppState>, id: Option<i64>, queue: QueueInput) -> Res<i64> {
    Ok(state.manager.db().save_queue(id, &queue).await?)
}

#[tauri::command]
pub async fn delete_queue(state: State<'_, AppState>, id: i64) -> Res<()> {
    state.manager.stop_queue(id).await?;
    Ok(state.manager.db().delete_queue(id).await?)
}

#[tauri::command]
pub async fn queue_action(state: State<'_, AppState>, id: i64, action: String) -> Res<()> {
    match action.as_str() {
        "start" => state.manager.start_queue(id).await?,
        "stop" => state.manager.stop_queue(id).await?,
        "pause" => state.manager.pause_queue(id).await?,
        _ => return Err(err("invalid", "unknown queue action")),
    }
    Ok(())
}

#[tauri::command]
pub async fn queue_memberships(state: State<'_, AppState>) -> Res<Vec<(i64, i64)>> {
    Ok(state.manager.db().queue_memberships().await?)
}

/// Move a download into a queue (it waits for the queue) or out of all queues.
#[tauri::command]
pub async fn set_download_queue(state: State<'_, AppState>, download_id: i64, queue_id: Option<i64>) -> Res<()> {
    let db = state.manager.db();
    match queue_id {
        Some(q) => {
            db.enqueue(q, download_id).await?;
            let d = db.get_download(download_id).await?;
            if matches!(d.status, DownloadStatus::Paused | DownloadStatus::Failed | DownloadStatus::Cancelled) && !state.manager.is_running(download_id) {
                db.set_status(download_id, DownloadStatus::Queued, None).await?;
            }
            state.manager.pump_queues().await?;
        }
        None => db.dequeue(download_id).await?,
    }
    Ok(())
}

#[tauri::command]
pub async fn list_jobs(state: State<'_, AppState>) -> Res<Vec<SchedulerJob>> {
    Ok(state.manager.db().list_jobs().await?)
}

#[tauri::command]
pub async fn save_job(state: State<'_, AppState>, job: SchedulerJob) -> Res<i64> {
    Ok(state.manager.db().save_job(&job).await?)
}

#[tauri::command]
pub async fn delete_job(state: State<'_, AppState>, id: i64) -> Res<()> {
    Ok(state.manager.db().delete_job(id).await?)
}

#[tauri::command]
pub fn cancel_shutdown(state: State<'_, AppState>) {
    if let Some(t) = state.shutdown.lock().unwrap().take() {
        t.cancel();
    }
}

// ---------------------------------------------------------------------------------------------
// Website grabber

#[tauri::command]
pub async fn start_grab(state: State<'_, AppState>, grab_id: String, opts: GrabOptions) -> Res<GrabResult> {
    let token = CancellationToken::new();
    state.grabs.lock().unwrap().insert(grab_id.clone(), token.clone());
    let client = engine::build_client(&state.manager.config())?;
    let r = grabber::crawl(&client, &opts, &token).await;
    state.grabs.lock().unwrap().remove(&grab_id);
    Ok(r?)
}

#[tauri::command]
pub fn cancel_grab(state: State<'_, AppState>, grab_id: String) {
    if let Some(t) = state.grabs.lock().unwrap().remove(&grab_id) {
        t.cancel();
    }
}

// ---------------------------------------------------------------------------------------------
// Notifications

#[tauri::command]
pub async fn list_notifications(state: State<'_, AppState>) -> Res<Vec<Notification>> {
    Ok(state.manager.db().list_notifications(100).await?)
}

#[tauri::command]
pub async fn mark_notifications_read(state: State<'_, AppState>) -> Res<()> {
    Ok(state.manager.db().mark_notifications_read().await?)
}

// ---------------------------------------------------------------------------------------------
// Video / adaptive streaming (yt-dlp + FFmpeg)

/// Title and quality options for a video page or HLS/DASH URL. Add it with `add_download` + `media`.
#[tauri::command]
pub async fn probe_media(state: State<'_, AppState>, url: String) -> Res<MediaInfo> {
    Ok(state.manager.extract_media(&url).await?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaToolsInfo {
    available: bool,
    ytdlp: Option<String>,
    ffmpeg: Option<String>,
    error: Option<String>,
}

#[tauri::command]
pub async fn media_tools_info(state: State<'_, AppState>) -> Res<MediaToolsInfo> {
    let t = match state.manager.media_tools() {
        Ok(t) => t,
        Err(e) => return Ok(MediaToolsInfo { available: false, ytdlp: None, ffmpeg: None, error: Some(e.to_string()) }),
    };
    Ok(match t.versions().await {
        Ok((y, f)) => MediaToolsInfo { available: true, ytdlp: Some(y), ffmpeg: Some(f), error: None },
        Err(e) => MediaToolsInfo { available: false, ytdlp: None, ffmpeg: None, error: Some(e.to_string()) },
    })
}

/// Sites change often; yt-dlp must be kept current. Updates the bundled copy in place.
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle, state: State<'_, AppState>) -> Res<String> {
    // Per-user copy: the installed one in Program Files is not writable without admin rights.
    let dir = app.path().app_local_data_dir().map_err(|e| err("io", e.to_string()))?.join("tools");
    // Linux/macOS have no bundled copy: fetch yt-dlp itself when it is missing.
    #[cfg(not(windows))]
    if state.manager.media_tools().is_err() {
        gale_core::media::install_ytdlp(&dir).await?;
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let tools = gale_core::media::MediaTools::locate(exe_dir.as_deref(), Some(&dir))?;
        state.manager.set_media_tools(Some(tools));
        return Ok("yt-dlp installed".into());
    }
    let mut tools = state.manager.media_tools()?;
    let (out, path) = tools.update_ytdlp(&dir).await?;
    tools.ytdlp = path;
    state.manager.set_media_tools(Some(tools));
    Ok(out.trim().to_string())
}

// ---------------------------------------------------------------------------------------------
// AI models (Hugging Face)

fn hf_client() -> Res<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("Gale/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| err("network", e.to_string()))
}

#[tauri::command]
pub async fn hf_search(state: State<'_, AppState>, query: String) -> Res<Vec<gale_core::hf::SearchHit>> {
    let q = query.trim();
    if q.is_empty() || q.len() > 200 {
        return Ok(vec![]);
    }
    let s = state.settings();
    Ok(gale_core::hf::search(&hf_client()?, &s.hf_endpoint, q, settings::hf_token().as_deref()).await?)
}

/// `input`: a model name (org/name) or any huggingface.co link to it.
#[tauri::command]
pub async fn hf_list(state: State<'_, AppState>, input: String) -> Res<gale_core::hf::ModelInfo> {
    let (repo, rev) = gale_core::hf::parse_repo(&input).ok_or_else(|| err("invalid_model", "Type a model name like Qwen/Qwen3-8B-GGUF, or paste its link."))?;
    let s = state.settings();
    Ok(gale_core::hf::list(&hf_client()?, &s.hf_endpoint, &repo, &rev, settings::hf_token().as_deref()).await?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFolders {
    /// <download folder>/Models
    downloads: String,
    /// LM Studio's models folder, when LM Studio is installed.
    lm_studio: Option<String>,
}

#[tauri::command]
pub async fn hf_folders(state: State<'_, AppState>) -> Res<ModelFolders> {
    let s = state.settings();
    Ok(ModelFolders {
        downloads: Path::new(&s.download_dir).join("Models").to_string_lossy().into_owned(),
        lm_studio: gale_core::hf::lm_studio_dir().map(|p| p.to_string_lossy().into_owned()),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAdded {
    added: usize,
    /// Already in the folder with the right size.
    skipped: usize,
}

/// Adds the chosen files. They go to <folder>/<org>/<name>/<path in the model>, the layout LM Studio expects, and
/// each large file is checked against the SHA-256 Hugging Face lists for it.
#[tauri::command]
pub async fn hf_download(state: State<'_, AppState>, repo: String, revision: String, paths: Vec<String>, folder: String) -> Res<ModelAdded> {
    let (repo, _) = gale_core::hf::parse_repo(&repo).ok_or_else(|| err("invalid_model", "unknown model"))?;
    if !Path::new(&folder).is_absolute() {
        return Err(err("invalid_folder", "choose a full folder path"));
    }
    let s = state.settings();
    // Sizes and checksums come from Hugging Face again, not from the page.
    let info = gale_core::hf::list(&hf_client()?, &s.hf_endpoint, &repo, &revision, settings::hf_token().as_deref()).await?;
    let base = repo.split('/').fold(PathBuf::from(&folder), |p, part| p.join(part));
    let (mut added, mut skipped) = (0, 0);
    for f in info.files.iter().filter(|f| paths.contains(&f.path)) {
        let rel = gale_core::hf::safe_relative(&f.path).ok_or_else(|| err("invalid_model", format!("unsafe file name {}", f.path)))?;
        let dest = base.join(&rel);
        if std::fs::metadata(&dest).is_ok_and(|m| m.len() == f.size) {
            skipped += 1;
            continue;
        }
        let opts = AddOptions {
            url: gale_core::hf::file_url(&s.hf_endpoint, &repo, &info.revision, &f.path),
            save_dir: dest.parent().unwrap_or(&base).to_string_lossy().into_owned(),
            file_name: dest.file_name().map(|n| n.to_string_lossy().into_owned()),
            expected_sha256: f.sha256.clone(),
            start: true,
            source: None, // "manual": the source column only allows the original values
            ..Default::default()
        };
        state.manager.add(opts).await?;
        added += 1;
    }
    Ok(ModelAdded { added, skipped })
}

/// The token goes straight to the system keychain; it never touches SQLite or logs.
#[tauri::command]
pub async fn set_hf_token(state: State<'_, AppState>, token: Option<String>) -> Res<()> {
    settings::set_hf_token(token.as_deref()).map_err(|e| err("credentials", e))?;
    state.manager.set_hf_token(settings::hf_token());
    Ok(())
}
