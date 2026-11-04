import { useEffect, useMemo, useState } from "react";
import { api } from "../services/api";
import { useDownloads } from "../stores/downloads";
import type { DashboardStats } from "../types";
import { bytes, date, speed } from "../utils/format";

export function DashboardPage() {
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const live = useDownloads((s) => s.live);
  const downloads = useDownloads((s) => s.downloads);
  const aggregate = Object.values(live).reduce((a, l) => a + l.bytesPerSec, 0);

  useEffect(() => {
    let alive = true;
    const load = () =>
      api.dashboard().then(
        (s) => alive && setStats(s),
        (e) => alive && setError(String(e?.message ?? e)),
      );
    load();
    const t = setInterval(load, 5000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, []);

  const recent = useMemo(
    () =>
      Object.values(downloads)
        .filter((d) => d.status === "completed")
        .sort((a, b) => (b.completedAt ?? "").localeCompare(a.completedAt ?? ""))
        .slice(0, 6),
    [downloads],
  );

  if (error) return <p className="p-6 text-bad">{error}</p>;
  if (!stats) return null;

  const days = lastNDays(14).map((day) => [day, stats.dailyBytes.find(([d]) => d === day)?.[1] ?? 0] as const);
  const maxDay = Math.max(1, ...days.map(([, b]) => b));
  const catTotal = stats.perCategory.reduce((a, [, n]) => a + n, 0) || 1;

  return (
    <div className="h-full overflow-auto px-8 py-6">
      <h1 className="mb-6 font-display text-[28px] font-semibold">Dashboard</h1>
      <div className="mb-8 grid grid-cols-3 gap-px overflow-hidden rounded-[var(--radius-surface)] border border-line bg-line xl:grid-cols-6">
        {[
          ["Total", stats.total.toLocaleString()],
          ["Active", stats.active.toLocaleString()],
          ["Completed", stats.completed.toLocaleString()],
          ["Failed", stats.failed.toLocaleString()],
          ["Downloaded", bytes(stats.totalDownloadedBytes)],
          ["Current speed", speed(aggregate)],
        ].map(([k, v]) => (
          <div key={k} className="bg-surface px-4 py-3">
            <div className="text-[12px] text-fg-2">{k}</div>
            <div className="tabular mt-1 font-display text-[22px] font-semibold">{v}</div>
          </div>
        ))}
      </div>

      <div className="grid grid-cols-1 gap-8 xl:grid-cols-[2fr_1fr]">
        <section>
          <h2 className="mb-3 text-[14px] font-semibold">Last 14 days</h2>
          <div className="flex h-40 items-end gap-1.5" role="img" aria-label="Bytes downloaded per day">
            {days.map(([day, b]) => (
              <div key={day} className="group relative flex flex-1 flex-col items-center gap-1">
                <div
                  className="w-full rounded-t-[2px] bg-accent transition-opacity group-hover:opacity-80"
                  style={{ height: `${Math.max(b > 0 ? 3 : 1, (b / maxDay) * 140)}px`, opacity: b > 0 ? 1 : 0.2 }}
                  title={`${day}: ${bytes(b)}`}
                />
                <span className="text-[10px] text-fg-3">{day.slice(8)}</span>
              </div>
            ))}
          </div>
        </section>
        <section>
          <h2 className="mb-3 text-[14px] font-semibold">By category</h2>
          {stats.perCategory.length === 0 ? (
            <p className="text-[12px] text-fg-3">No downloads yet.</p>
          ) : (
            <ul className="space-y-2.5">
              {stats.perCategory.map(([name, n]) => (
                <li key={name}>
                  <div className="mb-1 flex justify-between text-[12px]">
                    <span>{name}</span>
                    <span className="tabular text-fg-2">{n}</span>
                  </div>
                  <div className="h-1 rounded-full bg-line">
                    <div className="h-full rounded-full bg-accent" style={{ width: `${(n / catTotal) * 100}%` }} />
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>

      <section className="mt-8">
        <h2 className="mb-3 text-[14px] font-semibold">Recently downloaded</h2>
        {recent.length === 0 ? (
          <p className="text-[12px] text-fg-3">Completed downloads will appear here.</p>
        ) : (
          <ul className="divide-y divide-line rounded-[var(--radius-surface)] border border-line bg-surface">
            {recent.map((d) => (
              <li key={d.id} className="flex items-center gap-4 px-4 py-2.5 text-[13px]">
                <span className="flex-1 truncate">{d.fileName}</span>
                <span className="tabular text-fg-2">{bytes(d.totalBytes)}</span>
                <span className="w-40 text-right text-fg-3">{date(d.completedAt)}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function lastNDays(n: number): string[] {
  const out: string[] = [];
  const now = new Date();
  for (let i = n - 1; i >= 0; i--) {
    const d = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() - i));
    out.push(d.toISOString().slice(0, 10));
  }
  return out;
}
