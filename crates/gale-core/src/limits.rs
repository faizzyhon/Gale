//! Bandwidth and per-host connection limits shared across downloads.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Token bucket in "debt" form: consumers subtract what they used and sleep off any deficit.
/// Concurrent consumers queue up correctly because each one sleeps for the total debt it observed.
pub struct RateLimiter {
    bytes_per_sec: AtomicU64, // 0 = unlimited
    state: Mutex<(f64, Instant)>,
}

impl RateLimiter {
    pub fn new(bytes_per_sec: u64) -> Arc<Self> {
        Arc::new(Self { bytes_per_sec: AtomicU64::new(bytes_per_sec), state: Mutex::new((0.0, Instant::now())) })
    }

    pub fn set_rate(&self, bytes_per_sec: u64) {
        self.bytes_per_sec.store(bytes_per_sec, Ordering::Relaxed);
    }

    pub fn rate(&self) -> u64 {
        self.bytes_per_sec.load(Ordering::Relaxed)
    }

    pub async fn consume(&self, n: u64) {
        let rate = self.rate();
        if rate == 0 {
            return;
        }
        let wait = {
            let mut s = self.state.lock().unwrap();
            let now = Instant::now();
            // Allow at most one second of burst.
            s.0 = (s.0 + now.duration_since(s.1).as_secs_f64() * rate as f64).min(rate as f64);
            s.1 = now;
            s.0 -= n as f64;
            (s.0 < 0.0).then(|| Duration::from_secs_f64(-s.0 / rate as f64))
        };
        if let Some(w) = wait {
            tokio::time::sleep(w).await;
        }
    }
}

/// Caps simultaneous connections to one host across every running download.
pub struct HostLimiter {
    per_host: AtomicUsize, // 0 = unlimited
    hosts: Mutex<HashMap<String, Arc<Semaphore>>>,
}

impl HostLimiter {
    pub fn new(per_host: usize) -> Arc<Self> {
        Arc::new(Self { per_host: AtomicUsize::new(per_host), hosts: Mutex::new(HashMap::new()) })
    }

    /// New limit applies to connections opened from now on.
    pub fn set_limit(&self, per_host: usize) {
        self.per_host.store(per_host, Ordering::Relaxed);
        self.hosts.lock().unwrap().clear();
    }

    pub async fn acquire(&self, host: &str) -> Option<OwnedSemaphorePermit> {
        let limit = self.per_host.load(Ordering::Relaxed);
        if limit == 0 {
            return None;
        }
        let sem = self.hosts.lock().unwrap().entry(host.to_ascii_lowercase()).or_insert_with(|| Arc::new(Semaphore::new(limit))).clone();
        sem.acquire_owned().await.ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn limiter_enforces_rate_across_consumers() {
        let l = RateLimiter::new(100_000);
        l.consume(100_000).await; // burst bucket starts empty → ~1 s
        let start = Instant::now();
        let a = { let l = l.clone(); tokio::spawn(async move { for _ in 0..5 { l.consume(10_000).await } }) };
        let b = { let l = l.clone(); tokio::spawn(async move { for _ in 0..5 { l.consume(10_000).await } }) };
        a.await.unwrap();
        b.await.unwrap();
        let secs = start.elapsed().as_secs_f64();
        assert!((0.85..1.4).contains(&secs), "100 KB at 100 KB/s took {secs:.2}s");
    }

    #[tokio::test]
    async fn unlimited_is_free() {
        let l = RateLimiter::new(0);
        let start = Instant::now();
        l.consume(u64::MAX / 2).await;
        assert!(start.elapsed() < Duration::from_millis(5));
    }

    #[tokio::test]
    async fn host_limiter_caps_per_host() {
        let h = HostLimiter::new(2);
        let p1 = h.acquire("a.com").await;
        let _p2 = h.acquire("A.com").await;
        assert!(tokio::time::timeout(Duration::from_millis(50), h.acquire("a.com")).await.is_err(), "third blocks");
        assert!(h.acquire("b.com").await.is_some(), "other hosts unaffected");
        drop(p1);
        assert!(tokio::time::timeout(Duration::from_millis(50), h.acquire("a.com")).await.is_ok());
    }
}
