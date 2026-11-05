//! BitTorrent downloads (magnet links and .torrent files) through librqbit.
//!
//! One session is shared by all torrents. Gale keeps no torrent state of its own on disk besides the
//! .torrent metadata in `download_metadata`; on resume the torrent is added again and librqbit rechecks the
//! pieces already on disk, so pause/resume survives restarts the same way HTTP downloads do.

use std::path::PathBuf;
use std::sync::Arc;

use librqbit::{Session, SessionOptions};

use crate::{Error, Result};

/// .torrent files bigger than this are refused (real ones are a few hundred KB).
pub const MAX_TORRENT_FILE: usize = 16 * 1024 * 1024;

pub fn is_magnet(input: &str) -> bool {
    input.trim_start().get(..8).is_some_and(|p| p.eq_ignore_ascii_case("magnet:?"))
}

/// A magnet link with a BitTorrent v1 or v2 info hash. Returns the trimmed link.
pub fn validate_magnet(input: &str) -> Result<String> {
    let m = input.trim();
    let lower = m.to_ascii_lowercase();
    if !is_magnet(m) || m.len() > 8192 || !(lower.contains("xt=urn:btih:") || lower.contains("xt=urn:btmh:")) {
        return Err(Error::InvalidUrl(input.chars().take(200).collect()));
    }
    Ok(m.to_string())
}

/// The display name (`dn=`) of a magnet link, if it has one.
pub fn magnet_name(magnet: &str) -> Option<String> {
    let query = magnet.split_once('?')?.1;
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == "dn")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// A `file:` URL of a local .torrent file, as the app's own file picker produces. The browser bridge never
/// accepts these (see `protocol::check_url`).
pub fn local_torrent(input: &str) -> Option<PathBuf> {
    let u = url::Url::parse(input.trim()).ok()?;
    let p = u.to_file_path().ok().filter(|_| u.scheme() == "file")?;
    (p.is_absolute() && is_torrent_name(&p.to_string_lossy())).then_some(p)
}

pub fn is_torrent_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".torrent")
}

/// Folder librqbit uses for torrents added without an explicit output folder. Gale always passes one, so
/// nothing is written here; it only serves to work out a torrent's own sub-folder name.
pub fn session_root() -> PathBuf {
    std::env::temp_dir().join("gale-torrent")
}

pub async fn new_session() -> Result<Arc<Session>> {
    let opts = |ipv4_only: bool| SessionOptions {
        // DHT finds peers for magnet links without trackers. Its routing table is not saved.
        dht: Some(librqbit::DhtSessionConfig { persistence: None, ..Default::default() }),
        persistence: None,
        fastresume: false,
        listen: Some(librqbit::ListenerOptions {
            listen_addr: if ipv4_only { (std::net::Ipv4Addr::UNSPECIFIED, 0).into() } else { (std::net::Ipv6Addr::UNSPECIFIED, 0).into() },
            ipv4_only,
            ..Default::default()
        }),
        ipv4_only,
        ..Default::default()
    };
    // IPv6 can be switched off on a PC; dual-stack sockets then fail to open.
    match Session::new_with_opts(session_root(), opts(false)).await {
        Ok(s) => Ok(s),
        Err(e) => {
            tracing::info!(error = %format!("{e:#}"), "BitTorrent: IPv6 unavailable, using IPv4 only");
            Session::new_with_opts(session_root(), opts(true)).await.map_err(|e| Error::Torrent(format!("could not start BitTorrent: {e:#}")))
        }
    }
}

pub fn err(e: anyhow::Error) -> Error {
    Error::Torrent(format!("{e:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnet_validation() {
        let ok = "magnet:?xt=urn:btih:c9e15763f722f23e98a29decdfae341b98d53056&dn=Cosmos+Laundromat&tr=udp%3A%2F%2Ft";
        assert_eq!(validate_magnet(&format!("  {ok} ")).unwrap(), ok);
        assert_eq!(magnet_name(ok).as_deref(), Some("Cosmos Laundromat"));
        assert!(validate_magnet("MAGNET:?XT=URN:BTIH:abc").is_ok());
        assert!(validate_magnet("magnet:?dn=no-hash").is_err());
        assert!(validate_magnet("https://example.com/x.torrent").is_err());
        assert!(validate_magnet(&format!("magnet:?xt=urn:btih:{}", "a".repeat(9000))).is_err());
        assert_eq!(magnet_name("magnet:?xt=urn:btih:abc"), None);
        assert!(is_torrent_name("Ubuntu.ISO.Torrent"));
        #[cfg(unix)]
        assert_eq!(local_torrent("file:///home/a/b%20c.torrent"), Some(PathBuf::from("/home/a/b c.torrent")));
        assert_eq!(local_torrent("file:///home/a/notes.txt"), None);
        assert_eq!(local_torrent("https://x.org/a.torrent"), None);
    }
}
