//! End-to-end tests against a local HTTP server whose behaviour we control.

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{Path as UrlPath, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use futures_util::stream;
use gale_core::db::{Db, Download, DownloadStatus};
use gale_core::engine::{self, EngineConfig, Job, Outcome, RequestHeaders, Tick};
use gale_core::manager::{AddOptions, EventSink, Manager};
use gale_core::Error;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

const SIZE: usize = 3 * 1024 * 1024 + 12_345; // deliberately not a multiple of anything

fn content(len: usize, version: u32) -> Bytes {
    let mut x = 0x9E37_79B9u32 ^ version.wrapping_mul(0x85EB_CA6B);
    Bytes::from((0..len).map(|_| { x ^= x << 13; x ^= x >> 17; x ^= x << 5; x as u8 }).collect::<Vec<_>>())
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

#[derive(Default)]
struct ServerState {
    version: AtomicUsize,
    requests: AtomicUsize,
    flaky_failures: AtomicUsize,
    range_starts: Mutex<Vec<u64>>,
}

/// Modes: ranged | plain | lying | flaky | slow | changing | chunked | auth
async fn serve(State(st): State<Arc<ServerState>>, UrlPath(mode): UrlPath<String>, req: HeaderMap) -> Response {
    st.requests.fetch_add(1, Ordering::SeqCst);
    let version = if mode == "changing" { st.version.load(Ordering::SeqCst) as u32 } else { 0 };
    let data = content(SIZE, version);
    let etag = format!("\"v{version}\"");

    if mode == "auth" && req.get(header::AUTHORIZATION).is_none() {
        return Response::builder().status(401).body(Body::empty()).unwrap();
    }
    if mode == "chunked" {
        let chunks: Vec<Result<Bytes, Infallible>> = data.chunks(10_000).map(|c| Ok(Bytes::copy_from_slice(c))).collect();
        return Response::builder().status(200).body(Body::from_stream(stream::iter(chunks))).unwrap();
    }

    let honours_ranges = !matches!(mode.as_str(), "plain" | "lying");
    let if_range_ok = req.get(header::IF_RANGE).is_none_or(|v| v.to_str().unwrap() == etag);
    let range = req
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok()?.strip_prefix("bytes=").map(str::to_owned))
        .and_then(|r| {
            let (s, e) = r.split_once('-')?;
            let s: usize = s.parse().ok()?;
            let e: usize = if e.is_empty() { SIZE - 1 } else { e.parse::<usize>().ok()?.min(SIZE - 1) };
            Some((s, e))
        })
        .filter(|_| honours_ranges && if_range_ok);

    let mut b = Response::builder().header(header::ETAG, &etag).header(header::CONTENT_TYPE, "application/octet-stream");
    if mode != "plain" {
        b = b.header(header::ACCEPT_RANGES, "bytes"); // "lying" advertises ranges but ignores them
    }
    let (status, slice) = match range {
        Some((s, e)) => {
            if (s, e) != (0, 0) {
                st.range_starts.lock().unwrap().push(s as u64); // probe requests excluded
            }
            b = b.header(header::CONTENT_RANGE, format!("bytes {s}-{e}/{SIZE}"));
            (StatusCode::PARTIAL_CONTENT, data.slice(s..=e))
        }
        None => (StatusCode::OK, data),
    };
    b = b.status(status).header(header::CONTENT_LENGTH, slice.len());

    let fail_this_one = mode == "flaky" && slice.len() > 1 && st.flaky_failures.fetch_add(1, Ordering::SeqCst) < 6;
    let delay = if mode == "slow" { Some(Duration::from_millis(100)) } else { None };
    let chunks: Vec<Bytes> = slice.chunks(16 * 1024).map(Bytes::copy_from_slice).collect();
    let body = stream::unfold((chunks.into_iter(), 0usize), move |(mut it, n)| async move {
        if fail_this_one && n == 3 {
            return Some((Err(std::io::Error::other("simulated connection drop")), (it, n + 1)));
        }
        if fail_this_one && n > 3 {
            return None;
        }
        let c = it.next()?;
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
        Some((Ok(c), (it, n + 1)))
    });
    b.body(Body::from_stream(body)).unwrap()
}

async fn start_server() -> (SocketAddr, Arc<ServerState>) {
    let st = Arc::new(ServerState::default());
    let app = Router::new().route("/{mode}/file.bin", get(serve)).with_state(st.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (addr, st)
}

fn cfg() -> EngineConfig {
    EngineConfig { connections: 8, min_segment: 64 * 1024, retry_base: Duration::from_millis(20), progress_interval: Duration::from_millis(50), ..Default::default() }
}

fn url(addr: SocketAddr, mode: &str) -> url::Url {
    format!("http://{addr}/{mode}/file.bin").parse().unwrap()
}

async fn run_engine(addr: SocketAddr, mode: &str, dir: &Path, c: &EngineConfig) -> (gale_core::Result<Outcome>, u32) {
    let client = engine::build_client(c).unwrap();
    let probe = engine::probe(&client, &url(addr, mode), &RequestHeaders::default(), None).await.unwrap();
    let final_path = dir.join(&probe.file_name);
    let job = Job {
        probe,
        headers: RequestHeaders::default(),
        temp_path: gale_core::security::temp_path_for(&final_path),
        final_path,
        segments: vec![],
        expected_sha256: None,
    };
    let max_active = Arc::new(AtomicUsize::new(0));
    let m = max_active.clone();
    let on_tick: Arc<dyn Fn(Tick) + Send + Sync> = Arc::new(move |t: Tick| {
        m.fetch_max(t.active_connections as usize, Ordering::SeqCst);
    });
    let r = engine::run(&client, c, job, CancellationToken::new(), on_tick).await;
    (r, max_active.load(Ordering::SeqCst) as u32)
}

fn assert_completed_matches(r: gale_core::Result<Outcome>, version: u32) -> std::path::PathBuf {
    match r.expect("download should succeed") {
        Outcome::Completed { path, bytes, .. } => {
            assert_eq!(bytes as usize, SIZE);
            let got = std::fs::read(&path).unwrap();
            assert_eq!(sha(&got), sha(&content(SIZE, version)), "file content must match source");
            assert!(!gale_core::security::temp_path_for(&path).exists(), "temp file removed");
            path
        }
        o => panic!("unexpected outcome {o:?}"),
    }
}

#[tokio::test]
async fn multi_connection_download_matches_source() {
    let (addr, st) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let client = engine::build_client(&cfg()).unwrap();
    let p = engine::probe(&client, &url(addr, "ranged"), &RequestHeaders::default(), None).await.unwrap();
    assert!(p.ranges);
    assert_eq!(p.total, Some(SIZE as u64));
    assert_eq!(p.etag.as_deref(), Some("\"v0\""));
    assert_eq!(p.file_name, "file.bin");

    let (r, max_active) = run_engine(addr, "ranged", dir.path(), &cfg()).await;
    assert_completed_matches(r, 0);
    assert!(max_active > 1, "used parallel connections (max seen {max_active})");
    assert!(st.range_starts.lock().unwrap().iter().filter(|&&s| s > 0).count() >= 7);
}

#[tokio::test]
async fn single_connection_fallback_when_server_ignores_ranges() {
    for mode in ["plain", "lying"] {
        let (addr, _) = start_server().await;
        let dir = tempfile::tempdir().unwrap();
        let client = engine::build_client(&cfg()).unwrap();
        let p = engine::probe(&client, &url(addr, mode), &RequestHeaders::default(), None).await.unwrap();
        assert!(!p.ranges, "{mode}: must not trust ranges");
        let (r, max_active) = run_engine(addr, mode, dir.path(), &cfg()).await;
        assert_completed_matches(r, 0);
        assert!(max_active <= 1, "{mode}: single connection");
    }
}

#[tokio::test]
async fn unknown_length_stream_downloads() {
    let (addr, _) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let (r, _) = run_engine(addr, "chunked", dir.path(), &cfg()).await;
    assert_completed_matches(r, 0);
}

#[tokio::test]
async fn dropped_connections_are_retried_without_corruption() {
    let (addr, st) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let (r, _) = run_engine(addr, "flaky", dir.path(), &cfg()).await;
    assert_completed_matches(r, 0);
    assert!(st.flaky_failures.load(Ordering::SeqCst) >= 6, "failures were injected");
}

#[tokio::test]
async fn work_stealing_splits_slow_segments() {
    let (addr, st) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    // Two planned segments but four workers: idle workers must steal halves.
    let c = EngineConfig { connections: 4, min_segment: SIZE as u64 / 2, ..cfg() };
    let client = engine::build_client(&c).unwrap();
    let mut probe = engine::probe(&client, &url(addr, "slow"), &RequestHeaders::default(), None).await.unwrap();
    probe.file_name = "steal.bin".into();
    let final_path = dir.path().join("steal.bin");
    let c2 = EngineConfig { min_segment: 128 * 1024, ..c.clone() };
    let job = Job {
        probe,
        headers: RequestHeaders::default(),
        temp_path: gale_core::security::temp_path_for(&final_path),
        final_path,
        segments: engine::plan_segments(Some(SIZE as u64), true, 2, SIZE as u64 / 2),
        expected_sha256: None,
    };
    // Resume path requires an existing temp of the right length; emulate a fresh pre-allocated file.
    std::fs::File::create(&job.temp_path).unwrap().set_len(SIZE as u64).unwrap();
    let r = engine::run(&client, &c2, job, CancellationToken::new(), Arc::new(|_| {})).await;
    assert_completed_matches(r, 0);
    assert!(st.range_starts.lock().unwrap().len() > 3, "extra ranges were requested by stealing workers");
}

#[tokio::test]
async fn checksum_verification() {
    let (addr, _) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let c = cfg();
    let client = engine::build_client(&c).unwrap();
    let probe = engine::probe(&client, &url(addr, "ranged"), &RequestHeaders::default(), None).await.unwrap();
    let final_path = dir.path().join("x.bin");
    let mk = |sha: &str| Job {
        probe: probe.clone(),
        headers: RequestHeaders::default(),
        temp_path: gale_core::security::temp_path_for(&final_path),
        final_path: final_path.clone(),
        segments: vec![],
        expected_sha256: Some(sha.into()),
    };
    let bad = engine::run(&client, &c, mk(&"0".repeat(64)), CancellationToken::new(), Arc::new(|_| {})).await;
    assert!(matches!(bad, Err(Error::ChecksumMismatch { .. })));
    assert!(!final_path.exists(), "a file failing verification is never moved into place");
    let good = engine::run(&client, &c, mk(&sha(&content(SIZE, 0)).to_uppercase()), CancellationToken::new(), Arc::new(|_| {})).await;
    assert_completed_matches(good, 0);
}

// ---------------------------------------------------------------------------------------------
// Manager-level tests (DB + engine + state machine)

struct NoEvents;
impl EventSink for NoEvents {
    fn progress(&self, _: i64, _: &Tick) {}
    fn changed(&self, _: &Download) {}
}

async fn manager(db_path: &Path) -> Arc<Manager> {
    let db = Db::open(db_path).await.unwrap();
    db.recover_interrupted().await.unwrap();
    Manager::new(db, cfg(), 3, Arc::new(NoEvents)).unwrap()
}

async fn wait_for(m: &Manager, id: i64, pred: impl Fn(&Download) -> bool) -> Download {
    for _ in 0..600 {
        let d = m.db().get_download(id).await.unwrap();
        if pred(&d) {
            return d;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("timed out; last state {:?}", m.db().get_download(id).await.unwrap());
}

fn add(addr: SocketAddr, mode: &str, dir: &Path) -> AddOptions {
    AddOptions { url: url(addr, mode).to_string(), save_dir: dir.to_string_lossy().into(), start: true, ..Default::default() }
}

#[tokio::test]
async fn pause_resume_and_restart_recovery() {
    let (addr, st) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("gale.db");
    let m = manager(&db_path).await;
    let id = m.add(add(addr, "slow", dir.path())).await.unwrap();

    wait_for(&m, id, |d| d.status == DownloadStatus::Downloading && d.downloaded_bytes > 0).await;
    m.stop_and_wait(id).await.unwrap();
    let paused = m.db().get_download(id).await.unwrap();
    assert_eq!(paused.status, DownloadStatus::Paused);
    assert!(paused.downloaded_bytes > 0 && (paused.downloaded_bytes as usize) < SIZE);
    assert!(!m.db().load_segments(id).await.unwrap().is_empty(), "segments persisted");
    drop(m);

    // Simulate an application restart: new manager over the same database file.
    st.range_starts.lock().unwrap().clear();
    let m = manager(&db_path).await;
    m.start(id).await.unwrap();
    let done = wait_for(&m, id, |d| matches!(d.status, DownloadStatus::Completed | DownloadStatus::Failed)).await;
    assert_eq!(done.status, DownloadStatus::Completed, "{:?}", done.error_message);
    let bytes = std::fs::read(done.final_path.unwrap()).unwrap();
    assert_eq!(sha(&bytes), sha(&content(SIZE, 0)));
    let resumed_starts = st.range_starts.lock().unwrap().clone();
    assert!(resumed_starts.iter().all(|&s| s > 0), "resume never re-requested byte 0: {resumed_starts:?}");
    assert_eq!(m.db().list_history(10).await.unwrap()[0].outcome, "completed");
}

#[tokio::test]
async fn crash_mid_download_is_recovered_on_startup() {
    let (addr, _) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("gale.db");
    let m = manager(&db_path).await;
    let id = m.add(add(addr, "slow", dir.path())).await.unwrap();
    wait_for(&m, id, |d| d.downloaded_bytes > 0).await;
    m.stop_and_wait(id).await.unwrap();
    // Emulate a crash: status left as 'downloading' in the DB.
    m.db().set_status(id, DownloadStatus::Downloading, None).await.unwrap();
    drop(m);

    let m = manager(&db_path).await; // runs recover_interrupted()
    assert_eq!(m.db().get_download(id).await.unwrap().status, DownloadStatus::Paused);
    m.start(id).await.unwrap();
    let d = wait_for(&m, id, |d| d.status == DownloadStatus::Completed).await;
    assert_eq!(sha(&std::fs::read(d.final_path.unwrap()).unwrap()), sha(&content(SIZE, 0)));
}

#[tokio::test]
async fn changed_remote_file_restarts_instead_of_splicing() {
    let (addr, st) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let m = manager(&dir.path().join("gale.db")).await;
    let id = m.add(add(addr, "changing", dir.path())).await.unwrap();
    // "changing" is fast; pause as soon as any progress exists, or accept completion of v0.
    let d = wait_for(&m, id, |d| d.downloaded_bytes > 0 || d.status == DownloadStatus::Completed).await;
    if d.status == DownloadStatus::Completed {
        return; // too fast to interrupt on this machine; engine-level If-Range test below still covers it
    }
    m.stop_and_wait(id).await.unwrap();
    st.version.store(1, Ordering::SeqCst); // server now serves different bytes + new ETag
    m.start(id).await.unwrap();
    let d = wait_for(&m, id, |d| matches!(d.status, DownloadStatus::Completed | DownloadStatus::Failed)).await;
    assert_eq!(d.status, DownloadStatus::Completed);
    assert_eq!(sha(&std::fs::read(d.final_path.unwrap()).unwrap()), sha(&content(SIZE, 1)), "only new-version bytes");
}

#[tokio::test]
async fn if_range_mismatch_is_detected_by_engine() {
    let (addr, st) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let c = cfg();
    let client = engine::build_client(&c).unwrap();
    let probe = engine::probe(&client, &url(addr, "changing"), &RequestHeaders::default(), None).await.unwrap();
    st.version.store(7, Ordering::SeqCst); // file changes between probe and transfer
    let final_path = dir.path().join("c.bin");
    let job = Job {
        probe,
        headers: RequestHeaders::default(),
        temp_path: gale_core::security::temp_path_for(&final_path),
        final_path: final_path.clone(),
        segments: vec![],
        expected_sha256: None,
    };
    let r = engine::run(&client, &c, job, CancellationToken::new(), Arc::new(|_| {})).await;
    assert!(matches!(r, Err(Error::RemoteChanged)), "{r:?}");
    assert!(!final_path.exists());
}

#[tokio::test]
async fn invalid_urls_duplicates_and_auth_failures() {
    let (addr, _) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let m = manager(&dir.path().join("gale.db")).await;

    for bad in ["ftp://x/y", "file:///etc/passwd", "javascript:alert(1)", "nonsense"] {
        let e = m.add(AddOptions { url: bad.into(), save_dir: dir.path().to_string_lossy().into(), ..Default::default() }).await;
        assert!(matches!(e, Err(Error::InvalidUrl(_))), "{bad}");
    }
    let rel = m.add(AddOptions { url: url(addr, "ranged").to_string(), save_dir: "relative/dir".into(), ..Default::default() }).await;
    assert!(matches!(rel, Err(Error::UnsafePath(_))));

    let mut o = add(addr, "ranged", dir.path());
    o.start = false;
    m.add(o.clone()).await.unwrap();
    assert!(matches!(m.add(o).await, Err(Error::InvalidState(_))), "duplicate rejected");

    let id = m.add(add(addr, "auth", dir.path())).await.unwrap();
    let d = wait_for(&m, id, |d| d.status == DownloadStatus::Failed).await;
    assert_eq!(d.error_code.as_deref(), Some("http_error"));

    let mut o = add(addr, "auth", dir.path());
    o.url.push_str("?with-auth");
    o.headers = RequestHeaders(vec![("Authorization".into(), "Bearer test".into())]);
    let id = m.add(o).await.unwrap();
    let d = wait_for(&m, id, |d| matches!(d.status, DownloadStatus::Completed | DownloadStatus::Failed)).await;
    assert_eq!(d.status, DownloadStatus::Completed, "{:?}", d.error_message);
    let stored: String = sqlx::query_scalar("SELECT group_concat(COALESCE(error_message,'') || url) FROM downloads")
        .fetch_one(m.db().pool())
        .await
        .unwrap();
    assert!(!stored.contains("Bearer"), "auth headers never persisted");
}

#[tokio::test]
async fn cancel_removes_partial_but_remove_keeps_completed_file() {
    let (addr, _) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let m = manager(&dir.path().join("gale.db")).await;

    let id = m.add(add(addr, "slow", dir.path())).await.unwrap();
    let d = wait_for(&m, id, |d| d.downloaded_bytes > 0 && d.status == DownloadStatus::Downloading).await;
    m.cancel(id).await.unwrap();
    assert_eq!(m.db().get_download(id).await.unwrap().status, DownloadStatus::Cancelled);
    assert!(!Path::new(d.temp_path.as_deref().unwrap()).exists(), "partial deleted on cancel");

    let id = m.add(add(addr, "ranged", dir.path())).await.unwrap();
    let d = wait_for(&m, id, |d| d.status == DownloadStatus::Completed).await;
    m.remove(id, false).await.unwrap();
    assert!(Path::new(d.final_path.as_deref().unwrap()).exists(), "remove without consent keeps the file");
    assert!(m.db().get_download(id).await.is_err());
    assert_eq!(m.db().list_history(10).await.unwrap().len(), 1, "history survives removal");
}

#[tokio::test]
async fn simultaneous_downloads_and_duplicate_names() {
    let (addr, _) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let m = manager(&dir.path().join("gale.db")).await;
    let mut ids = vec![];
    for i in 0..4 {
        let mut o = add(addr, "ranged", dir.path());
        o.url.push_str(&format!("?n={i}"));
        ids.push(m.add(o).await.unwrap());
    }
    let mut names = vec![];
    for id in ids {
        let d = wait_for(&m, id, |d| matches!(d.status, DownloadStatus::Completed | DownloadStatus::Failed)).await;
        assert_eq!(d.status, DownloadStatus::Completed, "{:?}", d.error_message);
        names.push(d.file_name);
    }
    names.sort();
    assert_eq!(names, ["file (1).bin", "file (2).bin", "file (3).bin", "file.bin"]);
}

// ---------------------------------------------------------------------------------------------
// Phase 6: limits, proxy, queues, scheduler, grabber

#[tokio::test]
async fn bandwidth_limit_is_enforced() {
    let (addr, _) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let c = EngineConfig { download_limit_bps: 2 * 1024 * 1024, ..cfg() };
    let start = std::time::Instant::now();
    let (r, _) = run_engine(addr, "ranged", dir.path(), &c).await;
    assert_completed_matches(r, 0);
    let secs = start.elapsed().as_secs_f64();
    // ~3.01 MiB at 2 MiB/s with an empty bucket at start ≈ 1.5 s
    assert!(secs > 1.2, "limit ignored: {secs:.2}s");
}

#[tokio::test]
async fn per_host_connection_limit_caps_parallelism() {
    let (addr, _) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let c = EngineConfig { limits: gale_core::engine::Limits::new(None, Some(gale_core::limits::HostLimiter::new(2))), ..cfg() };
    let (r, max_active) = run_engine(addr, "slow", dir.path(), &c).await;
    assert_completed_matches(r, 0);
    assert!((1..=2).contains(&max_active), "max active {max_active}");
}

#[tokio::test]
async fn unreachable_proxy_fails_cleanly() {
    let (addr, _) = start_server().await;
    let c = EngineConfig {
        proxy: Some(gale_core::engine::ProxyConfig { url: "http://127.0.0.1:9".into(), username: Some("u".into()), password: Some("p".into()) }),
        connect_timeout: Duration::from_secs(2),
        ..cfg()
    };
    let client = engine::build_client(&c).unwrap();
    let e = engine::probe(&client, &url(addr, "ranged"), &RequestHeaders::default(), None).await.unwrap_err();
    assert_eq!(e.code(), "network");
    let bad = EngineConfig { proxy: Some(gale_core::engine::ProxyConfig { url: "::nope::".into(), username: None, password: None }), ..cfg() };
    assert!(engine::build_client(&bad).is_err());
}

struct QueueEvents(Arc<Mutex<Vec<(i64, bool)>>>, Arc<AtomicUsize>);
impl EventSink for QueueEvents {
    fn progress(&self, _: i64, _: &Tick) {}
    fn changed(&self, _: &Download) {}
    fn finished(&self, _: &Download) {
        self.1.fetch_add(1, Ordering::SeqCst);
    }
    fn queue_finished(&self, q: i64, shutdown: bool) {
        self.0.lock().unwrap().push((q, shutdown));
    }
}

#[tokio::test]
async fn queue_runs_items_one_at_a_time_and_reports_finish() {
    let (addr, _) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("gale.db")).await.unwrap();
    let qf = Arc::new(Mutex::new(vec![]));
    let fin = Arc::new(AtomicUsize::new(0));
    let m = Manager::new(db, cfg(), 8, Arc::new(QueueEvents(qf.clone(), fin.clone()))).unwrap();
    m.spawn_background();
    let q = m.db().save_queue(None, &gale_core::queue::QueueInput { name: "Q".into(), priority: 0, max_concurrent: 1, max_connections: 2 }).await.unwrap();
    let mut ids = vec![];
    for i in 0..3 {
        let mut o = add(addr, "ranged", dir.path());
        o.url.push_str(&format!("?q={i}"));
        o.start = false;
        o.queue_id = Some(q);
        ids.push(m.add(o).await.unwrap());
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(ids.iter().all(|id| !m.is_running(*id)), "stopped queue starts nothing");

    m.shutdown_after_queue(q, true);
    m.start_queue(q).await.unwrap();
    let mut max_parallel = 0;
    for _ in 0..400 {
        max_parallel = max_parallel.max(ids.iter().filter(|id| m.is_running(**id)).count());
        if !qf.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(max_parallel, 1, "queue limit respected");
    for id in &ids {
        let d = m.db().get_download(*id).await.unwrap();
        assert_eq!(d.status, DownloadStatus::Completed);
        assert!(d.connections <= 2, "queue connection cap applied");
    }
    assert_eq!(qf.lock().unwrap().as_slice(), &[(q, true)], "finished once, with shutdown request");
    assert_eq!(fin.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn scheduler_starts_a_queue_when_due() {
    let (addr, _) = start_server().await;
    let dir = tempfile::tempdir().unwrap();
    let m = manager(&dir.path().join("gale.db")).await;
    let q = m.db().default_queue_id().await.unwrap();
    m.db().set_queue_state(q, "stopped").await.unwrap();
    let mut o = add(addr, "ranged", dir.path());
    o.start = false;
    o.queue_id = Some(q);
    let id = m.add(o).await.unwrap();
    let now = chrono::Utc::now();
    m.db()
        .save_job(&gale_core::queue::SchedulerJob {
            id: 0, queue_id: q, kind: "start_queue".into(), run_at: (now + chrono::Duration::seconds(60)).to_rfc3339(),
            recurrence: None, enabled: true, shutdown_after: false, last_run_at: None,
        })
        .await
        .unwrap();
    m.run_scheduler(now).await.unwrap();
    assert!(!m.is_running(id), "not due yet");
    m.run_scheduler(now + chrono::Duration::seconds(61)).await.unwrap();
    let d = wait_for(&m, id, |d| d.status == DownloadStatus::Completed).await;
    assert_eq!(d.status, DownloadStatus::Completed);
    assert!(!m.db().list_jobs().await.unwrap()[0].enabled, "one-shot job disabled after running");
}

#[tokio::test]
async fn grabber_crawls_politely() {
    use axum::response::Html;
    let app = Router::new()
        .route("/robots.txt", get(|| async { "User-agent: *\nDisallow: /secret\n" }))
        .route("/", get(|| async { Html(r#"<a href="/a.html">a</a><a href="/secret/s.html">s</a><img src="/i/logo.png"><a href="https://other.example/x.pdf">x</a>"#) }))
        .route("/a.html", get(|| async { Html(r#"<a href="/docs/r.pdf">r</a><a href="/b.html">b</a><a href="/docs/r.pdf">dup</a>"#) }))
        .route("/b.html", get(|| async { Html(r#"<a href="/deep.pdf">too deep</a>"#) }))
        .route("/secret/s.html", get(|| async { Html(r#"<a href="/hidden.pdf">h</a>"#) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let client = engine::build_client(&cfg()).unwrap();
    let o = gale_core::grabber::GrabOptions {
        start_url: format!("http://{addr}/"),
        max_depth: 1,
        same_domain: true,
        extensions: vec!["pdf".into(), "png".into()],
        include: None,
        exclude: None,
        max_pages: 50,
        delay_ms: 200,
    };
    let r = gale_core::grabber::crawl(&client, &o, &CancellationToken::new()).await.unwrap();
    let mut urls: Vec<_> = r.resources.iter().map(|x| x.url.replace(&addr.to_string(), "H")).collect();
    urls.sort();
    assert_eq!(urls, ["http://H/docs/r.pdf", "http://H/i/logo.png", "https://other.example/x.pdf"]);
    assert_eq!(r.pages_blocked_by_robots, 1);
    assert_eq!(r.pages_visited, 2, "start page + a.html; b.html is beyond depth 1");
    let none = gale_core::grabber::GrabOptions { extensions: vec![], ..o };
    assert!(gale_core::grabber::crawl(&client, &none, &CancellationToken::new()).await.is_err());
}
