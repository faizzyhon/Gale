mod commands;
mod native_messaging;
mod settings;
mod system;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gale_core::db::{Db, Download, DownloadStatus};
use gale_core::engine::{RequestHeaders, Tick};
use gale_core::limits::{HostLimiter, RateLimiter};
use gale_core::manager::{EventSink, Manager};
use gale_core::{ipc, protocol};
use settings::AppSettings;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager as _, WindowEvent};
use tauri_plugin_notification::NotificationExt;
use tokio_util::sync::CancellationToken;

/// Browser download waiting for the user to confirm in the Add dialog. Headers never reach the webview.
pub struct PendingAdd {
    pub headers: RequestHeaders,
    created: Instant,
}

impl PendingAdd {
    pub fn new(headers: RequestHeaders) -> Self {
        Self { headers, created: Instant::now() }
    }
}

pub struct AppState {
    pub manager: Arc<Manager>,
    settings: Mutex<AppSettings>,
    pub global: Arc<RateLimiter>,
    pub hosts: Arc<HostLimiter>,
    pub pending: Mutex<HashMap<String, PendingAdd>>,
    pub grabs: Mutex<HashMap<String, CancellationToken>>,
    pub shutdown: Mutex<Option<CancellationToken>>,
    quitting: AtomicBool,
}

impl AppState {
    pub fn settings(&self) -> AppSettings {
        self.settings.lock().unwrap().clone()
    }
    pub fn set_settings(&self, s: AppSettings) {
        *self.settings.lock().unwrap() = s;
    }
    pub fn take_pending(&self, id: &str) -> Option<RequestHeaders> {
        let mut p = self.pending.lock().unwrap();
        p.retain(|_, v| v.created.elapsed() < Duration::from_secs(30 * 60));
        p.remove(id).map(|v| v.headers)
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Bridges engine events to the webview and to Windows. Payloads never contain request headers.
struct TauriEvents(AppHandle);

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent<'a> {
    id: i64,
    written: u64,
    total: Option<u64>,
    bytes_per_sec: f64,
    active_connections: u32,
    segments: &'a [gale_core::engine::SegmentState],
}

impl EventSink for TauriEvents {
    fn progress(&self, id: i64, t: &Tick) {
        let ev = ProgressEvent { id, written: t.written, total: t.total, bytes_per_sec: t.bytes_per_sec, active_connections: t.active_connections, segments: &t.segments };
        if let Err(e) = self.0.emit("download://progress", ev) {
            tracing::warn!(error = %e, "emit progress failed");
        }
    }

    fn changed(&self, d: &Download) {
        if let Err(e) = self.0.emit("download://changed", d) {
            tracing::warn!(error = %e, "emit changed failed");
        }
    }

    fn finished(&self, d: &Download) {
        let (app, d) = (self.0.clone(), d.clone());
        tauri::async_runtime::spawn(async move { after_finish(app, d).await });
    }

