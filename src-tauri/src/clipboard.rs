//! Optional clipboard watch: when a download link is copied anywhere, Gale offers to download it.
//! Nothing is read while the setting is off; text that was already on the clipboard when it was turned on is ignored.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

/// File types worth offering. Web pages and other links are left alone so copying text stays quiet.
const EXTENSIONS: &[&str] = &[
    "zip", "7z", "rar", "tar", "gz", "xz", "bz2", "zst", "iso", "img", "exe", "msi", "msix", "dmg", "pkg", "deb", "rpm", "apk",
    "appimage", "mp4", "mkv", "webm", "avi", "mov", "mp3", "m4a", "flac", "wav", "ogg", "opus", "pdf", "epub", "docx", "xlsx",
    "pptx", "torrent", "gguf", "safetensors", "bin", "jar",
];

/// The copied text when it is exactly one download link (a magnet, or an address ending in a known file type).
pub fn download_link(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.len() > 4096 || t.split_whitespace().count() != 1 {
        return None;
    }
    if t.len() > 8 && t[..8].eq_ignore_ascii_case("magnet:?") {
        return Some(t.to_string());
    }
    let u = url::Url::parse(t).ok().filter(|u| matches!(u.scheme(), "http" | "https"))?;
    let last = u.path_segments()?.next_back()?.to_ascii_lowercase();
    let ext = last.rsplit_once('.')?.1;
    EXTENSIONS.contains(&ext).then(|| t.to_string())
}

pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut board: Option<arboard::Clipboard> = None;
        let mut last: Option<String> = None;
        loop {
            std::thread::sleep(Duration::from_millis(800));
            if !app.state::<AppState>().settings().clipboard_watch {
                (board, last) = (None, None); // let go of the clipboard and forget what was on it
                continue;
            }
            if board.is_none() {
                board = arboard::Clipboard::new().ok();
            }
            let Some(cb) = board.as_mut() else { continue };
            let text = cb.get_text().unwrap_or_default();
            let first = last.is_none();
            if last.as_deref() == Some(text.as_str()) {
                continue;
            }
            last = Some(text.clone());
            if first {
                continue; // already there when watching started
            }
            if let Some(link) = download_link(&text) {
                crate::show_main(&app);
                let _ = app.emit("clipboard://link", link);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::download_link;

    #[test]
    fn only_single_download_links() {
        assert!(download_link(" https://example.com/files/setup.exe ").is_some());
        assert!(download_link("https://cdn.example.org/a/b/Movie.MKV?token=1").is_some());
        assert!(download_link("magnet:?xt=urn:btih:abc").is_some());
        assert!(download_link("https://huggingface.co/o/m/resolve/main/model-Q4_K_M.gguf").is_some());
        assert!(download_link("https://example.com/article").is_none());
        assert!(download_link("https://example.com/page.html").is_none());
        assert!(download_link("see https://example.com/a.zip").is_none());
        assert!(download_link("ftp://example.com/a.zip").is_none());
        assert!(download_link("").is_none());
    }
}
