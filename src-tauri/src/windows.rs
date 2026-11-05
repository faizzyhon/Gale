//! IDM-style secondary windows: one progress window per download, and the "Download File Info"
//! pop-up for browser downloads. They load the same frontend, routed by URL hash.

use gale_core::db::DownloadStatus;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::AppState;

pub fn download_label(id: i64) -> String {
    format!("download-{id}")
}

fn open(app: &AppHandle, label: &str, hash: &str, title: &str, size: (f64, f64), on_top: bool) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App(format!("index.html#{hash}").into()))
        .title(title)
        .inner_size(size.0, size.1)
        .min_inner_size(size.0, 220.0)
        .resizable(false)
        .maximizable(false)
        .decorations(false) // custom title bar; Windows 11 still draws the rounded frame and shadow
        .shadow(true)
        .always_on_top(on_top)
        .center()
        .focused(true)
        .visible(false) // the page shows itself once rendered, so it never flashes white
        .build();
    if let Err(e) = built {
        tracing::warn!(error = %e, label, "could not open window");
    }
}

/// Opens the progress window now if the download is already running, otherwise as soon as it starts.
pub async fn want_progress_window(app: &AppHandle, id: i64) {
    let st = app.state::<AppState>();
    if !st.settings().show_progress_window {
        return;
    }
    st.window_wanted.lock().unwrap().insert(id);
    if let Ok(d) = st.manager.db().get_download(id).await {
        if d.status.is_active() || d.status == DownloadStatus::Queued {
            open_progress_if_wanted(app, id);
        }
    }
}

/// Called from download events; opens at most once per user action (closing the window sticks).
pub fn open_progress_if_wanted(app: &AppHandle, id: i64) {
    let st = app.state::<AppState>();
    if !st.window_wanted.lock().unwrap().remove(&id) {
        return;
    }
    let app = app.clone();
    // Window creation must not run inside the event callback on Windows (it can deadlock).
    tauri::async_runtime::spawn(async move {
        open(&app, &download_label(id), &format!("/download/{id}"), "Gale download", (520.0, 330.0), false);
    });
}

pub fn open_capture(app: &AppHandle, pending_id: &str) {
    let label = format!("capture-{}", &pending_id[..8.min(pending_id.len())]);
    open(app, &label, &format!("/capture/{pending_id}"), "Download File Info", (560.0, 380.0), true);
}