    fn queue_finished(&self, queue_id: i64, shutdown: bool) {
        let app = self.0.clone();
        tauri::async_runtime::spawn(async move {
            let Some(st) = app.try_state::<AppState>() else { return };
            let name = st.manager.db().list_queues().await.ok().and_then(|qs| qs.into_iter().find(|q| q.id == queue_id)).map(|q| q.name);
            notify(&app, "info", &format!("Queue \"{}\" finished", name.unwrap_or_default()), None, None).await;
            if shutdown {
                start_shutdown_countdown(&app);
            }
        });
    }
}

async fn after_finish(app: AppHandle, d: Download) {
    let Some(st) = app.try_state::<AppState>() else { return };
    let s = st.settings();
    if d.status == DownloadStatus::Failed {
        notify(&app, "error", &format!("Download failed: {}", d.file_name), d.error_message.as_deref(), Some(d.id)).await;
        return;
    }
    let Some(path) = d.final_path.as_deref().map(std::path::PathBuf::from) else { return };
    if s.mark_of_the_web {
        if let Err(e) = system::mark_of_the_web(&path, d.final_url.as_deref().unwrap_or(&d.url), d.referrer.as_deref()) {
            tracing::warn!(error = %e, "could not write Zone.Identifier");
        }
    }
    if s.defender_scan {
        match system::defender_scan(&path).await {
            system::ScanResult::Clean => {}
            system::ScanResult::Threat => {
                notify(&app, "error", &format!("Threat found in {}", d.file_name), Some("Microsoft Defender has handled the file."), Some(d.id)).await;
                return;
            }
            system::ScanResult::Unavailable(why) => tracing::warn!(why, "virus scan skipped"),
        }
    }
    notify(&app, "success", &format!("Downloaded {}", d.file_name), None, Some(d.id)).await;
}

async fn notify(app: &AppHandle, level: &str, title: &str, body: Option<&str>, download_id: Option<i64>) {
    let Some(st) = app.try_state::<AppState>() else { return };
    if let Err(e) = st.manager.db().add_notification(level, title, body, download_id).await {
        tracing::warn!(error = %e, "could not store notification");
    }
    let _ = app.emit("notification://new", ());
    if st.settings().notifications {
        let mut n = app.notification().builder().title(title);
        if let Some(b) = body {
            n = n.body(b);
        }
        if let Err(e) = n.show() {
            tracing::warn!(error = %e, "toast failed");
        }
    }
}

/// 60 s visible, cancellable countdown before shutting Windows down.
fn start_shutdown_countdown(app: &AppHandle) {
    let Some(st) = app.try_state::<AppState>() else { return };
    let token = CancellationToken::new();
    if let Some(old) = st.shutdown.lock().unwrap().replace(token.clone()) {
        old.cancel();
    }
    show_main(app);
    let _ = app.emit("system://shutdown-countdown", 60);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::select! {
            _ = token.cancelled() => { let _ = app.emit("system://shutdown-cancelled", ()); }
            _ = tokio::time::sleep(Duration::from_secs(60)) => {
                let m = app.state::<AppState>().manager.clone();
                m.shutdown().await; // persist and stop transfers first
                if let Err(e) = system::shutdown_windows() {
                    tracing::error!(error = %e, "shutdown failed");
                }
            }
        }
    });
}

