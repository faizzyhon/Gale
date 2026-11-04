//! Native messaging protocol v1 (extension ⇄ host ⇄ app). See docs/architecture/ARCHITECTURE.md §6.
//! Everything here is untrusted input: strict shapes, size caps, scheme allow-list.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Error, Result};

/// Chrome allows 1 MB host→extension; we apply the same cap in both directions.
pub const MAX_MESSAGE: usize = 1024 * 1024;
pub const VERSION: u8 = 1;
/// Native messaging host name registered with Chrome/Edge.
pub const HOST_NAME: &str = "com.gale.nmh";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub v: u8,
    pub id: String,
    #[serde(flatten)]
    pub command: Command,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "cmd", content = "payload", rename_all = "snake_case")]
pub enum Command {
    Ping,
    AddDownload(AddPayload),
    AddBatch(BatchPayload),
    GetRules,
    Recent { limit: u32 },
    ShowApp,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddPayload {
    pub url: String,
    #[serde(default)]
    pub referrer: Option<String>,
    #[serde(default)]
    pub page_url: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub file_size: Option<i64>,
    #[serde(default)]
    pub mime: Option<String>,
    /// `name=value; …` for this URL only. Held in memory by the app, never stored or logged.
    #[serde(default)]
    pub cookies: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    /// true = start immediately; false = open the app's Add dialog for the user to confirm.
    #[serde(default)]
    pub silent: bool,
    /// Put into the default queue instead of starting.
    #[serde(default)]
    pub enqueue: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchItem {
    pub url: String,
    #[serde(default)]
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchPayload {
    pub items: Vec<BatchItem>,
    #[serde(default)]
    pub referrer: Option<String>,
    #[serde(default)]
    pub page_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub v: u8,
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

impl Response {
    pub fn ok(id: &str, result: serde_json::Value) -> Self {
        Self { v: VERSION, id: id.into(), ok: true, result: Some(result), error: None }
    }
    pub fn err(id: &str, code: &str, message: impl Into<String>) -> Self {
        Self { v: VERSION, id: id.into(), ok: false, result: None, error: Some(ErrorBody { code: code.into(), message: message.into() }) }
    }
}

/// Host → app envelope. The token proves the sender can read the user's private app folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub token: String,
    pub origin: String,
    pub request: Request,
}

fn check_url(u: &str) -> Result<()> {
    if u.len() > 8192 {
        return Err(Error::InvalidUrl("URL too long".into()));
    }
    crate::security::validate_url(u).map(|_| ())
}

fn check_len(name: &str, v: &Option<String>, max: usize) -> Result<()> {
    match v {
        Some(s) if s.len() > max || s.chars().any(|c| c == '\r' || c == '\n' || c == '\0') => {
            Err(Error::InvalidState(format!("{name} is too long or contains control characters")))
        }
        _ => Ok(()),
    }
}

impl Request {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let r: Request = serde_json::from_slice(bytes).map_err(|e| Error::InvalidState(format!("malformed message: {e}")))?;
        r.validate()?;
        Ok(r)
    }

    pub fn validate(&self) -> Result<()> {
        if self.v != VERSION {
            return Err(Error::InvalidState(format!("unsupported protocol version {}", self.v)));
        }
        if self.id.is_empty() || self.id.len() > 64 {
            return Err(Error::InvalidState("bad request id".into()));
        }
        match &self.command {
            Command::AddDownload(p) => {
                check_url(&p.url)?;
                // Header values must not be able to inject extra headers.
                check_len("referrer", &p.referrer, 8192)?;
                check_len("pageUrl", &p.page_url, 8192)?;
                check_len("filename", &p.filename, 1024)?;
                check_len("mime", &p.mime, 256)?;
                check_len("cookies", &p.cookies, 64 * 1024)?;
                check_len("userAgent", &p.user_agent, 1024)?;
            }
            Command::AddBatch(b) => {
                if b.items.is_empty() || b.items.len() > 2000 {
                    return Err(Error::InvalidState("a batch must have 1–2000 links".into()));
                }
                check_len("referrer", &b.referrer, 8192)?;
                check_len("pageUrl", &b.page_url, 8192)?;
                for i in &b.items {
                    check_url(&i.url)?;
                    check_len("filename", &i.filename, 1024)?;
                }
            }
            Command::Recent { limit } if *limit > 100 => return Err(Error::InvalidState("limit must be ≤ 100".into())),
            _ => {}
        }
        Ok(())
    }
}

/// Chrome native messaging framing: u32 native-endian (little-endian on Windows x64) length + JSON.
pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let n = u32::from_le_bytes(len) as usize;
    if n == 0 || n > MAX_MESSAGE {
        return Err(Error::InvalidState(format!("message length {n} out of range")));
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

pub async fn write_frame<W: AsyncWrite + Unpin>(w: &mut W, body: &[u8]) -> Result<()> {
    if body.len() > MAX_MESSAGE {
        return Err(Error::InvalidState("response too large".into()));
    }
    w.write_all(&(body.len() as u32).to_le_bytes()).await?;
    w.write_all(body).await?;
    w.flush().await?;
    Ok(())
}

/// Constant-time comparison for the IPC token.
pub fn token_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_commands() {
        let r = Request::parse(br#"{"v":1,"id":"a","cmd":"ping"}"#).unwrap();
        assert_eq!(r.command, Command::Ping);
        let r = Request::parse(br#"{"v":1,"id":"b","cmd":"add_download","payload":{"url":"https://x.com/f.zip","silent":true}}"#).unwrap();
        assert!(matches!(r.command, Command::AddDownload(AddPayload { silent: true, .. })));
    }

    #[test]
    fn rejects_unknown_or_dangerous_input() {
        for bad in [
            &br#"{"v":1,"id":"a","cmd":"run_shell","payload":{"cmd":"calc"}}"#[..],
            br#"{"v":2,"id":"a","cmd":"ping"}"#,
            br#"{"v":1,"id":"a","cmd":"add_download","payload":{"url":"file:///C:/x"}}"#,
            br#"{"v":1,"id":"a","cmd":"add_download","payload":{"url":"https://x/y","savePath":"C:\\Windows"}}"#,
            br#"{"v":1,"id":"a","cmd":"add_download","payload":{"url":"https://x/y","cookies":"a=b\r\nX-Evil: 1"}}"#,
            br#"{"v":1,"id":"a","cmd":"add_batch","payload":{"items":[]}}"#,
            br#"not json"#,
        ] {
            assert!(Request::parse(bad).is_err(), "{}", String::from_utf8_lossy(bad));
        }
    }

    #[tokio::test]
    async fn framing_roundtrip_and_limits() {
        let (mut a, mut b) = tokio::io::duplex(4096);
        write_frame(&mut a, b"{\"x\":1}").await.unwrap();
        assert_eq!(read_frame(&mut b).await.unwrap().unwrap(), b"{\"x\":1}");
        let (mut a, mut b) = tokio::io::duplex(64);
        a.write_all(&((MAX_MESSAGE as u32) + 1).to_le_bytes()).await.unwrap();
        assert!(read_frame(&mut b).await.is_err());
        drop(a);
        let (a, mut b) = tokio::io::duplex(64);
        drop(a);
        assert!(read_frame(&mut b).await.unwrap().is_none(), "clean EOF");
    }

    #[test]
    fn token_comparison() {
        assert!(token_eq("abc", "abc"));
        assert!(!token_eq("abc", "abd"));
        assert!(!token_eq("abc", "ab"));
    }
}
