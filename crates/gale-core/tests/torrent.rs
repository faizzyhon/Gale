//! End to end: a local seeder and a minimal HTTP tracker on 127.0.0.1, downloaded through the Manager.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::routing::get;
use axum::Router;
use gale_core::db::{Db, Download, DownloadStatus};
use gale_core::engine::{EngineConfig, Tick};
use gale_core::manager::{AddOptions, EventSink, Manager};
use librqbit::{AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerOptions, Session, SessionOptions};

struct NoEvents;
impl EventSink for NoEvents {
    fn progress(&self, _: i64, _: &Tick) {}
    fn changed(&self, _: &Download) {}
}

/// Answers every announce with the seeder's address (compact peer list).
async fn tracker(peer: Arc<Mutex<Option<SocketAddr>>>) -> SocketAddr {
    let app = Router::new().route(
        "/announce",
        get(move || {
            let peer = peer.clone();
            async move {
                let mut body = b"d8:intervali5e5:peers".to_vec();
                match *peer.lock().unwrap() {
                    Some(SocketAddr::V4(a)) => {
                        body.extend_from_slice(b"6:");
                        body.extend_from_slice(&a.ip().octets());
                        body.extend_from_slice(&a.port().to_be_bytes());
                    }
                    _ => body.extend_from_slice(b"0:"),
                }
                body.push(b'e');
                body
            }
        }),
    );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    addr
}

fn data(len: usize, seed: u8) -> Vec<u8> {
    (0..len).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
}

