import {
  ClipboardCopy,
  ExternalLink,
  FolderOpen,
  Info,
  ListOrdered,
  Pause,
  Play,
  RefreshCw,
  RotateCcw,
  Square,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { canPause, canResume, useDownloadActions } from "../hooks/useDownloadActions";
import { api } from "../services/api";
import { useDownloads } from "../stores/downloads";
import { toastError } from "../stores/toast";
import type { Download } from "../types";
import { bytes, date, eta, extension, speed } from "../utils/format";
import { AnimatePresence, motion } from "framer-motion";
import { ProgressBar } from "./motion";

const STAGE_LABEL: Record<string, string> = {
  extracting: "Reading video info",
  downloading: "Downloading",
  merging: "Merging audio + video",
  converting: "Converting",
  processing: "Processing",
  verifying: "Verifying",
  metadata: "Getting torrent info",
  checking: "Checking files",
};
const KIND_LABEL: Record<Download["kind"], string> = { http: "", hls: "Video · HLS", dash: "Video · DASH", video: "Video", audio: "Audio", torrent: "Torrent" };

const STATUS_LABEL: Record<Download["status"], string> = {
  queued: "Queued",
  probing: "Connecting",
  downloading: "Downloading",
  paused: "Paused",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
  scheduled: "Scheduled",
};

interface Props {
  rows: Download[];
  onProperties: (d: Download) => void;
  onRemove: (d: Download) => void;
}

export function DownloadTable({ rows, onProperties, onRemove }: Props) {
  const live = useDownloads((s) => s.live);
  const act = useDownloadActions();
  const [selected, setSelected] = useState<number | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; d: Download } | null>(null);
  const sel = rows.find((r) => r.id === selected) ?? null;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!sel || (e.target as HTMLElement).tagName === "INPUT") return;
      if (e.key === "Delete") onRemove(sel);
      if (e.key === "Enter") sel.status === "completed" ? act.open(sel) : onProperties(sel);
      if (e.key === " ") {
        e.preventDefault();
        canPause(sel) ? act.pause(sel) : canResume(sel) && act.resume(sel);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [sel, act, onProperties, onRemove]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-11 shrink-0 items-center gap-1 px-3">
        <RowAction icon={Play} label="Resume" disabled={!sel || !canResume(sel)} onClick={() => sel && act.resume(sel)} />
        <RowAction icon={Pause} label="Pause" disabled={!sel || !canPause(sel)} onClick={() => sel && act.pause(sel)} />
        <RowAction icon={Square} label="Cancel" disabled={!sel || sel.status === "completed" || sel.status === "cancelled"} onClick={() => sel && act.cancel(sel)} />
        <RowAction icon={RotateCcw} label="Restart" disabled={!sel} onClick={() => sel && act.restart(sel)} />
        <RowAction icon={Trash2} label="Remove" disabled={!sel} onClick={() => sel && onRemove(sel)} />
        <div className="mx-1 h-5 w-px bg-line" />
        <RowAction icon={ExternalLink} label="Open" disabled={sel?.status !== "completed"} onClick={() => sel && act.open(sel)} />
        <RowAction icon={FolderOpen} label="Show in folder" disabled={!sel} onClick={() => sel && act.reveal(sel)} />
        <RowAction icon={Info} label="Properties" disabled={!sel} onClick={() => sel && onProperties(sel)} />
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {rows.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-1 text-fg-3">
            <p className="text-[14px] text-fg-2">Nothing here yet</p>
            <p className="text-[12px]">Use Add URL to start a download.</p>
          </div>
        ) : (
          <table className="w-full min-w-[1180px] border-collapse text-left">
            <thead className="sticky top-0 z-10 bg-app text-[12px] text-fg-2">
              <tr className="border-b border-line">
                {["Name", "Type", "Progress", "Downloaded", "Size", "Speed", "Time left", "Conn.", "Status", "Added", "Save location"].map((h) => (
                  <th key={h} className="h-8 px-3 font-normal whitespace-nowrap">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              <AnimatePresence initial={false}>
              {rows.map((d) => {
                const l = live[d.id];
                const frac = d.totalBytes ? d.downloadedBytes / d.totalBytes : d.status === "completed" ? 1 : null;
                const isSel = d.id === selected;
                return (
                  <motion.tr
                    layout="position"
                    initial={{ opacity: 0, y: -6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, x: 12, transition: { duration: 0.15 } }}
                    transition={{ duration: 0.2, ease: "easeOut" }}
                    key={d.id}
                    onClick={() => setSelected(d.id)}
                    onDoubleClick={() => (d.status === "completed" ? act.open(d) : onProperties(d))}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setSelected(d.id);
                      setMenu({ x: e.clientX, y: e.clientY, d });
                    }}
                    aria-selected={isSel}
                    className={`h-10 border-b border-line text-[13px] ${isSel ? "bg-accent/10" : "hover:bg-hover"}`}
                  >
                    <td className="max-w-[300px] truncate px-3" title={d.fileName}>
                      {d.fileName}
                    </td>
                    <td className="px-3 whitespace-nowrap text-fg-2">{d.kind === "http" ? extension(d.fileName) : KIND_LABEL[d.kind]}</td>
                    <td className="w-[160px] px-3">
                      <div className="flex items-center gap-2">
                        {/* fixed width: auto table layout squeezes a %-width bar to nothing */}
                        <div className="w-24 shrink-0">
                          <ProgressBar value={frac} status={d.status} />
                        </div>
                        <span className="tabular w-10 text-right text-[12px] text-fg-2">{frac != null ? `${Math.floor(frac * 100)}%` : ""}</span>
                      </div>
                    </td>
                    <td className="tabular px-3 whitespace-nowrap text-fg-2">{bytes(d.downloadedBytes)}</td>
                    <td className="tabular px-3 whitespace-nowrap text-fg-2">{bytes(d.totalBytes)}</td>
                    <td className="tabular px-3 whitespace-nowrap">{d.status === "downloading" ? speed(l?.bytesPerSec) : "—"}</td>
                    <td className="tabular px-3 whitespace-nowrap text-fg-2">
                      {d.status === "downloading" ? eta(d.totalBytes != null ? d.totalBytes - d.downloadedBytes : null, l?.bytesPerSec) : "—"}
                    </td>
                    <td className="tabular px-3 text-fg-2">{d.kind === "torrent" && d.status === "downloading" ? (l?.activeConnections ?? 0) : d.kind !== "http" ? "—" : d.status === "downloading" ? `${l?.activeConnections ?? 0}/${d.connections}` : d.connections}</td>
                    <td className="px-3 whitespace-nowrap">
                      <span className={d.status === "failed" ? "text-bad" : d.status === "completed" ? "text-ok" : ""} title={d.errorMessage ?? undefined}>
                        {/* All bytes on disk but not yet marked complete = size/checksum check + rename in progress */}
                        {d.status === "downloading" && l?.stage
                          ? STAGE_LABEL[l.stage]
                          : d.status === "downloading" && d.kind === "http" && d.totalBytes != null && d.downloadedBytes >= d.totalBytes && !l?.activeConnections
                            ? "Verifying"
                            : STATUS_LABEL[d.status]}
                      </span>
                    </td>
                    <td className="px-3 whitespace-nowrap text-fg-2">{date(d.createdAt)}</td>
                    <td className="max-w-[240px] truncate px-3 text-fg-2" title={d.saveDir}>
                      {d.saveDir}
                    </td>
                  </motion.tr>
                );
              })}
              </AnimatePresence>
            </tbody>
          </table>
        )}
      </div>

      {menu && <ContextMenu {...menu} onClose={() => setMenu(null)} onProperties={onProperties} onRemove={onRemove} />}
    </div>
  );
}

