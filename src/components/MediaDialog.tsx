import { FolderOpen, Search } from "lucide-react";
import { useEffect, useState } from "react";
import { api, errorMessage } from "../services/api";
import { useDownloads } from "../stores/downloads";
import type { MediaInfo } from "../types";
import { bytes } from "../utils/format";
import { Button, Dialog, Field, inputCls } from "./ui";

const fmtDuration = (s: number | null) =>
  s == null ? "" : `${Math.floor(s / 3600) ? `${Math.floor(s / 3600)}:` : ""}${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

/** Video pages and HLS/DASH streams: extract → pick quality → download via the media pipeline. */
export function MediaDialog({ open, onClose, initialUrl }: { open: boolean; onClose: () => void; initialUrl?: string | null }) {
  const settings = useDownloads((s) => s.settings);
  const queues = useDownloads((s) => s.queues);
  const [url, setUrl] = useState("");
  const [dir, setDir] = useState("");
  const [info, setInfo] = useState<MediaInfo | null>(null);
  const [pick, setPick] = useState(0);
  const [queueId, setQueueId] = useState<number | "">("");
  const [busy, setBusy] = useState<"analyze" | "add" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const analyze = async (u: string) => {
    setBusy("analyze");
    setError(null);
    setInfo(null);
    try {
      const i = await api.probeMedia(u.trim());
      setInfo(i);
      // Default: the best quality not above 1080p (big 4K files only when asked for).
      const idx = i.options.findIndex((o) => o.choice.type === "video" && (o.choice.maxHeight ?? 0) <= 1080);
      setPick(idx >= 0 ? idx : 0);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  useEffect(() => {
    if (!open) return;
    setUrl(initialUrl ?? "");
    setDir(settings?.downloadDir ?? "");
    setInfo(null);
    setError(null);
    setQueueId("");
    if (initialUrl) void analyze(initialUrl);
  }, [open, initialUrl]); // reset only when the dialog opens, not when settings change underneath

  const opt = info?.options[pick];
  const add = async () => {
    if (!info || !opt) return;
    setBusy("add");
    setError(null);
    try {
      await api.addDownload({
        url: url.trim(),
        saveDir: dir.trim(),
        start: queueId === "",
        queueId: queueId === "" ? undefined : queueId,
        media: { choice: opt.choice, kind: opt.kind, title: info.title, expectedBytes: opt.estimatedSize ?? undefined },
      });
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Dialog
      open={open}
      title="Download video"
      width={600}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="accent" onClick={add} disabled={!opt || !dir.trim() || busy !== null}>
            {queueId === "" ? "Download" : "Add to queue"}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <Field label="Video page or stream address" hint="YouTube and most video sites, or a direct .m3u8 / .mpd stream.">
          <div className="flex gap-2">
            <input
              autoFocus
              className={inputCls}
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && url.trim() && void analyze(url)}
              placeholder="https://"
              spellCheck={false}
            />
            <Button className="shrink-0 whitespace-nowrap" onClick={() => analyze(url)} disabled={!url.trim() || busy !== null}>
              <Search size={15} /> {busy === "analyze" ? "Reading…" : "Get formats"}
            </Button>
          </div>
        </Field>

        {info && (
          <>
            <div>
              <p className="text-[14px] font-medium break-words">{info.title}</p>
              <p className="text-[12px] text-fg-3">
                {info.extractor} {info.duration != null && `· ${fmtDuration(info.duration)}`}
              </p>
            </div>
            <fieldset className="flex flex-col gap-1" aria-label="Quality">
              <legend className="mb-1.5 text-[12px] text-fg-2">Quality</legend>
              {info.options.map((o, i) => (
                <label
                  key={o.label}
                  className={`flex cursor-pointer items-center gap-3 rounded-[var(--radius-ctl)] border px-3 py-2 text-[13px] ${
                    i === pick ? "border-accent bg-accent/10" : "border-line bg-surface hover:bg-hover"
                  }`}
                >
                  <input type="radio" name="quality" checked={i === pick} onChange={() => setPick(i)} />
                  <span className="w-36 font-medium">{o.label}</span>
                  <span className="flex-1 text-[12px] text-fg-2">{o.detail}</span>
                  <span className="tabular text-[12px] text-fg-3">{o.estimatedSize ? `~${bytes(o.estimatedSize)}` : ""}</span>
                </label>
              ))}
            </fieldset>
            <div className="grid grid-cols-[1fr_200px] gap-3">
              <Field label="Save to">
                <div className="flex gap-2">
                  <input className={inputCls} value={dir} onChange={(e) => setDir(e.target.value)} spellCheck={false} />
                  <Button aria-label="Browse for folder" onClick={async () => (await api.pickFolder(dir || undefined).then((p) => p && setDir(p)))}>
                    <FolderOpen size={15} />
                  </Button>
                </div>
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
          </>
        )}
        {error && (
          <p role="alert" className="text-[12px] text-bad">
            {error}
          </p>
        )}
        <p className="text-[11px] text-fg-3">
          Uses yt-dlp and FFmpeg. DRM-protected video can't be downloaded. Only download content you have the right to save.
        </p>
      </div>
    </Dialog>
  );
}
