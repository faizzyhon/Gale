import { FolderOpen } from "lucide-react";
import { useEffect, useState } from "react";
import { api, errorMessage } from "../services/api";
import { useDownloads } from "../stores/downloads";
import { toastInfo } from "../stores/toast";
import type { BatchResult, Download, SegmentState } from "../types";
import { bytes, date } from "../utils/format";
import { Button, Dialog, Field, inputCls } from "./ui";

const CONNECTIONS = [1, 2, 4, 8, 16, 32] as const;

function FolderInput({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <div className="flex gap-2">
      <input className={inputCls} value={value} onChange={(e) => onChange(e.target.value)} spellCheck={false} />
      <Button
        aria-label="Browse for folder"
        onClick={async () => {
          const p = await api.pickFolder(value || undefined);
          if (p) onChange(p);
        }}
      >
        <FolderOpen size={15} />
      </Button>
    </div>
  );
}

/** Page URLs on video sites need the media pipeline, not a plain file download. */
const VIDEO_PAGE = /^https?:\/\/([^/]+\.)?(youtube\.com|youtu\.be|vimeo\.com|dailymotion\.com|twitch\.tv|facebook\.com|instagram\.com|tiktok\.com|x\.com|twitter\.com)\//i;
const STREAM_URL = /\.(m3u8|mpd)(\?|$)/i;

export function AddUrlDialog({ open, onClose, initial, onVideo }: { open: boolean; onClose: () => void; initial?: { url: string; fileName?: string | null; referrer?: string | null } | null; onVideo?: (url: string) => void }) {
  const settings = useDownloads((s) => s.settings);
  const queues = useDownloads((s) => s.queues);
  const [queueId, setQueueId] = useState<number | "">("");
  const [url, setUrl] = useState("");
  const [dir, setDir] = useState("");
  const [name, setName] = useState("");
  const [conn, setConn] = useState<number>(8);
  const [sha, setSha] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setUrl(initial?.url ?? "");
      setName(initial?.fileName?.split(/[\\/]/).pop() ?? "");
      setSha("");
      setError(null);
      setQueueId("");
      setDir(settings?.downloadDir ?? "");
      setConn(settings?.connections ?? 8);
    }
  }, [open, settings, initial]);

  const shaValid = sha === "" || /^[0-9a-fA-F]{64}$/.test(sha.trim());
  const submit = async (start: boolean) => {
    setBusy(true);
    setError(null);
    try {
      await api.addDownload({
        url: url.trim(),
        saveDir: dir.trim(),
        fileName: name.trim() || undefined,
        connections: conn,
        expectedSha256: sha.trim() || undefined,
        referrer: initial?.referrer ?? undefined,
        queueId: queueId === "" ? undefined : queueId,
        start: start && queueId === "",
      });
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      title="Add download"
      onClose={onClose}
      footer={
        <>
          <Button onClick={() => submit(false)} disabled={busy || !url || !dir.trim() || !shaValid}>
            Add paused
          </Button>
          <Button variant="accent" onClick={() => submit(true)} disabled={busy || !url || !dir.trim() || !shaValid}>
            {queueId === "" ? "Start download" : "Add to queue"}
          </Button>
        </>
      }
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          if (url && dir.trim() && shaValid) submit(true);
        }}
      >
        <Field label="Address">
          <input autoFocus className={inputCls} value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://" spellCheck={false} />
        </Field>
        <Field label="Save to">
          <FolderInput value={dir} onChange={setDir} />
        </Field>
        <div className="grid grid-cols-[1fr_120px] gap-3">
          <Field label="File name" hint="Leave empty to use the server's name">
            <input className={inputCls} value={name} onChange={(e) => setName(e.target.value)} spellCheck={false} />
          </Field>
          <Field label="Connections">
            <select className={inputCls} value={conn} onChange={(e) => setConn(Number(e.target.value))}>
              {CONNECTIONS.map((c) => (
                <option key={c} value={c}>
                  {c}
                </option>
              ))}
            </select>
          </Field>
        </div>
        <Field label="Queue">
          <select className={inputCls} value={queueId} onChange={(e) => setQueueId(e.target.value === "" ? "" : Number(e.target.value))}>
            <option value="">None — start now</option>
            {queues.map((q) => (
              <option key={q.id} value={q.id}>
                {q.name} ({q.state})
              </option>
            ))}
          </select>
        </Field>
        <Field label="SHA-256 (optional)" hint={shaValid ? "The file is only marked complete if it matches" : "Must be 64 hexadecimal characters"}>
          <input className={`${inputCls} font-mono text-[12px]`} value={sha} onChange={(e) => setSha(e.target.value)} spellCheck={false} />
        </Field>
        {onVideo && (VIDEO_PAGE.test(url.trim()) || STREAM_URL.test(url.trim())) && (
          <p className="rounded-[var(--radius-ctl)] border border-line bg-surface p-3 text-[12px]">
            This looks like a video page or stream.{" "}
            <button type="button" className="font-medium text-accent underline" onClick={() => onVideo(url.trim())}>
              Choose a quality and download it as video
            </button>
          </p>
        )}
        {error && (
          <p role="alert" className="text-[12px] text-bad">
            {error}
          </p>
        )}
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}