function RowAction({ icon: Icon, label, disabled, onClick }: { icon: LucideIcon; label: string; disabled?: boolean; onClick: () => void }) {
  return (
    <button
      title={label}
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className="flex h-8 items-center gap-1.5 rounded-[var(--radius-ctl)] px-2 text-[12px] text-fg-2 hover:bg-hover hover:text-fg disabled:opacity-35 disabled:hover:bg-transparent"
    >
      <Icon size={15} strokeWidth={1.75} />
      <span className="hidden xl:inline">{label}</span>
    </button>
  );
}

function ContextMenu({
  x,
  y,
  d,
  onClose,
  onProperties,
  onRemove,
}: {
  x: number;
  y: number;
  d: Download;
  onClose: () => void;
  onProperties: (d: Download) => void;
  onRemove: (d: Download) => void;
}) {
  const act = useDownloadActions();
  const { queues, memberships, refreshQueues } = useDownloads();
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (r) setPos({ x: Math.min(x, window.innerWidth - r.width - 8), y: Math.min(y, window.innerHeight - r.height - 8) });
  }, [x, y]);
  useEffect(() => {
    const close = () => onClose();
    window.addEventListener("mousedown", close);
    window.addEventListener("blur", close);
    window.addEventListener("keydown", close);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("keydown", close);
    };
  }, [onClose]);

  const items: [LucideIcon, string, boolean, () => void][] = [
    [ExternalLink, "Open", d.status === "completed", () => act.open(d)],
    [FolderOpen, "Open containing folder", true, () => act.reveal(d)],
    [Play, "Resume", canResume(d), () => act.resume(d)],
    [Pause, "Pause", canPause(d), () => act.pause(d)],
    [RotateCcw, "Restart", true, () => act.restart(d)],
    [Square, "Cancel", d.status !== "completed" && d.status !== "cancelled", () => act.cancel(d)],
    [ClipboardCopy, "Copy download URL", true, () => act.copyUrl(d)],
    [RefreshCw, "Recheck availability", true, () => act.recheck(d)],
    ...queues
      .filter((q) => memberships[d.id] !== q.id && d.status !== "completed")
      .map((q): [LucideIcon, string, boolean, () => void] => [ListOrdered, `Add to “${q.name}”`, true, () => void api.setDownloadQueue(d.id, q.id).then(refreshQueues, toastError)]),
    ...(memberships[d.id] != null
      ? [[ListOrdered, "Remove from queue", true, () => void api.setDownloadQueue(d.id, null).then(refreshQueues, toastError)] as [LucideIcon, string, boolean, () => void]]
      : []),
    [Trash2, "Remove…", true, () => onRemove(d)],
    [Info, "Properties", true, () => onProperties(d)],
  ];
  return (
    <div
      ref={ref}
      role="menu"
      style={{ left: pos.x, top: pos.y }}
      onMouseDown={(e) => e.stopPropagation()}
      className="fixed z-50 w-60 rounded-[var(--radius-surface)] border border-line bg-surface p-1 shadow-xl"
    >
      {items.map(([Icon, label, enabled, fn]) => (
        <button
          key={label}
          role="menuitem"
          disabled={!enabled}
          onClick={() => {
            fn();
            onClose();
          }}
          className="flex h-8 w-full items-center gap-3 rounded-[var(--radius-ctl)] px-2.5 text-left text-[13px] hover:bg-hover disabled:opacity-35 disabled:hover:bg-transparent"
        >
          <Icon size={15} strokeWidth={1.75} className="text-fg-2" />
          {label}
        </button>
      ))}
    </div>
  );
}
