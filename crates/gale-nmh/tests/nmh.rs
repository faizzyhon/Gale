//! Drives the real gale-nmh binary over stdin/stdout like Chrome does.
#![cfg(not(windows))]

use std::process::Stdio;
use std::sync::Arc;

use gale_core::ipc::{self, Handler};
use gale_core::protocol::{read_frame, write_frame, Command, Envelope, Response};
use tokio::process::{Child, Command as Proc};

const ORIGIN: &str = "chrome-extension://abiednbonckeaneafpemajipbefdfblo/";

fn spawn(dir: &std::path::Path, origin: &str) -> Child {
    Proc::new(env!("CARGO_BIN_EXE_gale-nmh"))
        .arg(origin)
        .env("GALE_DATA_DIR", dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

async fn ask(child: &mut Child, msg: &str) -> Response {
    write_frame(child.stdin.as_mut().unwrap(), msg.as_bytes()).await.unwrap();
    let f = read_frame(child.stdout.as_mut().unwrap()).await.unwrap().unwrap();
    serde_json::from_slice(&f).unwrap()
}

#[tokio::test]
async fn relays_valid_messages_and_rejects_bad_ones() {
    let dir = std::env::temp_dir().join(format!("gale-nmh-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // App not running: ping reports it and does not try to launch anything.
    let mut c = spawn(&dir, ORIGIN);
    let r = ask(&mut c, r#"{"v":1,"id":"p0","cmd":"ping"}"#).await;
    assert_eq!(r.error.unwrap().code, "not_running");
    drop(c);

    let token = ipc::write_token(&dir).unwrap();
    let seen = Arc::new(std::sync::Mutex::new(Vec::<Envelope>::new()));
    let s2 = seen.clone();
    let handler: Handler = Arc::new(move |env: Envelope| {
        s2.lock().unwrap().push(env.clone());
        Box::pin(async move { Response::ok(&env.request.id, serde_json::json!({"ok": 1})) })
    });
    tokio::spawn(ipc::serve(ipc::endpoint(&dir), token.clone(), handler));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut c = spawn(&dir, ORIGIN);
    assert!(ask(&mut c, r#"{"v":1,"id":"p1","cmd":"ping"}"#).await.ok);
    let r = ask(&mut c, r#"{"v":1,"id":"a1","cmd":"add_download","payload":{"url":"https://example.com/f.zip","cookies":"s=1"}}"#).await;
    assert!(r.ok);
    let r = ask(&mut c, r#"{"v":1,"id":"x","cmd":"exec","payload":"calc.exe"}"#).await;
    assert_eq!((r.id.as_str(), r.error.unwrap().code.as_str()), ("x", "invalid"));
    let r = ask(&mut c, r#"{"v":1,"id":"y","cmd":"add_download","payload":{"url":"file:///etc/passwd"}}"#).await;
    assert_eq!(r.error.unwrap().code, "invalid");

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "only valid requests reach the app");
    assert_eq!(seen[0].origin, ORIGIN);
    assert_eq!(seen[0].token, token);
    assert!(matches!(&seen[1].request.command, Command::AddDownload(p) if p.cookies.as_deref() == Some("s=1")));
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn unknown_extension_is_refused() {
    let dir = std::env::temp_dir();
    let mut c = spawn(&dir, "chrome-extension://evilevilevilevilevilevilevilevil/");
    let status = c.wait().await.unwrap();
    assert_eq!(status.code(), Some(1));
}
