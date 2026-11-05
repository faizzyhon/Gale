import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { AnimatePresence, motion } from "framer-motion";
import { ChevronDown, ExternalLink, FileArchive, FileAudio, FileText, FileVideo, File as FileIcon, FolderOpen, Pause, Play, RotateCcw, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { AnimatedNumber, DrawnCheck, ProgressBar, SegmentBars, SpeedGraph } from "../components/motion";
import { Button } from "../components/ui";
import { api, errorMessage } from "../services/api";
import { applyTheme } from "../stores/downloads";
import type { AppSettings, Download, MediaStage, SegmentState } from "../types";
import { bytes, eta, speed } from "../utils/format";
import { TitleBar } from "./TitleBar";

const STAGES: { key: MediaStage | "done"; label: string }[] = [
  { key: "extracting", label: "Read video info" },
  { key: "downloading", label: "Download" },
  { key: "merging", label: "Merge / convert" },
  { key: "verifying", label: "Verify" },
];
const STAGE_TEXT: Record<string, string> = {
  extracting: "Reading video info…",
  downloading: "Receiving data…",
  merging: "Merging audio and video…",
  converting: "Converting…",
  processing: "Processing…",
  verifying: "Verifying file…",
};
const COMPACT = 330;
const EXPANDED = 520;

function iconFor(d: Download) {
  if (d.kind === "audio" || /\.(mp3|m4a|flac|wav|ogg|opus)$/i.test(d.fileName)) return FileAudio;
  if (d.kind !== "http" || /\.(mp4|mkv|webm|avi|mov)$/i.test(d.fileName)) return FileVideo;
  if (/\.(zip|rar|7z|tar|gz|iso)$/i.test(d.fileName)) return FileArchive;
  if (/\.(pdf|docx?|xlsx?|pptx?|txt|epub)$/i.test(d.fileName)) return FileText;
  return FileIcon;
}

export function DownloadWindow({ id }: { id: number }) {
  const [d, setD] = useState<Download | null>(null);
  const [live, setLive] = useState<{ bps: number; conns: number; segments: SegmentState[]; stage?: MediaStage }>({ bps: 0, conns: 0, segments: [] });
  const [history, setHistory] = useState<number[]>([]);
  const [expanded, setExpanded] = useState(false);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const shown = useRef(false);
  const bps = useRef(0);
  const win = getCurrentWindow();

  useEffect(() => {
    api.getSettings().then((s) => (setSettings(s), applyTheme(s.theme)), () => {});
    api.getDownload(id).then(setD, (e) => setError(errorMessage(e)));
    api.segments(id).then((segments) => setLive((l) => ({ ...l, segments })), () => {});
    const subs = [
      api.onProgress((e) => {
        if (e.id !== id) return;
        bps.current = e.bytesPerSec;
        setLive({ bps: e.bytesPerSec, conns: e.activeConnections, segments: e.segments, stage: e.stage });
        setD((x) => (x ? { ...x, downloadedBytes: e.written, totalBytes: e.total ?? x.totalBytes } : x));
      }),
      api.onChanged((x) => {
        if (x.id !== id) return;
        setD(x);
        if (x.status !== "downloading") bps.current = 0;
      }),
    ];
    // One speed sample per second, independent of how often ticks arrive.
    const t = setInterval(() => setHistory((h) => [...h.slice(-59), bps.current]), 1000);
    return () => {
      clearInterval(t);
      subs.forEach((p) => void p.then((f) => f()));
    };
  }, [id]);

  // Show the window only once there is something to show (no white flash).
  useEffect(() => {
    if ((d || error) && !shown.current) {
      shown.current = true;
      void win.show().then(() => win.setFocus());
    }
  }, [d, error, win]);

  const done = d?.status === "completed";
  useEffect(() => {
    if (done && settings?.closeWindowWhenDone) {
      const t = setTimeout(() => void win.close(), 2200);
      return () => clearTimeout(t);
    }
  }, [done, settings, win]);
  // The completion card is compact: fold the details panel away when the download finishes.
  useEffect(() => {
    if (done && expanded) {
      setExpanded(false);
      void win.setSize(new LogicalSize(520, COMPACT));
    }
  }, [done, expanded, win]);

  const toggle = async () => {
    const next = !expanded;
    setExpanded(next);
    await win.setSize(new LogicalSize(520, next ? EXPANDED : COMPACT));
  };
  const act = (p: Promise<unknown>) => p.catch((e) => setError(errorMessage(e)));

  if (!d) return <div className="flex h-full flex-col bg-app"><TitleBar title="Gale" />{error && <p className="p-5 text-bad">{error}</p>}</div>;

  const frac = d.totalBytes ? d.downloadedBytes / d.totalBytes : done ? 1 : null;
  const pct = frac != null ? Math.floor(frac * 100) : null;
  const running = d.status === "downloading" || d.status === "probing" || d.status === "queued";
  const Icon = iconFor(d);
  const stage = live.stage;
  const statusText =
    d.status === "downloading" && stage ? STAGE_TEXT[stage] : d.status === "downloading" ? "Receiving data…" : d.status === "probing" ? "Connecting…" : d.status === "queued" ? "Waiting for a free slot…" : d.status === "paused" ? "Paused" : d.status === "failed" ? "Failed" : d.status === "cancelled" ? "Cancelled" : "Complete";
  const title = done ? `Download complete · ${d.fileName}` : `${pct != null ? `${pct}% · ` : ""}${d.fileName}`;

  return (
    <div className="flex h-full flex-col overflow-hidden border border-line bg-app">
      <TitleBar title={title} />
      <AnimatePresence mode="wait" initial={false}>
        {done ? (
          <motion.div key="done" className="flex flex-1 flex-col px-6 pb-5" initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} transition={{ duration: 0.25 }}>
            <div className="flex flex-1 items-center gap-5">
              <DrawnCheck />
              <div className="min-w-0">
                <p className="font-display text-[18px] font-semibold">Download complete</p>
                <p className="truncate text-[13px]" title={d.fileName}>{d.fileName}</p>
                <p className="text-[12px] text-fg-3">{bytes(d.totalBytes ?? d.downloadedBytes)} · saved to <span className="select-text">{d.finalPath ? d.finalPath.replace(/[\\/][^\\/]*$/, "") : d.saveDir}</span></p>
              </div>
            </div>
            {error && <p role="alert" className="mb-2 text-[12px] text-bad">{error}</p>}
            <div className="flex justify-end gap-2">
              <Button onClick={() => act(api.reveal(id))}><FolderOpen size={15} /> Open folder</Button>
              <Button onClick={() => win.close()}>Close</Button>
              <Button variant="accent" autoFocus onClick={() => act(api.open(id).then(() => win.close()))}><ExternalLink size={15} /> Open</Button>
            </div>
          </motion.div>
        ) : (
          <motion.div key="progress" className="flex flex-1 flex-col px-5 pb-4" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0, y: -8 }} transition={{ duration: 0.2 }}>
            <div className="flex items-center gap-3 pt-1">
              <motion.div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-[var(--radius-surface)] bg-accent/12 text-accent" animate={running ? { y: [0, -2, 0] } : { y: 0 }} transition={running ? { repeat: Infinity, duration: 1.8, ease: "easeInOut" } : {}}>
                <Icon size={22} strokeWidth={1.75} />
              </motion.div>
              <div className="min-w-0 flex-1">
                <p className="truncate text-[14px] font-medium" title={d.fileName}>{d.fileName}</p>
                <p className="truncate text-[11px] text-fg-3" title={d.url}>{d.url}</p>
              </div>
            </div>

            <div className="mt-4 flex items-end justify-between">
              <AnimatePresence mode="wait">
                <motion.p key={statusText} className={`text-[13px] ${d.status === "failed" ? "text-bad" : "text-fg-2"}`} initial={{ opacity: 0, y: 4 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -4 }} transition={{ duration: 0.15 }}>
                  {statusText}
                </motion.p>
              </AnimatePresence>
              <p className="font-display text-[26px] leading-none font-semibold">{pct != null ? <><AnimatedNumber value={pct} format={(v) => String(Math.round(v))} /><span className="text-[15px] text-fg-3">%</span></> : ""}</p>
            </div>
            <div className="mt-2"><ProgressBar value={frac} status={d.status} height={8} /></div>

            <dl className="mt-3 grid grid-cols-3 gap-x-4 gap-y-1.5 text-[12px]">
              <Stat k="Downloaded" v={<>{bytes(d.downloadedBytes)} <span className="text-fg-3">/ {bytes(d.totalBytes)}</span></>} />
              <Stat k="Speed" v={d.status === "downloading" ? <AnimatedNumber value={live.bps} format={(v) => speed(v)} /> : "—"} />
              <Stat k="Time left" v={d.status === "downloading" ? eta(d.totalBytes != null ? d.totalBytes - d.downloadedBytes : null, live.bps) : "—"} />
              <Stat k={d.kind === "http" ? "Connections" : "Type"} v={d.kind === "http" ? `${live.conns} of ${d.connections}` : d.kind === "audio" ? "Audio" : `Video · ${d.kind.toUpperCase()}`} />
              <Stat k="Resume" v={d.kind !== "http" ? "Yes" : d.resumable ? "Yes" : d.status === "probing" ? "Checking…" : "No"} />
              <Stat k="Save to" v={<span title={d.saveDir}>{d.saveDir.split(/[\\/]/).pop()}</span>} />
            </dl>

            <AnimatePresence initial={false}>
              {expanded && (
                <motion.div className="mt-3 overflow-hidden" initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} transition={{ duration: 0.22 }}>
                  <div className="rounded-[var(--radius-surface)] border border-line bg-surface p-3">
                    <SpeedGraph samples={history} height={56} />
                    <div className="mt-3">
                      {d.kind === "http" ? (
                        d.totalBytes && live.segments.length > 0 ? <SegmentBars total={d.totalBytes} segments={live.segments} /> : <p className="text-[11px] text-fg-3">Single connection (the server doesn't support splitting this file).</p>
                      ) : (
                        <StageSteps stage={stage} status={d.status} />
                      )}
                    </div>
                  </div>
                </motion.div>
              )}
            </AnimatePresence>

            {(error || (d.status === "failed" && d.errorMessage)) && <p role="alert" className="mt-2 truncate text-[12px] text-bad" title={error ?? d.errorMessage ?? ""}>{error ?? d.errorMessage}</p>}
            <div className="mt-auto flex items-center gap-2 pt-3">
              <Button variant="subtle" onClick={toggle} aria-expanded={expanded}>
                <motion.span animate={{ rotate: expanded ? 180 : 0 }} className="inline-flex"><ChevronDown size={15} /></motion.span>
                {expanded ? "Hide details" : "Show details"}
              </Button>
              <span className="flex-1" />
              {running ? (
                <Button onClick={() => act(api.pause(id))}><Pause size={15} /> Pause</Button>
              ) : d.status === "failed" || d.status === "cancelled" ? (
                <Button variant="accent" onClick={() => act(api.restart(id))}><RotateCcw size={15} /> Try again</Button>
              ) : (
                <Button variant="accent" onClick={() => act(api.resume(id))}><Play size={15} /> Resume</Button>
              )}
              <Button onClick={() => act((running || d.status === "paused" ? api.cancel(id) : Promise.resolve()).then(() => win.close()))}>
                <X size={15} /> {running || d.status === "paused" ? "Cancel" : "Close"}
              </Button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function Stat({ k, v }: { k: string; v: React.ReactNode }) {
  return (
    <div className="min-w-0">
      <dt className="text-[11px] text-fg-3">{k}</dt>
      <dd className="truncate">{v}</dd>
    </div>
  );
}

function StageSteps({ stage, status }: { stage?: MediaStage; status: string }) {
  const order = ["extracting", "downloading", "merging", "verifying"];
  const cur = stage === "converting" || stage === "processing" ? 2 : Math.max(0, order.indexOf(stage ?? "extracting"));
  return (
    <ol className="flex items-center gap-2 text-[11px]">
      {STAGES.map((s, i) => {
        const state = status === "completed" || i < cur ? "done" : i === cur && status === "downloading" ? "now" : "todo";
        return (
          <li key={s.key} className="flex flex-1 items-center gap-2">
            <motion.span
              className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[10px] ${state === "done" ? "bg-ok text-white" : state === "now" ? "bg-accent text-accent-fg" : "bg-line text-fg-3"}`}
              animate={state === "now" ? { scale: [1, 1.12, 1] } : { scale: 1 }}
              transition={state === "now" ? { repeat: Infinity, duration: 1.4 } : {}}
            >
              {state === "done" ? "✓" : i + 1}
            </motion.span>
            <span className={state === "todo" ? "text-fg-3" : ""}>{s.label}</span>
          </li>
        );
      })}
    </ol>
  );
}
