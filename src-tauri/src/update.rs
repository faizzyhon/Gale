//! Checks GitHub for a newer Gale and, on Windows, installs it. The MSI's MajorUpgrade removes the old version
//! and installs the new one in one step, keeping settings and the download list.

use serde::Serialize;
use sha2::{Digest, Sha256};

const API: &str = "https://api.github.com/repos/faizzyhon/Gale/releases/latest";
/// Assets are only ever taken from this repository's own releases.
const ASSET_PREFIX: &str = "https://github.com/faizzyhon/Gale/releases/download/";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    pub notes: String,
    pub page: String,
    /// The installer for this platform, when the release has one.
    pub installer: Option<String>,
    /// True when Gale can install it itself (Windows MSI build); otherwise the page is opened.
    pub can_install: bool,
}

/// "v0.10.2" → (0, 10, 2). Pre-release tags never count as newer.
fn version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    if v.contains('-') {
        return None;
    }
    let mut p = v.split('.').map(|x| x.parse::<u64>().ok());
    Some((p.next()??, p.next().flatten().unwrap_or(0), p.next().flatten().unwrap_or(0)))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((version(latest), version(current)), (Some(l), Some(c)) if l > c)
}

/// The asset name this build installs from.
fn installer_name(v: &str) -> String {
    let v = v.trim_start_matches('v');
    if cfg!(target_os = "windows") {
        format!("Gale_{v}_x64.msi")
    } else if cfg!(target_os = "macos") {
        format!("Gale_{v}_universal.dmg")
    } else {
        format!("Gale_{v}_amd64.deb")
    }
}

fn client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("Gale/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(600))
        .build()
}

pub async fn check() -> Result<UpdateInfo, String> {
    let r: serde_json::Value = client()
        .map_err(|e| e.to_string())?
        .get(API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("could not reach GitHub: {e}"))?
        .bytes()
        .await
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))?;
    let tag = r["tag_name"].as_str().unwrap_or_default().to_string();
    let current = env!("CARGO_PKG_VERSION").to_string();
    let want = installer_name(&tag);
    let installer = r["assets"].as_array().into_iter().flatten().find_map(|a| {
        let url = a["browser_download_url"].as_str()?;
        (a["name"].as_str() == Some(want.as_str()) && url.starts_with(ASSET_PREFIX)).then(|| url.to_string())
    });
    Ok(UpdateInfo {
        newer: is_newer(&tag, &current),
        latest: tag.trim_start_matches('v').to_string(),
        current,
        notes: r["body"].as_str().unwrap_or_default().chars().take(4000).collect(),
        page: r["html_url"].as_str().filter(|u| u.starts_with("https://github.com/faizzyhon/Gale/")).unwrap_or("https://github.com/faizzyhon/Gale/releases").to_string(),
        can_install: cfg!(target_os = "windows") && installer.is_some() && !crate::startup::is_packaged(),
        installer,
    })
}

/// `"<hex>  name"` (sha256sum) or certutil-style output → the lowercase hash.
pub fn parse_sha256(text: &str) -> Option<String> {
    text.split_whitespace().map(|t| t.to_ascii_lowercase()).find(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Downloads the installer and its .sha256 file from the release, checks them against each other, and returns the
/// installer's path. Nothing is run unless the hash matches.
pub async fn download(installer: &str) -> Result<std::path::PathBuf, String> {
    if !installer.starts_with(ASSET_PREFIX) || installer.contains("..") {
        return Err("refusing an installer that is not from Gale's GitHub releases".into());
    }
    let c = client().map_err(|e| e.to_string())?;
    let get = |u: String| {
        let c = c.clone();
        async move { c.get(u).send().await.and_then(|r| r.error_for_status()).map_err(|e| format!("download failed: {e}"))?.bytes().await.map_err(|e| e.to_string()) }
    };
    let expected = parse_sha256(&String::from_utf8_lossy(&get(format!("{installer}.sha256")).await?)).ok_or("the release has no valid checksum file")?;
    let bytes = get(installer.to_string()).await?;
    if hex::encode(Sha256::digest(&bytes)) != expected {
        return Err("the downloaded installer does not match its checksum; nothing was installed".into());
    }
    let dir = std::env::temp_dir().join("gale-update");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(installer.rsplit('/').next().unwrap_or("Gale.msi"));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Runs the MSI and, when it succeeds, starts the new Gale as the signed-in user (not elevated). The MSI closes the
/// running Gale itself. Windows asks for administrator approval once, because Gale is installed for all users.
#[cfg(windows)]
pub fn launch(msi: &std::path::Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let (msi, exe) = (msi.display().to_string(), exe.display().to_string());
    // Both paths are ours (temp folder and install folder); Windows paths cannot contain quotes.
    if msi.contains('"') || exe.contains('"') {
        return Err("unexpected characters in a path".into());
    }
    std::process::Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .raw_arg(format!("/S /C \"msiexec /i \"{msi}\" /passive && start \"\" \"{exe}\"\""))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn launch(_: &std::path::Path) -> Result<(), String> {
    Err("automatic install is only available on Windows; download the new version from the release page".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("v0.6.0", "0.5.0"));
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("v0.5.0", "0.5.0"));
        assert!(!is_newer("v0.4.9", "0.5.0"));
        assert!(!is_newer("v0.7.0-beta.1", "0.5.0"));
        assert!(!is_newer("garbage", "0.5.0"));
    }

    #[test]
    fn checksums() {
        let h = "6127f2228d024289582fabdb3a161f2a73d16cfc0bb3cb7156ebb4d1b51df6e8";
        assert_eq!(parse_sha256(&format!("{h}  dist-installer/Gale_0.4.0_x64.msi\n")).as_deref(), Some(h));
        assert_eq!(parse_sha256(&format!("SHA256 hash of x:\r\n{}\r\nCertUtil: done", h.to_uppercase())).as_deref(), Some(h));
        assert_eq!(parse_sha256("nothing here"), None);
    }

    #[test]
    fn only_this_repository() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let e = rt.block_on(download("https://evil.example/Gale_9.9.9_x64.msi")).unwrap_err();
        assert!(e.contains("not from Gale"));
    }
}
