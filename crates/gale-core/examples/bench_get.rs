//! Downloads one URL with Gale's engine (the same code the app uses), for benchmarks.
//!   cargo run --release --example bench_get -- <url> <out-file> [connections]
//! Prints: seconds, bytes, MiB/s, sha256.
use std::sync::Arc;
use std::time::Instant;

use gale_core::engine::{self, EngineConfig, Job, Outcome, RequestHeaders, Tick};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let mut a = std::env::args().skip(1);
    let url: url::Url = a.next().expect("url").parse().expect("valid url");
    let out = std::path::PathBuf::from(a.next().expect("output file"));
    let connections: u8 = a.next().and_then(|c| c.parse().ok()).unwrap_or(16);
    let cfg = EngineConfig { connections, compute_sha256: true, max_retries: 20, ..Default::default() };
    let client = engine::build_client(&cfg).unwrap();
    let t0 = Instant::now();
    let probe = engine::probe(&client, &url, &RequestHeaders::default(), None).await.expect("probe");
    let job = Job {
        probe,
        headers: RequestHeaders::default(),
        temp_path: gale_core::security::temp_path_for(&out),
        final_path: out.clone(),
        segments: vec![],
        expected_sha256: None,
    };
    let tick: Arc<dyn Fn(Tick) + Send + Sync> = Arc::new(|_| {});
    match engine::run(&client, &cfg, job, CancellationToken::new(), tick).await {
        Ok(Outcome::Completed { bytes, sha256, .. }) => {
            let s = t0.elapsed().as_secs_f64();
            println!("{s:.2} {bytes} {:.1} {}", bytes as f64 / 1048576.0 / s, sha256.unwrap_or_default());
        }
        Ok(o) => {
            eprintln!("not completed: {o:?}");
            std::process::exit(2);
        }
        Err(e) => {
            eprintln!("failed: {e}");
            std::process::exit(1);
        }
    }
}
