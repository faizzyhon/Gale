use std::sync::Arc;

use gale_core::db::Db;
use gale_core::engine::{EngineConfig, Limits, ProxyConfig};
use gale_core::limits::{HostLimiter, RateLimiter};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub const KEY: &str = "app";
const CRED_SERVICE: &str = "Gale Download Manager";
const CRED_PROXY: &str = "proxy";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    // General
    pub theme: Theme,
    pub download_dir: String,
    pub category_folders: bool,
    pub notifications: bool,
    pub start_with_windows: bool,
    pub minimize_to_tray: bool,
    pub show_progress_window: bool,
    pub close_window_when_done: bool,
    /// Resume downloads that were running when Gale closed, and retry after network errors.
    pub auto_resume: bool,
    // Downloads
    pub connections: u8,
    /// Applied at startup.
    pub max_simultaneous: u8,
    pub max_retries: u32,
    // Browser
    pub browser_enabled: bool,
    pub browser_excluded_domains: Vec<String>,
    pub browser_excluded_types: Vec<String>,
    // Network
    pub proxy_kind: ProxyKind,
    pub proxy_host: String,
    pub proxy_port: u16,
    pub proxy_user: String,
    pub connect_timeout_secs: u64,
    pub read_timeout_secs: u64,
    pub user_agent: String,
    pub global_limit_kbps: u64,
    pub per_download_limit_kbps: u64,
    pub per_host_connections: u32,
    // Security
    pub mark_of_the_web: bool,
    pub defender_scan: bool,
    pub compute_sha256: bool,
    pub blocked_extensions: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyKind {
    #[default]
    None,
    Http,
    Https,
    Socks5,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            download_dir: String::new(),
            category_folders: false,
            notifications: true,
            start_with_windows: true,
            minimize_to_tray: true,
            show_progress_window: true,
            close_window_when_done: false,
            auto_resume: true,
            connections: 8,
            max_simultaneous: 3,
            max_retries: 6,
            browser_enabled: true,
            browser_excluded_domains: vec![],
            browser_excluded_types: vec![],
            proxy_kind: ProxyKind::None,
            proxy_host: String::new(),
            proxy_port: 8080,
            proxy_user: String::new(),
            connect_timeout_secs: 15,
            read_timeout_secs: 30,
            user_agent: String::new(),
            global_limit_kbps: 0,
            per_download_limit_kbps: 0,
            per_host_connections: 0,
            mark_of_the_web: true,
            defender_scan: false,
            compute_sha256: false,
            blocked_extensions: vec![],
        }
    }
}

fn clean_list(v: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = v.into_iter().map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase()).filter(|s| !s.is_empty()).collect();
    out.sort();
    out.dedup();
    out
}

impl AppSettings {
    pub fn validated(mut self) -> Result<Self, String> {
        if !std::path::Path::new(&self.download_dir).is_absolute() {
            return Err("download folder must be a full path".into());
        }
        if ![1, 2, 4, 8, 16, 32].contains(&self.connections) {
            return Err("connections must be 1, 2, 4, 8, 16 or 32".into());
        }
        self.max_simultaneous = self.max_simultaneous.clamp(1, 32);
        self.max_retries = self.max_retries.min(20);
        self.connect_timeout_secs = self.connect_timeout_secs.clamp(3, 300);
        self.read_timeout_secs = self.read_timeout_secs.clamp(5, 600);
        self.per_host_connections = self.per_host_connections.min(64);
        if self.user_agent.len() > 512 || self.user_agent.contains(['\r', '\n']) {
            return Err("user agent is too long or has line breaks".into());
        }
        if self.proxy_kind != ProxyKind::None {
            let host_ok = !self.proxy_host.is_empty() && self.proxy_host.chars().all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c));
            if !host_ok || self.proxy_port == 0 {
                return Err("enter a valid proxy host and port".into());
            }
        }
        self.browser_excluded_domains = clean_list(self.browser_excluded_domains);
        self.browser_excluded_types = clean_list(self.browser_excluded_types);
        self.blocked_extensions = clean_list(self.blocked_extensions);
        Ok(self)
    }

    /// Engine configuration for these settings. The proxy password comes from Credential Manager.
    pub fn engine_config(&self, global: &Arc<RateLimiter>, hosts: &Arc<HostLimiter>) -> EngineConfig {
        global.set_rate(self.global_limit_kbps * 1024);
        hosts.set_limit(self.per_host_connections as usize);
        let base = EngineConfig::default();
        EngineConfig {
            connections: self.connections,
            max_retries: self.max_retries,
            user_agent: if self.user_agent.trim().is_empty() { base.user_agent.clone() } else { self.user_agent.clone() },
            connect_timeout: std::time::Duration::from_secs(self.connect_timeout_secs),
            read_timeout: std::time::Duration::from_secs(self.read_timeout_secs),
            compute_sha256: self.compute_sha256,
            proxy: self.proxy(),
            download_limit_bps: self.per_download_limit_kbps * 1024,
            limits: Limits::new(Some(global.clone()), Some(hosts.clone())),
            ..base
        }
    }

    fn proxy(&self) -> Option<ProxyConfig> {
        let scheme = match self.proxy_kind {
            ProxyKind::None => return None,
            ProxyKind::Http => "http",
            ProxyKind::Https => "https",
            ProxyKind::Socks5 => "socks5h", // resolve names through the proxy
        };
        let user = Some(self.proxy_user.clone()).filter(|u| !u.is_empty());
        let password = user.as_ref().and_then(|_| proxy_password());
        Some(ProxyConfig { url: format!("{scheme}://{}:{}", self.proxy_host, self.proxy_port), username: user, password })
    }
}

