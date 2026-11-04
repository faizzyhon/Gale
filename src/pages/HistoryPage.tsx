import { useEffect, useState } from "react";
import { api } from "../services/api";
import type { HistoryEntry } from "../types";
import { bytes, date } from "../utils/format";

export function HistoryPage() {
  const [rows, setRows] = useState<HistoryEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.history(1000).then(setRows, (e) => setError(String(e?.message ?? e)));
  }, []);

  return (
    <div className="flex h-full flex-col">
      <h1 className="px-8 pt-6 pb-4 font-display text-[28px] font-semibold">Download history</h1>
      {error && <p className="px-8 text-bad">{error}</p>}
      <div className="min-h-0 flex-1 overflow-auto px-5">
        {rows && rows.length === 0 && <p className="px-3 text-[13px] text-fg-3">Finished downloads are recorded here, even after you remove them from the list.</p>}
        {rows && rows.length > 0 && (
          <table className="w-full border-collapse text-left text-[13px]">
            <thead className="sticky top-0 bg-app text-[12px] text-fg-2">
              <tr className="border-b border-line">
                {["Name", "Result", "Size", "Duration", "Finished", "Address"].map((h) => (
                  <th key={h} className="h-8 px-3 font-normal">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((h) => (
                <tr key={h.id} className="h-9 border-b border-line hover:bg-hover">
                  <td className="max-w-[280px] truncate px-3" title={h.finalPath ?? h.fileName}>
                    {h.fileName}
                  </td>
                  <td className={`px-3 capitalize ${h.outcome === "completed" ? "text-ok" : "text-bad"}`}>{h.outcome}</td>
                  <td className="tabular px-3 text-fg-2">{bytes(h.bytes)}</td>
                  <td className="tabular px-3 text-fg-2">{h.durationMs != null ? `${(h.durationMs / 1000).toFixed(1)}s` : "—"}</td>
                  <td className="px-3 whitespace-nowrap text-fg-2">{date(h.completedAt)}</td>
                  <td className="max-w-[320px] truncate px-3 text-fg-3 select-text" title={h.url}>
                    {h.url}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