async fn wait_for(m: &Manager, id: i64, pred: impl Fn(&Download) -> bool) -> Download {
    for _ in 0..1200 {
        let d = m.db().get_download(id).await.unwrap();
        if pred(&d) {
            return d;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timed out; last state {:?}", m.db().get_download(id).await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn downloads_a_torrent_from_a_local_seeder() {
    let tmp = tempfile::tempdir().unwrap();
    let seed_root = tmp.path().join("seed");
    let pack = seed_root.join("Pack");
    std::fs::create_dir_all(pack.join("sub")).unwrap();
    let (a, b) = (data(300_000, 1), data(200_000, 7));
    std::fs::write(pack.join("a.bin"), &a).unwrap();
    std::fs::write(pack.join("sub").join("b.bin"), &b).unwrap();

    let peer = Arc::new(Mutex::new(None));
    let tr = tracker(peer.clone()).await;
    let spawner = librqbit::spawn_utils::BlockingSpawner::new(2);
    let created = librqbit::create_torrent(
        &pack,
        CreateTorrentOptions { name: None, trackers: vec![format!("http://{tr}/announce")], piece_length: Some(16384) },
        &spawner,
    )
    .await
    .unwrap();
    let torrent_bytes = created.as_bytes().unwrap();

    let seeder = Session::new_with_opts(
        seed_root.clone(),
        SessionOptions {
            dht: None,
            disable_local_service_discovery: true,
            listen: Some(ListenerOptions { listen_addr: "127.0.0.1:0".parse().unwrap(), ipv4_only: true, ..Default::default() }),
            ipv4_only: true,
            ..Default::default()
        },
    )
    .await;
    let seeder = match seeder {
        // librqbit's UDP tracker socket is IPv6 dual-stack; Windows always has it, some Linux sandboxes do not.
        Err(e) if format!("{e:#}").contains("Address family not supported") => return eprintln!("skipped: no IPv6 in this environment"),
        r => r.unwrap(),
    };
    let h = seeder
        .add_torrent(
            AddTorrent::from_bytes(torrent_bytes.clone()),
            Some(AddTorrentOptions { output_folder: Some(pack.to_string_lossy().into()), overwrite: true, ..Default::default() }),
        )
        .await
        .unwrap()
        .into_handle()
        .unwrap();
    h.wait_until_completed().await.unwrap();
    let port = seeder.listen_addr().expect("seeder listens").port();
    *peer.lock().unwrap() = Some(SocketAddr::from(([127, 0, 0, 1], port)));

    let torrent_file = tmp.path().join("pack.torrent");
    std::fs::write(&torrent_file, &torrent_bytes).unwrap();
    let file_url = url::Url::from_file_path(&torrent_file).unwrap().to_string();
    let dl = tmp.path().join("dl");
    let m = Manager::new(Db::open(&tmp.path().join("gale.db")).await.unwrap(), EngineConfig::default(), 3, Arc::new(NoEvents)).unwrap();
    let opts = |url: &str| AddOptions { url: url.into(), save_dir: dl.to_string_lossy().into(), start: true, ..Default::default() };

    let id = m.add(opts(&file_url)).await.unwrap();
    assert_eq!(m.db().get_download(id).await.unwrap().kind, "torrent");
    let d = wait_for(&m, id, |d| matches!(d.status, DownloadStatus::Completed | DownloadStatus::Failed)).await;
    assert_eq!(d.status, DownloadStatus::Completed, "{:?}", d.error_message);
    assert_eq!(d.file_name, "Pack");
    assert_eq!(d.downloaded_bytes, 500_000);
    assert_eq!(Path::new(d.final_path.as_deref().unwrap()), dl.join("Pack"));
    assert_eq!(std::fs::read(dl.join("Pack").join("a.bin")).unwrap(), a);
    assert_eq!(std::fs::read(dl.join("Pack").join("sub").join("b.bin")).unwrap(), b);

    // The same torrent again must not write into the finished folder.
    let id2 = m.add(opts(&file_url)).await.unwrap();
    let d2 = wait_for(&m, id2, |d| matches!(d.status, DownloadStatus::Completed | DownloadStatus::Failed)).await;
    assert_eq!(d2.status, DownloadStatus::Completed, "{:?}", d2.error_message);
    assert_eq!(Path::new(d2.final_path.as_deref().unwrap()), dl.join("Pack (1)"));
    assert_eq!(std::fs::read(dl.join("Pack (1)").join("a.bin")).unwrap(), a);

    // A magnet link: the metadata comes from the seeder, found through the tracker in `tr=`.
    let magnet = format!("magnet:?xt=urn:btih:{}&dn=Pack&tr={}", created.info_hash().as_string(), url::form_urlencoded::byte_serialize(format!("http://{tr}/announce").as_bytes()).collect::<String>());
    let dl3 = tmp.path().join("dl3");
    let id3 = m.add(AddOptions { url: magnet, save_dir: dl3.to_string_lossy().into(), start: true, ..Default::default() }).await.unwrap();
    let d3 = wait_for(&m, id3, |d| matches!(d.status, DownloadStatus::Completed | DownloadStatus::Failed)).await;
    assert_eq!(d3.status, DownloadStatus::Completed, "{:?}", d3.error_message);
    assert_eq!(std::fs::read(dl3.join("Pack").join("sub").join("b.bin")).unwrap(), b);

    // Pause part-way, then resume: pieces already on disk are kept and the files come out whole.
    let dl2 = tmp.path().join("dl2");
    let slow = EngineConfig { download_limit_bps: 40_000, ..Default::default() };
    let m2 = Manager::new(Db::open(&tmp.path().join("gale2.db")).await.unwrap(), slow, 3, Arc::new(NoEvents)).unwrap();
    let id = m2
        .add(AddOptions { url: file_url.clone(), save_dir: dl2.to_string_lossy().into(), start: true, ..Default::default() })
        .await
        .unwrap();
    wait_for(&m2, id, |d| d.downloaded_bytes >= 50_000).await;
    m2.stop_and_wait(id).await.unwrap();
    let paused = m2.db().get_download(id).await.unwrap();
    assert_eq!(paused.status, DownloadStatus::Paused);
    assert!(paused.downloaded_bytes < 500_000, "finished before the pause");
    m2.start(id).await.unwrap();
    let d = wait_for(&m2, id, |d| matches!(d.status, DownloadStatus::Completed | DownloadStatus::Failed)).await;
    assert_eq!(d.status, DownloadStatus::Completed, "{:?}", d.error_message);
    assert_eq!(std::fs::read(dl2.join("Pack").join("a.bin")).unwrap(), a);
    assert_eq!(std::fs::read(dl2.join("Pack").join("sub").join("b.bin")).unwrap(), b);
}

#[tokio::test]
async fn rebuild_keeps_rows_and_children() {
    // Db::open on a database from 0.4.0 rebuilds `downloads`; segments and metadata must survive it.
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("gale.db");
    let db = Db::open(&path).await.unwrap();
    let id = db
        .insert_download(&gale_core::db::NewDownload {
            url: "https://example.com/a.zip".into(),
            file_name: "a.zip".into(),
            save_dir: "/tmp".into(),
            connections: 4,
            ..Default::default()
        })
        .await
        .unwrap();
    db.set_metadata(id, "k", "v").await.unwrap();
    // Put the old CHECK back, as a 0.4.0 database has it.
    {
        let mut c = db.pool().acquire().await.unwrap();
        sqlx::query("PRAGMA writable_schema = ON").execute(&mut *c).await.unwrap();
        sqlx::query("UPDATE sqlite_master SET sql = replace(sql, ', ''torrent'')', ')') WHERE name = 'downloads'").execute(&mut *c).await.unwrap();
    }
    db.pool().close().await;
    drop(db);

    let db = Db::open(&path).await.unwrap();
    let sql: String = sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE name = 'downloads'").fetch_one(db.pool()).await.unwrap();
    assert!(sql.contains("'torrent'"));
    assert_eq!(db.get_metadata(id, "k").await.unwrap().as_deref(), Some("v"));
    assert_eq!(db.get_download(id).await.unwrap().file_name, "a.zip");
    let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys").fetch_one(db.pool()).await.unwrap();
    assert_eq!(fk, 1);
}
