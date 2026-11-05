//! Gale core: download engine, persistence and security. No UI or Tauri dependency.
// A std MutexGuard held across `.await` deadlocked the manager once; keep it a hard error.
#![deny(clippy::await_holding_lock)]

pub mod categories;
pub mod db;
pub mod engine;
pub mod grabber;
pub mod ipc;
pub mod limits;
pub mod manager;
pub mod media;
pub mod protocol;
pub mod queue;
pub mod security;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid or unsupported URL: {0}")]
    InvalidUrl(String),
    #[error("unsafe file path: {0}")]
    UnsafePath(String),
    #[error("server returned HTTP {status}")]
    Http { status: u16 },
    #[error("the download link has expired or access was revoked (HTTP {status})")]
    UrlExpired { status: u16 },
    #[error("the file on the server changed since the download started")]
    RemoteChanged,
    #[error("server response did not match the requested byte range")]
    RangeMismatch,
    #[error("received {got} bytes but expected {expected}")]
    SizeMismatch { got: u64, expected: u64 },
    #[error("SHA-256 mismatch: expected {expected}, got {got}")]
    ChecksumMismatch { expected: String, got: String },
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("disk error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("download {0} not found")]
    NotFound(i64),
    #[error("{0}")]
    InvalidState(String),
    #[error("{0}")]
    Media(String),
    #[error("{0} was not found; reinstall Gale or install it and add it to PATH")]
    ToolMissing(String),
}

impl Error {
    /// Stable machine-readable code stored in `downloads.error_code` and sent to the UI.
    pub fn code(&self) -> &'static str {
        match self {
            Error::InvalidUrl(_) => "invalid_url",
            Error::UnsafePath(_) => "unsafe_path",
            Error::Http { .. } => "http_error",
            Error::UrlExpired { .. } => "url_expired",
            Error::RemoteChanged => "remote_changed",
            Error::RangeMismatch => "range_mismatch",
            Error::SizeMismatch { .. } => "size_mismatch",
            Error::ChecksumMismatch { .. } => "checksum_mismatch",
            Error::Network(_) => "network",
            Error::Io(_) => "io",
            Error::Db(_) | Error::Migrate(_) => "database",
            Error::NotFound(_) => "not_found",
            Error::InvalidState(_) => "invalid_state",
            Error::Media(_) => "media",
            Error::ToolMissing(_) => "tool_missing",
        }
    }

    /// Whether a fresh attempt of the same request could succeed.
    pub fn is_transient(&self) -> bool {
        match self {
            Error::Network(_) => true,
            Error::Http { status } => matches!(status, 408 | 429 | 500..=599),
            Error::Io(e) => matches!(
                e.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::TimedOut
            ),
            _ => false,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
