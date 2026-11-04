import { Globe, Search, Square } from "lucide-react";
import { useMemo, useState } from "react";
import { Button, Field, inputCls } from "../components/ui";
import { api, errorMessage } from "../services/api";
import { useDownloads } from "../stores/downloads";
import { toastInfo } from "../stores/toast";
import type { GrabResult } from "../types";

export function GrabberPage() {
  const { categories, settings } = useDownloads();
  const [url, setUrl] = useState("");
  const [cats, setCats] = useState<string[]>(["Images", "Documents"]);
  const [extra, setExtra] = useState("");
  const [depth, setDepth] = useState(1);
  const [sameDomain, setSameDomain] = useState(true);
  const [include, setInclude] = useState("");
  const [exclude, setExclude] = useState("");
  const [maxPages, setMaxPages] = useState(50);
  const [running, setRunning] = useState<string | null>(null);
  const [result, setResult] = useState<GrabResult | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);

  const extensions = useMemo(() => {
    const fromCats = categories.filter((c) => cats.includes(c.name)).flatMap((c) => c.extensions.split(/\s+/));
    return [...new Set([...fromCats, ...extra.split(/[\s,]+/).map((e) => e.replace(/^\./, "").toLowerCase())].filter(Boolean))];
  }, [categories, cats, extra]);

  const start = async () => {
    const id = crypto.randomUUID();
    setRunning(id);
    setError(null);
    setResult(null);
    try {
      const r = await api.startGrab(id, { startUrl: url.trim(), maxDepth: depth, sameDomain, extensions, include, exclude, maxPages, delayMs: 500 });
      setResult(r);
      setPicked(new Set(r.resources.map((x) => x.url)));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setRunning(null);
    }
  };

  const download = async () => {
    if (!result || !settings) return;
    let host = "site";
    try {
      host = new URL(url).hostname;
    } catch {
      /* keep default */
    }
    const dir = `${settings.downloadDir}${settings.downloadDir.includes("\\") ? "\\" : "/"}${host}`;
    const r = await api.addBatch([...picked], dir, true, undefined, "grabber");
    const ok = r.filter((x) => x.id != null).length;
    toastInfo(`Added ${ok} of ${r.length} files into ${host}`);
  };

  return (
    <div className="flex h-full flex-col">
      <div className="shrink-0 px-8 pt-6 pb-4">
        <h1 className="mb-4 font-display text-[28px] font-semibold">Site Grabber</h1>
        <div className="grid max-w-[900px] grid-cols-[1fr_auto] gap-3">
          <input className={inputCls} placeholder="https://example.com/gallery" value={url} onChange={(e) => setUrl(e.target.value)} spellCheck={false} aria-label="Website address" />
          {running ? (
            <Button onClick={() => api.cancelGrab(running)}>
              <Square size={15} /> Stop
            </Button>
          ) : (
            <Button variant="accent" onClick={start} disabled={!url.trim() || extensions.length === 0}>
              <Search size={15} /> Find files
            </Button>
          )}
        </div>
        <div className="mt-4 grid max-w-[900px] grid-cols-2 gap-x-6 gap-y-3 xl:grid-cols-4">
          <fieldset className="col-span-2 flex flex-wrap gap-x-4 gap-y-1">
            <legend className="mb-1.5 text-[12px] text-fg-2">File types</legend>
            {categories
              .filter((c) => c.extensions)
              .map((c) => (
                <label key={c.id} className="flex items-center gap-1.5 text-[13px]">
                  <input type="checkbox" checked={cats.includes(c.name)} onChange={(e) => setCats(e.target.checked ? [...cats, c.name] : cats.filter((x) => x !== c.name))} />
                  {c.name}
                </label>
              ))}
          </fieldset>
          <Field label="More extensions">
            <input className={inputCls} value={extra} onChange={(e) => setExtra(e.target.value)} placeholder="svg, csv" />
          </Field>
          <Field label="Link depth">
            <select className={inputCls} value={depth} onChange={(e) => setDepth(Number(e.target.value))}>
              {[0, 1, 2, 3, 4, 5].map((d) => (
                <option key={d} value={d}>
                  {d === 0 ? "This page only" : `${d} level${d > 1 ? "s" : ""}`}
                </option>
              ))}
            </select>
          </Field>
          <Field label="URL must contain">
            <input className={inputCls} value={include} onChange={(e) => setInclude(e.target.value)} />
          </Field>
          <Field label="URL must not contain">
            <input className={inputCls} value={exclude} onChange={(e) => setExclude(e.target.value)} />
          </Field>
          <Field label="Page limit">
            <input type="number" min={1} max={500} className={inputCls} value={maxPages} onChange={(e) => setMaxPages(Number(e.target.value) || 1)} />
          </Field>
          <label className="flex items-end gap-2 pb-2 text-[13px]">
            <input type="checkbox" checked={sameDomain} onChange={(e) => setSameDomain(e.target.checked)} /> Stay on this site
          </label>
        </div>
        <p className="mt-3 text-[11px] text-fg-3">Gale follows robots.txt, waits between pages and never signs in or bypasses access controls.</p>
        {error && <p role="alert" className="mt-2 text-[12px] text-bad">{error}</p>}
      </div>

      <div className="min-h-0 flex-1 overflow-auto border-t border-line px-5">
        {running && <p className="p-3 text-[13px] text-fg-2">Crawling…</p>}
        {!running && !result && (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-fg-3">
            <Globe size={28} strokeWidth={1.25} />
            <p className="text-[12px]">Found files appear here for you to choose from.</p>
          </div>
        )}
        {result && (
          <>
            <div className="sticky top-0 flex items-center gap-3 bg-app py-3 text-[12px] text-fg-2">
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={picked.size === result.resources.length && picked.size > 0}
                  onChange={(e) => setPicked(e.target.checked ? new Set(result.resources.map((r) => r.url)) : new Set())}
                />
                {result.resources.length} files from {result.pagesVisited} pages
                {result.pagesBlockedByRobots > 0 && ` · ${result.pagesBlockedByRobots} skipped by robots.txt`}
              </label>
              <span className="flex-1" />
              <Button variant="accent" disabled={picked.size === 0} onClick={download}>
                Download {picked.size} files
              </Button>
            </div>
            <ul className="pb-4 text-[13px]">
              {result.resources.map((r) => (
                <li key={r.url} className="flex items-center gap-3 border-b border-line py-1.5">
                  <input
                    type="checkbox"
                    aria-label={r.fileName}
                    checked={picked.has(r.url)}
                    onChange={(e) => {
                      const n = new Set(picked);
                      if (e.target.checked) n.add(r.url);
                      else n.delete(r.url);
                      setPicked(n);
                    }}
                  />
                  <span className="w-12 text-[11px] text-fg-3 uppercase">{r.extension}</span>
                  <span className="w-64 truncate">{r.fileName}</span>
                  <span className="flex-1 truncate text-[12px] text-fg-3 select-text" title={r.url}>
                    {r.url}
                  </span>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
    </div>
  );
}