pub fn proxy_password() -> Option<String> {
    keyring::Entry::new(CRED_SERVICE, CRED_PROXY).ok()?.get_password().ok()
}

/// Stores (or with `None`, deletes) the proxy password in Windows Credential Manager.
pub fn set_proxy_password(pw: Option<&str>) -> Result<(), String> {
    let e = keyring::Entry::new(CRED_SERVICE, CRED_PROXY).map_err(|e| e.to_string())?;
    match pw {
        Some(p) if !p.is_empty() => e.set_password(p).map_err(|e| e.to_string()),
        _ => match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
    }
}

pub fn has_proxy_password() -> bool {
    proxy_password().is_some()
}

pub async fn load(db: &Db, app: &AppHandle) -> AppSettings {
    let mut s: AppSettings = db.get_setting(KEY).await.ok().flatten().unwrap_or_default();
    if s.download_dir.is_empty() {
        // Downloads folder, else ~/Downloads, else app data: never leave it empty.
        let p = app.path();
        let base = p
            .download_dir()
            .or_else(|_| p.home_dir().map(|h| h.join("Downloads")))
            .or_else(|_| p.app_data_dir().map(|d| d.join("Downloads")));
        s.download_dir = base.map(|d| d.join("Gale").to_string_lossy().into_owned()).unwrap_or_default();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_normalises_and_rejects() {
        let base = AppSettings { download_dir: if cfg!(windows) { r"C:\D".into() } else { "/d".into() }, ..Default::default() };
        let s = AppSettings { browser_excluded_types: vec![".EXE".into(), "exe".into(), " ".into()], ..base.clone() }.validated().unwrap();
        assert_eq!(s.browser_excluded_types, ["exe"]);
        assert!(AppSettings { connections: 3, ..base.clone() }.validated().is_err());
        assert!(AppSettings { proxy_kind: ProxyKind::Http, proxy_host: "a b".into(), ..base.clone() }.validated().is_err());
        assert!(AppSettings { user_agent: "x\r\nEvil: 1".into(), ..base.clone() }.validated().is_err());
        assert!(AppSettings { download_dir: "rel".into(), ..base }.validated().is_err());
    }

    #[test]
    fn engine_config_applies_limits() {
        let (g, h) = (RateLimiter::new(0), HostLimiter::new(0));
        let s = AppSettings { global_limit_kbps: 100, per_download_limit_kbps: 50, proxy_kind: ProxyKind::Socks5, proxy_host: "127.0.0.1".into(), proxy_port: 1080, ..Default::default() };
        let c = s.engine_config(&g, &h);
        assert_eq!(g.rate(), 100 * 1024);
        assert_eq!(c.download_limit_bps, 50 * 1024);
        assert_eq!(c.proxy.unwrap().url, "socks5h://127.0.0.1:1080");
    }
}
