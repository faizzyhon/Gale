//! Test server for benchmarks (not part of Gale). Serves N bytes of fixed pseudo-random data with ranges.
//!   cargo run --release --example bench_server -- <port>
//! URL: /<size_mib>/<mode>/file.bin, mode:
//!   fast          no limits
//!   throttle      each connection limited to 4 MiB/s (what many CDNs and mirrors do)
//!   limit4        each connection 2 MiB/s, and HTTP 503 for more than 4 connections at once
//!   flaky         each 64 KiB chunk has a 2% chance of the connection dropping
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

const CHUNK: usize = 64 * 1024;

struct St {
    data: Bytes, // 64 MiB pattern, repeated to the requested size
    in_flight: Arc<AtomicUsize>,
}

struct Guard(Arc<AtomicUsize>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn byte_at(data: &Bytes, pos: usize, len: usize) -> Bytes {
    let off = pos % data.len();
    let n = len.min(data.len() - off);
    data.slice(off..off + n)
}

async fn serve(State(st): State<Arc<St>>, Path((mib, mode)): Path<(usize, String)>, req: HeaderMap) -> Response {
    let size = mib * 1024 * 1024;
    let range = req
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok()?.strip_prefix("bytes=").map(str::to_owned))
        .and_then(|r| {
            let (s, e) = r.split_once('-')?;
            let s: usize = s.parse().ok()?;
            let e: usize = if e.is_empty() { size - 1 } else { e.parse::<usize>().ok()?.min(size - 1) };
            Some((s, e))
        });
    let n = st.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
    let guard = Guard(st.in_flight.clone());
    if mode == "limit4" && n > 4 {
        return Response::builder().status(503).header(header::RETRY_AFTER, "1").body(Body::empty()).unwrap();
    }
    let (start, end) = range.unwrap_or((0, size - 1));
    let mut b = Response::builder()
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ETAG, format!("\"b{mib}\""))
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, end - start + 1);
    b = if range.is_some() {
        b.status(StatusCode::PARTIAL_CONTENT).header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"))
    } else {
        b.status(StatusCode::OK)
    };
    let rate = match mode.as_str() {
        "throttle" => Some(4.0 * 1024.0 * 1024.0),
        "limit4" => Some(2.0 * 1024.0 * 1024.0),
        _ => None,
    };
    let flaky = mode == "flaky";
    let data = st.data.clone();
    let t0 = Instant::now();
    let body = futures_util::stream::unfold((start, guard, 0u64), move |(pos, guard, sent)| {
        let data = data.clone();
        async move {
            if pos > end {
                return None;
            }
            if flaky && pos > start && fastrand_u32() % 50 == 0 {
                return Some((Err(std::io::Error::other("dropped")), (end + 1, guard, sent)));
            }
            let chunk = byte_at(&data, pos, (end + 1 - pos).min(CHUNK));
            if let Some(r) = rate {
                let due = Duration::from_secs_f64((sent + chunk.len() as u64) as f64 / r);
                if let Some(wait) = due.checked_sub(t0.elapsed()) {
                    tokio::time::sleep(wait).await;
                }
            }
            let len = chunk.len();
            Some((Ok::<_, std::io::Error>(chunk), (pos + len, guard, sent + len as u64)))
        }
    });
    b.body(Body::from_stream(body)).unwrap()
}

fn fastrand_u32() -> u32 {
    use std::cell::Cell;
    thread_local!(static S: Cell<u64> = const { Cell::new(0x9E3779B97F4A7C15) });
    S.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        (x >> 32) as u32
    })
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args().nth(1).and_then(|p| p.parse().ok()).unwrap_or(8900);
    let mut x: u64 = 1;
    let data: Vec<u8> = (0..64 * 1024 * 1024)
        .map(|_| {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (x >> 56) as u8
        })
        .collect();
    let st = Arc::new(St { data: Bytes::from(data), in_flight: Arc::new(AtomicUsize::new(0)) });
    let app = Router::new().route("/{mib}/{mode}/file.bin", get(serve)).with_state(st);
    let l = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    eprintln!("bench server on 127.0.0.1:{port}");
    axum::serve(l, app).await.unwrap();
}