fn init_logging(app: &AppHandle) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::EnvFilter;
    let dir = app.path().app_log_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    let (writer, guard) = tracing_appender::non_blocking(tracing_appender::rolling::daily(dir, "gale.log"));
    tracing_subscriber::fmt()
        .json()
        .with_writer(writer)
        .with_env_filter(EnvFilter::try_from_env("GALE_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .try_init()
        .ok()?;
    Some(guard)
}

/// A restore chosen in Settings → Advanced is swapped in before the database opens.
fn apply_pending_restore(db_path: &std::path::Path) {
    let restore = db_path.with_extension("db.restore");
    if restore.exists() {
        let _ = std::fs::rename(db_path, db_path.with_extension("db.before-restore"));
        for ext in ["db-wal", "db-shm"] {
            let _ = std::fs::remove_file(db_path.with_extension(ext));
        }
        match std::fs::rename(&restore, db_path) {
            Ok(()) => tracing::info!("database restored from backup"),
            Err(e) => tracing::error!(error = %e, "database restore failed"),
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Gale", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause_all", "Pause all downloads", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Gale", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &pause, &PredefinedMenuItem::separator(app)?, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundled icon"))
        .tooltip("Gale")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "show" => show_main(app),
            "pause_all" => {
                let m = app.state::<AppState>().manager.clone();
                tauri::async_runtime::spawn(async move { m.shutdown().await });
            }
            "quit" => {
                app.state::<AppState>().quitting.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Removes HKCU\\…\\Run\\Gale (written by the autostart plugin). Missing value is fine.
pub fn uninstall_cleanup() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Registry::{RegDeleteKeyValueW, HKEY_CURRENT_USER};
        let w = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (key, name) = (w(r"Software\Microsoft\Windows\CurrentVersion\Run"), w("Gale"));
        // SAFETY: both strings are NUL-terminated UTF-16 buffers alive for the call.
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
    }
}

pub fn run() {
    let background = std::env::args().any(|a| a == "--background");
    tauri::Builder::default()
        // Must be first: a second launch focuses the running window instead of starting another engine.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !args.iter().any(|a| a == "--background") {
                show_main(app);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .setup(move |app| {
            let handle = app.handle().clone();
            if let Some(guard) = init_logging(&handle) {
                app.manage(guard); // keep the log writer alive for the app's lifetime
            }
            let db_path = handle.path().app_data_dir()?.join("gale.db");
            apply_pending_restore(&db_path);
            let (global, hosts) = (RateLimiter::new(0), HostLimiter::new(0));
            let (manager, s) = tauri::async_runtime::block_on(async {
                let db = Db::open(&db_path).await?;
                let recovered = db.recover_interrupted().await?;
                tracing::info!(recovered, "database ready");
                let s = settings::load(&db, &handle).await;
                let cfg = s.engine_config(&global, &hosts);
                let m = Manager::new(db, cfg, s.max_simultaneous as usize, Arc::new(TauriEvents(handle.clone())))?;
                Ok::<_, gale_core::Error>((m, s))
            })?;
            tauri::async_runtime::spawn({
                let m = manager.clone();
                async move { m.spawn_background() }
            });
            app.manage(AppState {
                manager,
                settings: Mutex::new(s),
                global,
                hosts,
                pending: Mutex::new(HashMap::new()),
                grabs: Mutex::new(HashMap::new()),
                shutdown: Mutex::new(None),
                quitting: AtomicBool::new(false),
            });

            // Browser bridge: per-session token in the user's private folder + local-only pipe.
            if let Some(dir) = ipc::app_local_dir() {
                match ipc::write_token(&dir) {
                    Ok(token) => {
                        let h = handle.clone();
                        let handler: ipc::Handler = Arc::new(move |env: protocol::Envelope| {
                            let h = h.clone();
                            Box::pin(native_messaging::handle(h, env))
                        });
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = ipc::serve(ipc::endpoint(&dir), token, handler).await {
                                tracing::error!(error = %e, "browser bridge unavailable");
                            }
                        });
                    }
                    Err(e) => tracing::error!(error = %e, "could not create IPC token"),
                }
            }

            // Keep the startup entry in sync with settings (an upgrade or a moved install may have dropped it).
            {
                use tauri_plugin_autostart::ManagerExt;
                let want = app.state::<AppState>().settings().start_with_windows;
                let al = handle.autolaunch();
                if want && !al.is_enabled().unwrap_or(false) {
                    let _ = al.enable();
                }
            }
            build_tray(&handle)?;
            if !background {
                show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let st = window.state::<AppState>();
                if st.settings().minimize_to_tray && !st.quitting.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_downloads,
            commands::add_download,
            commands::add_batch,
            commands::resume_download,
            commands::pause_download,
            commands::cancel_download,
            commands::restart_download,
            commands::remove_download,
            commands::recheck_download,
            commands::open_download,
            commands::reveal_download,
            commands::get_download_segments,
            commands::dashboard,
            commands::list_categories,
            commands::list_history,
            commands::get_settings,
            commands::save_settings,
            commands::set_proxy_password,
            commands::test_connection,
            commands::list_queues,
            commands::save_queue,
            commands::delete_queue,
            commands::queue_action,
            commands::queue_memberships,
            commands::set_download_queue,
            commands::list_jobs,
            commands::save_job,
            commands::delete_job,
            commands::start_grab,
            commands::cancel_grab,
            commands::list_notifications,
            commands::mark_notifications_read,
            commands::cancel_shutdown,
            commands::backup_database,
            commands::restore_database,
            commands::open_logs,
            commands::reset_settings,
            commands::browser_status,
            commands::quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Gale")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // Progress is persisted continuously; this just stops transfers cleanly.
                let m = app.state::<AppState>().manager.clone();
                tauri::async_runtime::block_on(m.shutdown());
            }
        });
}