export function BatchDialog({ open, onClose, initialUrls }: { open: boolean; onClose: () => void; initialUrls?: string[] | null }) {
  const settings = useDownloads((s) => s.settings);
  const queues = useDownloads((s) => s.queues);
  const [queueId, setQueueId] = useState<number | "">("");
  const [text, setText] = useState("");
  const [dir, setDir] = useState("");
  const [results, setResults] = useState<BatchResult[] | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setText(initialUrls?.join("\n") ?? "");
      setResults(null);
      setQueueId("");
      setDir(settings?.downloadDir ?? "");
    }
  }, [open, settings, initialUrls]);

  const urls = text.split(/\s+/).filter(Boolean);
  const failed = results?.filter((r) => r.error) ?? [];
  const submit = async (start: boolean) => {
    setBusy(true);
    try {
      const r = await api.addBatch(urls, dir, start, queueId === "" ? undefined : queueId, initialUrls ? "browser" : undefined);
      const ok = r.filter((x) => x.id != null).length;
      toastInfo(`Added ${ok} of ${r.length} downloads`);
      if (r.every((x) => x.id != null)) onClose();
      else setResults(r);
    } catch (e) {
      setResults([{ url: "", id: null, error: errorMessage(e) }]);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      title={initialUrls ? `Add ${initialUrls.length} links` : "Add batch"}
      width={620}
      onClose={onClose}
      footer={
        <>
          <Button onClick={() => submit(false)} disabled={busy || urls.length === 0 || !dir.trim()}>
            Add paused
          </Button>
          <Button variant="accent" onClick={() => submit(true)} disabled={busy || urls.length === 0 || !dir.trim()}>
            {queueId === "" ? `Start ${urls.length || ""} downloads` : `Add ${urls.length} to queue`}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <Field label="Addresses" hint="One per line. Only http and https links are accepted.">
          <textarea
            autoFocus
            className={`${inputCls} h-48 resize-none py-2 font-mono text-[12px]`}
            value={text}
            onChange={(e) => setText(e.target.value)}
            spellCheck={false}
          />
        </Field>
        <div className="grid grid-cols-[1fr_200px] gap-3">
          <Field label="Save to">
            <FolderInput value={dir} onChange={setDir} />
          </Field>
          <Field label="Queue">
            <select className={inputCls} value={queueId} onChange={(e) => setQueueId(e.target.value === "" ? "" : Number(e.target.value))}>
              <option value="">None — start now</option>
              {queues.map((q) => (
                <option key={q.id} value={q.id}>
                  {q.name}
                </option>
              ))}
            </select>
          </Field>
        </div>
        {failed.length > 0 && (
          <div role="alert" className="rounded-[var(--radius-ctl)] border border-line bg-surface p-3 text-[12px]">
            <p className="mb-2 font-medium text-bad">{failed.length} not added</p>
            <ul className="space-y-1">
              {failed.map((f, i) => (
                <li key={i} className="truncate text-fg-2" title={f.url}>
                  {f.url ? <span className="font-mono">{f.url}</span> : null} — {f.error}
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </Dialog>
  );
}

export function RemoveDialog({ download, onClose, onConfirm }: { download: Download | null; onClose: () => void; onConfirm: (deleteFile: boolean) => void }) {
  const [deleteFile, setDeleteFile] = useState(false);
  useEffect(() => setDeleteFile(false), [download]);
  return (
    <Dialog
      open={!!download}
      title="Remove download"
      width={440}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Keep</Button>
          <Button variant={deleteFile ? "danger" : "accent"} onClick={() => onConfirm(deleteFile)}>
            {deleteFile ? "Remove and delete file" : "Remove from list"}
          </Button>
        </>
      }
    >
      <p className="mb-4 text-[13px] break-all text-fg-2">{download?.fileName}</p>
      {download?.status === "completed" ? (
        <label className="flex items-center gap-2 text-[13px]">
          <input type="checkbox" checked={deleteFile} onChange={(e) => setDeleteFile(e.target.checked)} />
          Also delete the downloaded file from disk
        </label>
      ) : (
        <p className="text-[12px] text-fg-3">Any partially downloaded data will be deleted.</p>
      )}
    </Dialog>
  );
}

export function PropertiesDialog({ download, onClose }: { download: Download | null; onClose: () => void }) {
  const fresh = useDownloads((s) => (download ? s.downloads[download.id] : undefined)) ?? download;
  const live = useDownloads((s) => (download ? s.live[download.id] : undefined));
  const [stored, setStored] = useState<SegmentState[]>([]);

  useEffect(() => {
    if (download) api.segments(download.id).then(setStored, () => setStored([]));
  }, [download]);

  if (!fresh) return <Dialog open={false} title="" onClose={onClose}>{null}</Dialog>;
  const segments = live?.segments ?? stored;
  const rows: [string, string | null][] = [
    ["Address", fresh.url],
    ["Final address", fresh.finalUrl !== fresh.url ? fresh.finalUrl : null],
    ["Referrer", fresh.referrer],
    ["Saved to", fresh.finalPath ?? fresh.saveDir],
    ["Type", fresh.mimeType],
    ["Size", fresh.totalBytes != null ? `${bytes(fresh.totalBytes)} (${fresh.totalBytes.toLocaleString()} bytes)` : "Unknown"],
    ["Downloaded", bytes(fresh.downloadedBytes)],
    ["Resumable", fresh.resumable ? "Yes — server supports byte ranges" : "No — single connection"],
    ["Connections", String(fresh.connections)],
    ["ETag", fresh.etag],
    ["Last modified", fresh.lastModified],
    ["SHA-256", fresh.sha256],
    ["Expected SHA-256", fresh.expectedSha256],
    ["Added", date(fresh.createdAt)],
    ["Started", date(fresh.startedAt)],
    ["Completed", date(fresh.completedAt)],
    ["Error", fresh.errorMessage],
  ];

  return (
    <Dialog open={!!download} title="Properties" width={640} onClose={onClose} footer={<Button onClick={onClose}>Close</Button>}>
      <p className="mb-4 text-[15px] font-medium break-all">{fresh.fileName}</p>
      {fresh.totalBytes ? <SegmentMap total={fresh.totalBytes} segments={segments} /> : null}
      <dl className="grid grid-cols-[140px_1fr] gap-x-4 gap-y-2 text-[13px]">
        {rows
          .filter(([, v]) => v)
          .map(([k, v]) => (
            <div key={k} className="contents">
              <dt className="text-fg-2">{k}</dt>
              <dd className="font-mono text-[12px] break-all select-text">{v}</dd>
            </div>
          ))}
      </dl>
    </Dialog>
  );
}

/** Byte map of the file: each bar is a segment, filled up to what is on disk. */
function SegmentMap({ total, segments }: { total: number; segments: SegmentState[] }) {
  if (segments.length === 0) return null;
  return (
    <div className="mb-5">
      <div className="relative h-3 w-full overflow-hidden rounded-[2px] bg-line" aria-label="Segment progress">
        {segments.map((s) => (
          <div
            key={s.start}
            className="absolute top-0 h-full bg-accent"
            style={{ left: `${(s.start / total) * 100}%`, width: `${(s.written / total) * 100}%` }}
          />
        ))}
      </div>
      <p className="mt-1.5 text-[11px] text-fg-3">{segments.length} segments</p>
    </div>
  );
}

export function ShutdownDialog({ seconds, onCancel }: { seconds: number | null; onCancel: () => void }) {
  const [left, setLeft] = useState(seconds ?? 0);
  useEffect(() => {
    if (seconds == null) return;
    setLeft(seconds);
    const t = setInterval(() => setLeft((l) => Math.max(0, l - 1)), 1000);
    return () => clearInterval(t);
  }, [seconds]);
  return (
    <Dialog
      open={seconds != null}
      title="Shutting down Windows"
      width={420}
      onClose={onCancel}
      footer={
        <Button variant="accent" autoFocus onClick={onCancel}>
          Cancel shutdown
        </Button>
      }
    >
      <p className="text-[13px]">
        Your scheduled downloads have finished. Windows will shut down in <span className="tabular font-semibold">{left}</span> seconds.
      </p>
    </Dialog>
  );
}
