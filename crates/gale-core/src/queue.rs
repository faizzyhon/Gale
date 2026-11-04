//! Download queues and the scheduler that starts/stops them. All state lives in SQLite.

use chrono::{DateTime, Datelike, Duration as CDuration, Utc, Weekday};
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Queue {
    pub id: i64,
    pub name: String,
    pub priority: i64,
    pub max_concurrent: i64,
    pub max_connections: i64,
    pub state: String,
    pub is_default: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueInput {
    pub name: String,
    pub priority: i64,
    pub max_concurrent: i64,
    pub max_connections: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerJob {
    #[serde(default)]
    pub id: i64,
    pub queue_id: i64,
    /// `start_queue` | `stop_queue`
    pub kind: String,
    /// RFC 3339 UTC.
    pub run_at: String,
    /// None | `daily` | `weekdays` | `weekly:MO,TU,...`
    pub recurrence: Option<String>,
    pub enabled: bool,
    pub shutdown_after: bool,
    #[serde(default)]
    pub last_run_at: Option<String>,
}

impl Db {
    pub async fn list_queues(&self) -> Result<Vec<Queue>> {
        Ok(sqlx::query_as::<_, Queue>("SELECT * FROM queues ORDER BY priority DESC, id").fetch_all(self.pool()).await?)
    }

    pub async fn default_queue_id(&self) -> Result<i64> {
        Ok(sqlx::query_scalar("SELECT id FROM queues WHERE is_default = 1").fetch_one(self.pool()).await?)
    }

    pub async fn save_queue(&self, id: Option<i64>, q: &QueueInput) -> Result<i64> {
        let name = q.name.trim();
        if name.is_empty() || name.len() > 80 {
            return Err(Error::InvalidState("queue name must be 1–80 characters".into()));
        }
        let (mc, mx) = (q.max_concurrent.clamp(1, 32), q.max_connections.clamp(1, 32));
        let r = match id {
            Some(id) => sqlx::query("UPDATE queues SET name = ?, priority = ?, max_concurrent = ?, max_connections = ? WHERE id = ?")
                .bind(name).bind(q.priority).bind(mc).bind(mx).bind(id)
                .execute(self.pool()).await.map(|_| id),
            None => sqlx::query("INSERT INTO queues (name, priority, max_concurrent, max_connections) VALUES (?, ?, ?, ?)")
                .bind(name).bind(q.priority).bind(mc).bind(mx)
                .execute(self.pool()).await.map(|r| r.last_insert_rowid()),
        };
        r.map_err(|e| match e {
            sqlx::Error::Database(d) if d.is_unique_violation() => Error::InvalidState(format!("a queue named \"{name}\" already exists")),
            e => e.into(),
        })
    }

    /// Items go back to "no queue"; the downloads themselves are kept.
    pub async fn delete_queue(&self, id: i64) -> Result<()> {
        let n = sqlx::query("DELETE FROM queues WHERE id = ? AND is_default = 0").bind(id).execute(self.pool()).await?.rows_affected();
        if n == 0 {
            return Err(Error::InvalidState("the main queue cannot be deleted".into()));
        }
        Ok(())
    }

    pub async fn set_queue_state(&self, id: i64, state: &str) -> Result<()> {
        sqlx::query("UPDATE queues SET state = ? WHERE id = ?").bind(state).bind(id).execute(self.pool()).await?;
        Ok(())
    }

    /// Moves a download into a queue (removing it from any other) at the end.
    pub async fn enqueue(&self, queue_id: i64, download_id: i64) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("DELETE FROM queue_items WHERE download_id = ?").bind(download_id).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO queue_items (queue_id, download_id, position)
             VALUES (?, ?, COALESCE((SELECT MAX(position) + 1 FROM queue_items WHERE queue_id = ?), 0))",
        )
        .bind(queue_id).bind(download_id).bind(queue_id)
        .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn dequeue(&self, download_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM queue_items WHERE download_id = ?").bind(download_id).execute(self.pool()).await?;
        Ok(())
    }

    /// Map of download id → queue id, for the UI.
    pub async fn queue_memberships(&self) -> Result<Vec<(i64, i64)>> {
        Ok(sqlx::query_as("SELECT download_id, queue_id FROM queue_items").fetch_all(self.pool()).await?)
    }

    /// (download_id, status) of every item in a queue, in queue order.
    pub async fn queue_items(&self, queue_id: i64) -> Result<Vec<(i64, String)>> {
        Ok(sqlx::query_as(
            "SELECT d.id, d.status FROM queue_items qi JOIN downloads d ON d.id = qi.download_id
             WHERE qi.queue_id = ? ORDER BY qi.position",
        )
        .bind(queue_id)
        .fetch_all(self.pool())
        .await?)
    }

    pub async fn list_jobs(&self) -> Result<Vec<SchedulerJob>> {
        Ok(sqlx::query_as::<_, SchedulerJob>("SELECT * FROM scheduler_jobs ORDER BY run_at").fetch_all(self.pool()).await?)
    }

    pub async fn save_job(&self, j: &SchedulerJob) -> Result<i64> {
        DateTime::parse_from_rfc3339(&j.run_at).map_err(|_| Error::InvalidState("invalid date/time".into()))?;
        if !matches!(j.kind.as_str(), "start_queue" | "stop_queue") {
            return Err(Error::InvalidState("unknown job kind".into()));
        }
        parse_recurrence(j.recurrence.as_deref())?;
        let run_at = normalize(&j.run_at);
        if j.id > 0 {
            sqlx::query("UPDATE scheduler_jobs SET queue_id = ?, kind = ?, run_at = ?, recurrence = ?, enabled = ?, shutdown_after = ? WHERE id = ?")
                .bind(j.queue_id).bind(&j.kind).bind(&run_at).bind(&j.recurrence).bind(j.enabled).bind(j.shutdown_after).bind(j.id)
                .execute(self.pool()).await?;
            Ok(j.id)
        } else {
            Ok(sqlx::query("INSERT INTO scheduler_jobs (queue_id, kind, run_at, recurrence, enabled, shutdown_after) VALUES (?, ?, ?, ?, ?, ?)")
                .bind(j.queue_id).bind(&j.kind).bind(&run_at).bind(&j.recurrence).bind(j.enabled).bind(j.shutdown_after)
                .execute(self.pool()).await?
                .last_insert_rowid())
        }
    }

    pub async fn delete_job(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM scheduler_jobs WHERE id = ?").bind(id).execute(self.pool()).await?;
        Ok(())
    }

    /// Jobs whose time has come. Each is advanced (or disabled) in the same transaction it is claimed in.
    pub async fn take_due_jobs(&self, now: DateTime<Utc>) -> Result<Vec<SchedulerJob>> {
        let mut tx = self.pool().begin().await?;
        let due: Vec<SchedulerJob> = sqlx::query_as("SELECT * FROM scheduler_jobs WHERE enabled = 1 AND run_at <= ?")
            .bind(normalize(&now.to_rfc3339()))
            .fetch_all(&mut *tx)
            .await?;
        for j in &due {
            let next = next_run(&j.run_at, j.recurrence.as_deref(), now)?;
            sqlx::query("UPDATE scheduler_jobs SET last_run_at = ?, run_at = COALESCE(?, run_at), enabled = ? WHERE id = ?")
                .bind(normalize(&now.to_rfc3339()))
                .bind(next.map(|n| normalize(&n.to_rfc3339())))
                .bind(next.is_some())
                .bind(j.id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(due)
    }
}

/// Fixed-width UTC so string comparison in SQLite orders correctly.
fn normalize(rfc3339: &str) -> String {
    DateTime::parse_from_rfc3339(rfc3339)
        .map(|d| d.with_timezone(&Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|_| rfc3339.to_string())
}

fn parse_recurrence(r: Option<&str>) -> Result<Option<Vec<Weekday>>> {
    let all = || vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri, Weekday::Sat, Weekday::Sun];
    Ok(match r {
        None => None,
        Some("daily") => Some(all()),
        Some("weekdays") => Some(all()[..5].to_vec()),
        Some(w) if w.starts_with("weekly:") => {
            let days = w[7..]
                .split(',')
                .map(|d| match d.trim() {
                    "MO" => Ok(Weekday::Mon),
                    "TU" => Ok(Weekday::Tue),
                    "WE" => Ok(Weekday::Wed),
                    "TH" => Ok(Weekday::Thu),
                    "FR" => Ok(Weekday::Fri),
                    "SA" => Ok(Weekday::Sat),
                    "SU" => Ok(Weekday::Sun),
                    _ => Err(Error::InvalidState(format!("bad weekday in {w}"))),
                })
                .collect::<Result<Vec<_>>>()?;
            if days.is_empty() {
                return Err(Error::InvalidState("pick at least one day".into()));
            }
            Some(days)
        }
        Some(o) => return Err(Error::InvalidState(format!("unknown recurrence {o}"))),
    })
}

/// Next occurrence strictly after `now`, keeping the original time of day (UTC). None = one-shot.
pub fn next_run(run_at: &str, recurrence: Option<&str>, now: DateTime<Utc>) -> Result<Option<DateTime<Utc>>> {
    let Some(days) = parse_recurrence(recurrence)? else { return Ok(None) };
    let mut t = DateTime::parse_from_rfc3339(run_at).map_err(|_| Error::InvalidState("invalid date/time".into()))?.with_timezone(&Utc);
    // shortcut: weekdays are evaluated in UTC; a job near midnight local time may land on the adjacent day.
    while t <= now || !days.contains(&t.weekday()) {
        t += CDuration::days(1);
    }
    Ok(Some(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn recurrence_rules() {
        let now = at("2026-11-06T09:00:00Z"); // Friday
        assert_eq!(next_run("2026-11-06T08:00:00Z", None, now).unwrap(), None);
        assert_eq!(next_run("2026-11-06T08:00:00Z", Some("daily"), now).unwrap(), Some(at("2026-11-07T08:00:00Z")));
        assert_eq!(next_run("2026-11-06T08:00:00Z", Some("weekdays"), now).unwrap(), Some(at("2026-11-09T08:00:00Z")));
        assert_eq!(next_run("2026-11-06T08:00:00Z", Some("weekly:TU,TH"), now).unwrap(), Some(at("2026-11-10T08:00:00Z")));
        // App was off for days: catches up to the next future slot, not every missed one.
        assert_eq!(next_run("2026-10-01T08:00:00Z", Some("daily"), now).unwrap(), Some(at("2026-11-07T08:00:00Z")));
        assert!(next_run("2026-11-06T08:00:00Z", Some("weekly:XX"), now).is_err());
    }

    #[tokio::test]
    async fn queues_and_due_jobs_persist() {
        let db = Db::open_in_memory().await.unwrap();
        let q = db.save_queue(None, &QueueInput { name: "Night".into(), priority: 1, max_concurrent: 2, max_connections: 4 }).await.unwrap();
        assert!(db.save_queue(None, &QueueInput { name: "Night".into(), priority: 0, max_concurrent: 1, max_connections: 1 }).await.is_err());
        assert!(db.delete_queue(db.default_queue_id().await.unwrap()).await.is_err());

        let job = SchedulerJob {
            id: 0, queue_id: q, kind: "start_queue".into(), run_at: "2026-11-06T08:00:00+03:00".into(),
            recurrence: Some("daily".into()), enabled: true, shutdown_after: false, last_run_at: None,
        };
        let jid = db.save_job(&job).await.unwrap();
        assert!(db.take_due_jobs(at("2026-11-06T04:59:00Z")).await.unwrap().is_empty());
        let due = db.take_due_jobs(at("2026-11-06T05:00:30Z")).await.unwrap();
        assert_eq!(due.len(), 1);
        assert!(db.take_due_jobs(at("2026-11-06T05:01:00Z")).await.unwrap().is_empty(), "claimed once");
        let j = db.list_jobs().await.unwrap().into_iter().find(|j| j.id == jid).unwrap();
        assert_eq!((j.run_at.as_str(), j.enabled), ("2026-11-07T05:00:00Z", true));
    }
}
