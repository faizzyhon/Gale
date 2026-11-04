//! Local IPC between the native messaging host and the running app.
//! Windows: a per-user named pipe that rejects remote clients. Elsewhere (dev/tests): a Unix socket.
//! Every message carries a random token stored in the user's private app folder.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use crate::protocol::{read_frame, token_eq, write_frame, Envelope, Response};
use crate::{Error, Result};

pub const APP_ID: &str = "com.faizzyhon.gale";

/// `%LOCALAPPDATA%\com.faizzyhon.gale` — the same folder Tauri uses for app_local_data_dir.
pub fn app_local_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join(APP_ID))
}

pub fn token_path(dir: &Path) -> PathBuf {
    dir.join("ipc.token")
}

#[cfg(windows)]
pub fn endpoint(_dir: &Path) -> String {
    let user: String = std::env::var("USERNAME").unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    format!(r"\\.\pipe\gale-ipc-{user}")
}

/// Dev/test only. Socket paths are limited to ~108 bytes, so use a short per-folder name in the temp dir.
#[cfg(not(windows))]
pub fn endpoint(dir: &Path) -> String {
    use sha2::Digest;
    let tag = hex::encode(&sha2::Sha256::digest(dir.to_string_lossy().as_bytes())[..6]);
    std::env::temp_dir().join(format!("gale-{tag}.sock")).to_string_lossy().into_owned()
}

/// Create a fresh token for this app session; only the current user can read it.
pub fn write_token(dir: &Path) -> Result<String> {
    std::fs::create_dir_all(dir)?;
    let token = hex::encode(rand::random::<[u8; 32]>());
    let path = token_path(dir);
    std::fs::write(&path, &token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(token)
}

pub type Handler = Arc<dyn Fn(Envelope) -> Pin<Box<dyn Future<Output = Response> + Send>> + Send + Sync>;

async fn handle_conn<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(mut s: S, token: Arc<String>, handler: Handler) {
    while let Ok(Some(frame)) = read_frame(&mut s).await {
        let resp = match serde_json::from_slice::<Envelope>(&frame) {
            Ok(env) if !token_eq(&env.token, &token) => {
                tracing::warn!("IPC message with a bad token rejected");
                Response::err(&env.request.id, "unauthorized", "not authorized")
            }
            Ok(env) => match env.request.validate() {
                Ok(()) => handler(env).await,
                Err(e) => Response::err(&env.request.id, "invalid", e.to_string()),
            },
            Err(_) => Response::err("", "invalid", "malformed envelope"),
        };
        let Ok(body) = serde_json::to_vec(&resp) else { break };
        if write_frame(&mut s, &body).await.is_err() {
            break;
        }
    }
}

/// Accept connections forever. Returns only on a setup error (e.g. another instance owns the pipe).
pub async fn serve(endpoint: String, token: String, handler: Handler) -> Result<()> {
    let token = Arc::new(token);
    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ServerOptions;
        // first_pipe_instance: fail instead of joining a pipe someone else created first.
        let mut server = ServerOptions::new().first_pipe_instance(true).reject_remote_clients(true).create(&endpoint)?;
        loop {
            server.connect().await?;
            let conn = server;
            server = ServerOptions::new().reject_remote_clients(true).create(&endpoint)?;
            tokio::spawn(handle_conn(conn, token.clone(), handler.clone()));
        }
    }
    #[cfg(not(windows))]
    {
        let _ = std::fs::remove_file(&endpoint);
        let listener = tokio::net::UnixListener::bind(&endpoint)?;
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&endpoint, std::fs::Permissions::from_mode(0o600))?;
        }
        loop {
            let (conn, _) = listener.accept().await?;
            tokio::spawn(handle_conn(conn, token.clone(), handler.clone()));
        }
    }
}

#[cfg(windows)]
pub type Stream = tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(not(windows))]
pub type Stream = tokio::net::UnixStream;

pub async fn connect(endpoint: &str) -> Result<Stream> {
    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ClientOptions;
        const ERROR_PIPE_BUSY: i32 = 231;
        for _ in 0..20 {
            match ClientOptions::new().open(endpoint) {
                Ok(c) => return Ok(c),
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
                Err(e) => return Err(e.into()),
            }
        }
        Err(Error::InvalidState("app is busy".into()))
    }
    #[cfg(not(windows))]
    {
        Ok(tokio::net::UnixStream::connect(endpoint).await?)
    }
}

/// One request/response over an open connection.
pub async fn call(stream: &mut Stream, env: &Envelope) -> Result<Response> {
    let body = serde_json::to_vec(env).map_err(|e| Error::InvalidState(e.to_string()))?;
    write_frame(stream, &body).await?;
    let frame = read_frame(stream).await?.ok_or_else(|| Error::InvalidState("app closed the connection".into()))?;
    serde_json::from_slice(&frame).map_err(|e| Error::InvalidState(format!("bad response from app: {e}")))
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use crate::protocol::{Command, Request};

    #[tokio::test]
    async fn authenticated_roundtrip() {
        let dir = std::env::temp_dir().join(format!("gale-ipc-{}", rand::random::<u32>()));
        let token = write_token(&dir).unwrap();
        let ep = endpoint(&dir);
        let handler: Handler = Arc::new(|env: Envelope| Box::pin(async move { Response::ok(&env.request.id, serde_json::json!({"pong": true})) }));
        tokio::spawn(serve(ep.clone(), token.clone(), handler));
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut s = connect(&ep).await.unwrap();
        let req = Request { v: 1, id: "1".into(), command: Command::Ping };
        let ok = call(&mut s, &Envelope { token: token.clone(), origin: "o".into(), request: req.clone() }).await.unwrap();
        assert!(ok.ok);
        let bad = call(&mut s, &Envelope { token: "x".repeat(64), origin: "o".into(), request: req }).await.unwrap();
        assert_eq!(bad.error.unwrap().code, "unauthorized");
        std::fs::remove_dir_all(dir).ok();
    }
}
