//! Native messaging host. Chrome/Edge start this per extension connection and talk over stdin/stdout.
//! It validates the caller and every message, then relays allow-listed commands to the running app.
//! It never runs commands, opens paths, or touches files besides reading the IPC token.

use std::path::PathBuf;
use std::time::Duration;

use gale_core::ipc::{self, Stream};
use gale_core::protocol::{read_frame, write_frame, Command, Envelope, Request, Response};

/// Derived from browser-extension/manifest.json "key"; see browser-extension/keys/extension-id.txt.
const ALLOWED_ORIGINS: &[&str] = &["chrome-extension://abiednbonckeaneafpemajipbefdfblo/"];

fn app_exe() -> Option<PathBuf> {
    let name = if cfg!(windows) { "gale.exe" } else { "gale" };
    Some(std::env::current_exe().ok()?.parent()?.join(name))
}

fn data_dir() -> Option<PathBuf> {
    // Test hook, compiled out of release builds so the relay target can't be redirected in production.
    #[cfg(debug_assertions)]
    if let Ok(d) = std::env::var("GALE_DATA_DIR") {
        return Some(PathBuf::from(d));
    }
    ipc::app_local_dir()
}

struct Relay {
    origin: String,
    dir: PathBuf,
    conn: Option<(Stream, String)>,
}

impl Relay {
    async fn connect(&mut self, launch: bool) -> Result<(), String> {
        let ep = ipc::endpoint(&self.dir);
        let mut launched = false;
        for _ in 0..50 {
            if let Ok(s) = ipc::connect(&ep).await {
                verify_server(&s)?;
                let token = std::fs::read_to_string(ipc::token_path(&self.dir)).map_err(|_| "Gale is not running".to_string())?;
                self.conn = Some((s, token.trim().to_string()));
                return Ok(());
            }
            if !launch {
                break;
            }
            if !launched {
                let exe = app_exe().ok_or("cannot locate Gale")?;
                std::process::Command::new(&exe).arg("--background").spawn().map_err(|e| format!("could not start Gale: {e}"))?;
                launched = true;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        Err("Gale is not running".into())
    }

    async fn forward(&mut self, request: Request) -> Response {
        let id = request.id.clone();
        // Only these commands may launch the app; ping must report the real state.
        let may_launch = matches!(request.command, Command::AddDownload(_) | Command::AddBatch(_) | Command::ShowApp);
        for attempt in 0..2 {
            if self.conn.is_none() {
                if let Err(e) = self.connect(may_launch).await {
                    return Response::err(&id, "not_running", e);
                }
            }
            let (stream, token) = self.conn.as_mut().expect("connected above");
            let env = Envelope { token: token.clone(), origin: self.origin.clone(), request: request.clone() };
            match ipc::call(stream, &env).await {
                Ok(r) => return r,
                // App restarted since we connected: reconnect once with the new token.
                Err(_) if attempt == 0 => self.conn = None,
                Err(e) => return Response::err(&id, "internal", e.to_string()),
            }
        }
        Response::err(&id, "internal", "unreachable")
    }
}

/// Refuse to talk to a pipe owned by anything other than our own gale.exe (pipe-squatting defence).
#[cfg(windows)]
fn verify_server(s: &Stream) -> Result<(), String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    let mut pid = 0u32;
    // SAFETY: valid pipe handle owned by `s` for the duration of the call.
    if unsafe { GetNamedPipeServerProcessId(s.as_raw_handle() as _, &mut pid) } == 0 {
        return Err("cannot identify the Gale process".into());
    }
    // SAFETY: plain Win32 calls with checked results; the handle is closed below.
    let path = unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return Err("cannot identify the Gale process".into());
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return Err("cannot identify the Gale process".into());
        }
        String::from_utf16_lossy(&buf[..len as usize])
    };
    let expected = app_exe().ok_or("cannot locate Gale")?;
    if !path.eq_ignore_ascii_case(&expected.to_string_lossy()) {
        return Err("the Gale connection is owned by an unexpected program".into());
    }
    Ok(())
}

#[cfg(not(windows))]
fn verify_server(_s: &Stream) -> Result<(), String> {
    Ok(()) // Unix socket lives in a 0700 user folder; dev/test only.
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Chrome passes the caller as the first argument; Edge/Chrome on Windows add --parent-window.
    let origin = std::env::args().skip(1).find(|a| a.starts_with("chrome-extension://")).unwrap_or_default();
    if !ALLOWED_ORIGINS.contains(&origin.as_str()) {
        eprintln!("gale-nmh: rejected caller {origin:?}");
        std::process::exit(1);
    }
    let Some(dir) = data_dir() else {
        eprintln!("gale-nmh: no local app data folder");
        std::process::exit(1);
    };
    let mut relay = Relay { origin, dir, conn: None };
    let (mut stdin, mut stdout) = (tokio::io::stdin(), tokio::io::stdout());
    loop {
        let frame = match read_frame(&mut stdin).await {
            Ok(Some(f)) => f,
            Ok(None) => break, // browser closed the port
            Err(e) => {
                eprintln!("gale-nmh: {e}");
                break;
            }
        };
        let resp = match Request::parse(&frame) {
            Ok(req) => relay.forward(req).await,
            Err(e) => {
                let id = serde_json::from_slice::<serde_json::Value>(&frame).ok().and_then(|v| v["id"].as_str().map(String::from)).unwrap_or_default();
                Response::err(&id, "invalid", e.to_string())
            }
        };
        let body = serde_json::to_vec(&resp).unwrap_or_default();
        if write_frame(&mut stdout, &body).await.is_err() {
            break;
        }
    }
}
