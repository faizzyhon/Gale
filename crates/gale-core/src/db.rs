//! SQLite persistence. All queries are parameterised; secrets never reach this layer.

use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::categories::Category;
use crate::engine::SegmentState;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Probing,
    Downloading,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Scheduled,
}

impl DownloadStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Probing | Self::Downloading)
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    pub id: i64,
    pub url: String,
    pub final_url: Option<String>,
    pub referrer: Option<String>,
    pub file_name: String,
    pub save_dir: String,
    pub temp_path: Option<String>,
    pub final_path: Option<String>,
    pub mime_type: Option<String>,
    pub total_bytes: Option<i64>,
    pub downloaded_bytes: i64,
    pub status: DownloadStatus,
    pub resumable: bool,
    pub connections: i64,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub sha256: Option<String>,
    pub expected_sha256: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub category_id: Option<i64>,
    pub source: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDownload {
    pub url: String,
    pub referrer: Option<String>,
    pub file_name: String,
    pub save_dir: String,
    pub connections: i64,
    pub category_id: Option<i64>,
    pub expected_sha256: Option<String>,
    pub source: Option<String>,
    pub status: Option<DownloadStatus>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub total: i64,
    pub active: i64,
    pub completed: i64,
    pub failed: i64,
    pub total_downloaded_bytes: i64,
    pub per_category: Vec<(String, i64)>,
    /// (YYYY-MM-DD, bytes) for the last 14 days, from download_history.
    pub daily_bytes: Vec<(String, i64)>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub download_id: Option<i64>,
    pub url: String,
    pub file_name: String,
    pub final_path: Option<String>,
    pub bytes: i64,
    pub outcome: String,
    pub duration_ms: Option<i64>,
    pub completed_at: String,
}

#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

impl Db {
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(5));
        Self::connect(opts, 8).await
    }

    pub async fn open_in_memory() -> Result<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        // One connection: every in-memory connection is a separate database.
        Self::connect(opts, 1).await
    }

    async fn connect(opts: SqliteConnectOptions, max: u32) -> Result<Self> {
        let pool = SqlitePoolOptions::new().max_connections(max).connect_with(opts).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Crash/restart recovery: anything that was mid-transfer becomes paused (resumable) on startup.
    pub async fn recover_interrupted(&self) -> Result<u64> {
        let r = sqlx::query(&format!(
            "UPDATE downloads SET status = 'paused', updated_at = {NOW} WHERE status IN ('probing', 'downloading')"
        ))
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected())
    }

    pub async fn insert_download(&self, d: &NewDownload) -> Result<i64> {
        let status = d.status.unwrap_or(DownloadStatus::Queued);
        let id = sqlx::query(
            "INSERT INTO downloads (url, referrer, file_name, save_dir, connections, category_id, expected_sha256, source, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&d.url)
        .bind(&d.referrer)
        .bind(&d.file_name)
        .bind(&d.save_dir)
        .bind(d.connections.clamp(1, 32))
        .bind(d.category_id)
        .bind(&d.expected_sha256)
        .bind(d.source.as_deref().unwrap_or("manual"))
        .bind(status)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    pub async fn get_download(&self, id: i64) -> Result<Download> {
        sqlx::query_as::<_, Download>("SELECT * FROM downloads WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(Error::NotFound(id))
    }

    pub async fn list_downloads(&self) -> Result<Vec<Download>> {
        Ok(sqlx::query_as::<_, Download>("SELECT * FROM downloads ORDER BY created_at DESC, id DESC")
            .fetch_all(&self.pool)
            .await?)
    }

    /// True when a non-finished download for the same URL already exists (duplicate guard).
    pub async fn find_unfinished_by_url(&self, url: &str) -> Result<Option<i64>> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM downloads WHERE url = ? AND status NOT IN ('completed','failed','cancelled') LIMIT 1",
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn set_status(&self, id: i64, status: DownloadStatus, err: Option<&Error>) -> Result<()> {
        sqlx::query(&format!(
            "UPDATE downloads SET status = ?, error_code = ?, error_message = ?,
                    started_at = CASE WHEN ? = 'downloading' AND started_at IS NULL THEN {NOW} ELSE started_at END,
                    updated_at = {NOW} WHERE id = ?"
        ))
        .bind(status)
        .bind(err.map(Error::code))
        .bind(err.map(|e| e.to_string()))
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn save_probe(
        &self,
        id: i64,
        final_url: &str,
        file_name: &str,
        temp_path: &str,
        final_path: &str,
        mime: Option<&str>,
        total: Option<u64>,
        resumable: bool,
        etag: Option<&str>,
        last_modified: Option<&str>,
        category_id: Option<i64>,
    ) -> Result<()> {
        sqlx::query(&format!(
            "UPDATE downloads SET final_url = ?, file_name = ?, temp_path = ?, final_path = ?, mime_type = ?,
                    total_bytes = ?, resumable = ?, etag = ?, last_modified = ?,
                    category_id = COALESCE(?, category_id), updated_at = {NOW} WHERE id = ?"
        ))
        .bind(final_url)
        .bind(file_name)
        .bind(temp_path)
        .bind(final_path)
        .bind(mime)
        .bind(total.map(|t| t as i64))
        .bind(resumable)
        .bind(etag)
        .bind(last_modified)
        .bind(category_id)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Atomically persist segment layout + progress so a crash leaves a consistent resume point.
    pub async fn save_progress(&self, id: i64, segments: &[SegmentState]) -> Result<()> {
        let written: u64 = segments.iter().map(|s| s.written).sum();
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM download_segments WHERE download_id = ?").bind(id).execute(&mut *tx).await?;
        for s in segments {
            sqlx::query("INSERT INTO download_segments (download_id, idx, start_byte, end_byte, written) VALUES (?, ?, ?, ?, ?)")
                .bind(id)
                .bind(s.index as i64)
                .bind(s.start as i64)
                .bind(s.end.map(|e| e as i64))
                .bind(s.written as i64)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query(&format!("UPDATE downloads SET downloaded_bytes = ?, updated_at = {NOW} WHERE id = ?"))
            .bind(written as i64)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn load_segments(&self, id: i64) -> Result<Vec<SegmentState>> {
        let rows: Vec<(i64, i64, Option<i64>, i64)> =
            sqlx::query_as("SELECT idx, start_byte, end_byte, written FROM download_segments WHERE download_id = ? ORDER BY idx")
                .bind(id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows
            .into_iter()
            .map(|(i, s, e, w)| SegmentState { index: i as u32, start: s as u64, end: e.map(|e| e as u64), written: w as u64 })
            .collect())
    }

    /// Clears progress so the next start begins from byte 0 (restart, or remote file changed).
    pub async fn reset_progress(&self, id: i64) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM download_segments WHERE download_id = ?").bind(id).execute(&mut *tx).await?;
        sqlx::query(&format!(
            "UPDATE downloads SET downloaded_bytes = 0, etag = NULL, last_modified = NULL, sha256 = NULL,
                    error_code = NULL, error_message = NULL, completed_at = NULL, updated_at = {NOW} WHERE id = ?"
        ))
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Only called after the engine verified size (and checksum) and renamed the file into place.
    pub async fn mark_completed(&self, id: i64, final_path: &str, bytes: u64, sha256: Option<&str>) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(&format!(
            "UPDATE downloads SET status = 'completed', final_path = ?, file_name = ?, downloaded_bytes = ?, total_bytes = ?,
                    sha256 = ?, temp_path = NULL, error_code = NULL, error_message = NULL,
                    completed_at = {NOW}, updated_at = {NOW} WHERE id = ?"
        ))
        .bind(final_path)
        .bind(Path::new(final_path).file_name().map(|n| n.to_string_lossy().into_owned()))
        .bind(bytes as i64)
        .bind(bytes as i64)
        .bind(sha256)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM download_segments WHERE download_id = ?").bind(id).execute(&mut *tx).await?;
        insert_history(&mut tx, id, "completed").await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_failed(&self, id: i64, err: &Error) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(&format!(
            "UPDATE downloads SET status = 'failed', error_code = ?, error_message = ?, updated_at = {NOW} WHERE id = ?"
        ))
        .bind(err.code())
        .bind(err.to_string())
        .bind(id)
        .execute(&mut *tx)
        .await?;
        insert_history(&mut tx, id, "failed").await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_download(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM downloads WHERE id = ?").bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn list_categories(&self) -> Result<Vec<Category>> {
        Ok(sqlx::query_as::<_, Category>("SELECT * FROM download_categories ORDER BY sort_order")
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn list_history(&self, limit: i64) -> Result<Vec<HistoryEntry>> {
        Ok(sqlx::query_as::<_, HistoryEntry>(
            "SELECT id, download_id, url, file_name, final_path, bytes, outcome, duration_ms, completed_at
             FROM download_history ORDER BY completed_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn dashboard(&self) -> Result<DashboardStats> {
        let (total, active, completed, failed, bytes): (i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT COUNT(*),
                    COALESCE(SUM(status IN ('probing','downloading')), 0),
                    COALESCE(SUM(status = 'completed'), 0),
                    COALESCE(SUM(status = 'failed'), 0),
                    COALESCE(SUM(downloaded_bytes), 0)
             FROM downloads",
        )
        .fetch_one(&self.pool)
        .await?;
        let per_category = sqlx::query_as(
            "SELECT COALESCE(c.name, 'Other'), COUNT(*) FROM downloads d
             LEFT JOIN download_categories c ON c.id = d.category_id GROUP BY 1 ORDER BY 2 DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        let daily_bytes = sqlx::query_as(
            "SELECT substr(completed_at, 1, 10) AS day, SUM(bytes) FROM download_history
             WHERE outcome = 'completed' AND completed_at >= strftime('%Y-%m-%d', 'now', '-13 days')
             GROUP BY day ORDER BY day",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(DashboardStats { total, active, completed, failed, total_downloaded_bytes: bytes, per_category, daily_bytes })
    }

    pub async fn get_setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> = sqlx::query_scalar("SELECT value FROM application_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(raw.and_then(|r| serde_json::from_str(&r).ok()))
    }

    pub async fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value).map_err(|e| Error::InvalidState(e.to_string()))?;
        sqlx::query(&format!(
            "INSERT INTO application_settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = {NOW}"
        ))
        .bind(key)
        .bind(json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Consistent online snapshot of the whole database.
    pub async fn backup_to(&self, dest: &Path) -> Result<()> {
        if dest.exists() {
            return Err(Error::InvalidState(format!("{} already exists", dest.display())));
        }
        sqlx::query("VACUUM INTO ?").bind(dest.to_string_lossy().as_ref()).execute(&self.pool).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: i64,
    pub level: String,
    pub title: String,
    pub body: Option<String>,
    pub download_id: Option<i64>,
    pub is_read: bool,
    pub created_at: String,
}

impl Db {
    pub async fn add_notification(&self, level: &str, title: &str, body: Option<&str>, download_id: Option<i64>) -> Result<i64> {
        Ok(sqlx::query("INSERT INTO notifications (level, title, body, download_id) VALUES (?, ?, ?, ?)")
            .bind(level).bind(title).bind(body).bind(download_id)
            .execute(&self.pool).await?
            .last_insert_rowid())
    }

    pub async fn list_notifications(&self, limit: i64) -> Result<Vec<Notification>> {
        Ok(sqlx::query_as::<_, Notification>("SELECT * FROM notifications ORDER BY id DESC LIMIT ?")
            .bind(limit).fetch_all(&self.pool).await?)
    }

    pub async fn mark_notifications_read(&self) -> Result<()> {
        sqlx::query("UPDATE notifications SET is_read = 1 WHERE is_read = 0").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn add_browser_event(&self, kind: &str, url: Option<&str>, page_url: Option<&str>, download_id: Option<i64>) -> Result<()> {
        sqlx::query("INSERT INTO browser_events (kind, url, page_url, download_id) VALUES (?, ?, ?, ?)")
            .bind(kind).bind(url).bind(page_url).bind(download_id)
            .execute(&self.pool).await?;
        // Keep the log bounded.
        sqlx::query("DELETE FROM browser_events WHERE id <= (SELECT MAX(id) - 5000 FROM browser_events)").execute(&self.pool).await?;
        Ok(())
    }

    /// (last contact time, number of intercepted downloads) for Settings → Browser.
    pub async fn browser_activity(&self) -> Result<(Option<String>, i64)> {
        Ok(sqlx::query_as("SELECT MAX(created_at), COALESCE(SUM(kind = 'intercepted'), 0) FROM browser_events").fetch_one(&self.pool).await?)
    }

    /// Restore from a backup made with `backup_to`: validated here, swapped in at next start.
    pub async fn check_backup(path: &Path) -> Result<()> {
        let opts = SqliteConnectOptions::new().filename(path).read_only(true);
        let pool = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
        let ok: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&pool).await?;
        let has: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE name = 'downloads'").fetch_one(&pool).await?;
        pool.close().await;
        if ok != "ok" || has != 1 {
            return Err(Error::InvalidState("this file is not a valid Gale backup".into()));
        }
        Ok(())
    }
}

async fn insert_history(tx: &mut sqlx::SqliteConnection, id: i64, outcome: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO download_history (download_id, url, file_name, final_path, bytes, outcome, category_id, duration_ms)
         SELECT id, url, file_name, final_path, downloaded_bytes, ?, category_id,
                CAST((julianday('now') - julianday(COALESCE(started_at, created_at))) * 86400000 AS INTEGER)
         FROM downloads WHERE id = ?",
    )
    .bind(outcome)
    .bind(id)
    .execute(tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(url: &str) -> NewDownload {
        NewDownload { url: url.into(), file_name: "f.bin".into(), save_dir: "/tmp".into(), connections: 8, ..Default::default() }
    }

    #[tokio::test]
    async fn migrations_seed_defaults() {
        let db = Db::open_in_memory().await.unwrap();
        assert_eq!(db.list_categories().await.unwrap().len(), 7);
        let q: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM queues WHERE is_default = 1").fetch_one(db.pool()).await.unwrap();
        assert_eq!(q, 1);
    }

    #[tokio::test]
    async fn progress_roundtrip_and_completion_writes_history() {
        let db = Db::open_in_memory().await.unwrap();
        let id = db.insert_download(&new("https://h/f.bin")).await.unwrap();
        let segs = vec![
            SegmentState { index: 0, start: 0, end: Some(50), written: 50 },
            SegmentState { index: 1, start: 50, end: Some(100), written: 10 },
        ];
        db.save_progress(id, &segs).await.unwrap();
        assert_eq!(db.load_segments(id).await.unwrap(), segs);
        assert_eq!(db.get_download(id).await.unwrap().downloaded_bytes, 60);

        db.mark_completed(id, "/tmp/f.bin", 100, Some("ab")).await.unwrap();
        let d = db.get_download(id).await.unwrap();
        assert_eq!(d.status, DownloadStatus::Completed);
        assert!(db.load_segments(id).await.unwrap().is_empty());
        let h = db.list_history(10).await.unwrap();
        assert_eq!((h.len(), h[0].bytes, h[0].outcome.as_str()), (1, 100, "completed"));
        let stats = db.dashboard().await.unwrap();
        assert_eq!((stats.total, stats.completed, stats.daily_bytes.len()), (1, 1, 1));
    }

    #[tokio::test]
    async fn interrupted_downloads_become_paused() {
        let db = Db::open_in_memory().await.unwrap();
        let id = db.insert_download(&new("https://h/a")).await.unwrap();
        db.set_status(id, DownloadStatus::Downloading, None).await.unwrap();
        assert_eq!(db.recover_interrupted().await.unwrap(), 1);
        assert_eq!(db.get_download(id).await.unwrap().status, DownloadStatus::Paused);
    }

    #[tokio::test]
    async fn duplicate_guard_and_cascade() {
        let db = Db::open_in_memory().await.unwrap();
        let id = db.insert_download(&new("https://h/dup")).await.unwrap();
        assert_eq!(db.find_unfinished_by_url("https://h/dup").await.unwrap(), Some(id));
        db.save_progress(id, &[SegmentState { index: 0, start: 0, end: None, written: 5 }]).await.unwrap();
        db.delete_download(id).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_segments").fetch_one(db.pool()).await.unwrap();
        assert_eq!(n, 0, "segments cascade-deleted");
    }

    #[tokio::test]
    async fn settings_roundtrip() {
        let db = Db::open_in_memory().await.unwrap();
        db.set_setting("max_connections", &16u8).await.unwrap();
        db.set_setting("max_connections", &32u8).await.unwrap();
        assert_eq!(db.get_setting::<u8>("max_connections").await.unwrap(), Some(32));
        assert_eq!(db.get_setting::<u8>("missing").await.unwrap(), None);
    }
}
