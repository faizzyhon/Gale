import {
  AppWindow,
  Archive,
  Bell,
  Globe,
  ListOrdered,
  Play,
  Square,
  CalendarClock,
  CheckCircle2,
  Clock,
  Download as DownloadIcon,
  FileText,
  Film,
  FolderOpen,
  Gauge,
  History,
  Image,
  ListPlus,
  Music,
  PauseCircle,
  Plus,
  Search,
  Info,
  Settings,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import { motion } from "framer-motion";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api } from "../services/api";
import { toastError } from "../stores/toast";
import type { AppNotification } from "../types";
import { date } from "../utils/format";
import { CATEGORY_NAV, STATUS_NAV, matches, type NavItem, type View } from "../app/views";
import { useDownloads } from "../stores/downloads";

const ICONS: Record<string, LucideIcon> = {
  all: DownloadIcon,
  progress: Gauge,
  completed: CheckCircle2,
  queued: Clock,
  scheduled: CalendarClock,
  paused: PauseCircle,
  failed: XCircle,
  "cat-video": Film,
  "cat-audio": Music,
  "cat-docs": FileText,
  "cat-images": Image,
  "cat-arch": Archive,
  "cat-soft": AppWindow,
  "cat-other": FolderOpen,
};

interface Props {
  view: View;
  onNavigate: (v: View) => void;
  query: string;
  onQuery: (q: string) => void;
  onAdd: () => void;
  onBatch: () => void;
  onVideo: () => void;
  children: ReactNode;
}

export function Shell({ view, onNavigate, query, onQuery, onAdd, onBatch, onVideo, children }: Props) {
  const { downloads, categories, queues, refreshQueues } = useDownloads();
  const main = queues.find((q) => q.isDefault);
  const queueAction = (action: "start" | "stop") => main && api.queueAction(main.id, action).then(refreshQueues, toastError);
  const counts = useMemo(() => {
    const list = Object.values(downloads);
    const c: Record<string, number> = {};
    for (const n of [...STATUS_NAV, ...CATEGORY_NAV]) c[n.id] = list.filter((d) => matches(d, n.filter!, categories, "")).length;
    return c;
  }, [downloads, categories]);

  const navButton = (id: string, label: string, Icon: LucideIcon, active: boolean, onClick: () => void, count?: number) => (
    <button
      key={id}
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={`relative flex h-8 w-full items-center gap-3 rounded-[var(--radius-ctl)] px-3 text-left text-[13px] transition-colors ${
        active ? "bg-hover font-medium" : "hover:bg-hover"
      }`}
    >
      {active && <motion.span layoutId="nav-indicator" className="absolute top-2 bottom-2 left-0 w-[3px] rounded-full bg-accent" transition={{ type: "spring", stiffness: 500, damping: 38 }} />}
      <Icon size={16} className={active ? "text-accent" : "text-fg-2"} strokeWidth={1.75} />
      <span className="flex-1 truncate">{label}</span>
      {count ? <span className="tabular text-[11px] text-fg-3">{count}</span> : null}
    </button>
  );
  const listNav = (n: NavItem) =>
    navButton(n.id, n.label, ICONS[n.id] ?? DownloadIcon, view.page === "list" && view.nav.id === n.id, () => onNavigate({ page: "list", nav: n }), counts[n.id]);

  return (
    <div className="flex h-full">
      <aside className="flex w-[232px] shrink-0 flex-col bg-pane px-2 pt-3 pb-2">
        <div className="mb-4 flex items-center gap-2 px-3">
          <svg width="20" height="20" viewBox="0 0 24 24" aria-hidden className="text-accent">
            <path fill="currentColor" d="M3 7h12a3 3 0 1 0-3-3h2a1 1 0 1 1 1 1H3zm0 5h16a3 3 0 1 1-3 3h2a1 1 0 1 0 1-1H3zm0 5h8v2H3z" />
          </svg>
          <span className="font-display text-[15px] font-semibold">Gale</span>
        </div>
        <nav className="flex flex-1 flex-col gap-0.5 overflow-y-auto" aria-label="Downloads">
          {navButton("dash", "Dashboard", Gauge, view.page === "dashboard", () => onNavigate({ page: "dashboard" }))}
          <div className="my-2 h-px bg-line" />
          {STATUS_NAV.map(listNav)}
          <div className="mt-4 mb-1 px-3 text-[11px] font-medium tracking-wide text-fg-3 uppercase">Categories</div>
          {CATEGORY_NAV.map(listNav)}
          <div className="my-2 h-px bg-line" />
          {navButton("queues", "Queues & Scheduler", ListOrdered, view.page === "queues", () => onNavigate({ page: "queues" }))}
          {navButton("grabber", "Site Grabber", Globe, view.page === "grabber", () => onNavigate({ page: "grabber" }))}
          {navButton("history", "Download History", History, view.page === "history", () => onNavigate({ page: "history" }))}
        </nav>
        {navButton("about", "About Gale", Info, view.page === "about", () => onNavigate({ page: "about" }))}
        {navButton("settings", "Settings", Settings, view.page === "settings", () => onNavigate({ page: "settings" }))}
      </aside>

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-12 shrink-0 items-center gap-1 border-b border-line px-3">
          <ToolbarButton icon={Plus} label="Add URL" onClick={onAdd} primary />
          <ToolbarButton icon={ListPlus} label="Add Batch" onClick={onBatch} />
          <ToolbarButton icon={Film} label="Add Video" onClick={onVideo} />
          <div className="mx-1 h-5 w-px bg-line" />
          <ToolbarButton icon={Play} label="Start Queue" onClick={() => queueAction("start")} disabled={!main || main.state === "running"} />
          <ToolbarButton icon={Square} label="Stop Queue" onClick={() => queueAction("stop")} disabled={!main || main.state === "stopped"} />
          <ToolbarButton icon={CalendarClock} label="Scheduler" onClick={() => onNavigate({ page: "queues" })} />
          <div className="flex-1" />
          <div className="relative w-60 shrink">
            <Search size={14} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-fg-3" />
            <input
              value={query}
              onChange={(e) => {
                onQuery(e.target.value);
                if (view.page !== "list") onNavigate({ page: "list", nav: STATUS_NAV[0]! });
              }}
              placeholder="Search downloads"
              aria-label="Search downloads"
              className="h-8 w-full rounded-[var(--radius-ctl)] border border-line bg-surface pr-2 pl-8 text-[13px] outline-none focus:border-b-2 focus:border-b-accent"
            />
          </div>
          <NotificationBell />
          <ToolbarButton icon={Settings} label="Settings" onClick={() => onNavigate({ page: "settings" })} iconOnly />
        </header>
        <div className="min-h-0 flex-1">{children}</div>
      </main>
    </div>
  );
}

function ToolbarButton({
  icon: Icon,
  label,
  onClick,
  primary,
  iconOnly,
  disabled,
}: {
  icon: LucideIcon;
  label: string;
  onClick: () => void;
  primary?: boolean;
  iconOnly?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={label}
      aria-label={label}
      className="flex h-8 items-center gap-2 rounded-[var(--radius-ctl)] px-2.5 text-[13px] hover:bg-hover disabled:opacity-35 disabled:hover:bg-transparent"
    >
      <Icon size={16} strokeWidth={1.75} className={primary ? "text-accent" : "text-fg-2"} />
      {!iconOnly && <span className="whitespace-nowrap">{label}</span>}
    </button>
  );
}

function NotificationBell() {
  const [items, setItems] = useState<AppNotification[]>([]);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const load = () => api.notifications().then(setItems, () => {});
  useEffect(() => {
    void load();
    const un = api.onNotification(() => void load());
    return () => void un.then((f) => f());
  }, []);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [open]);
  const unread = items.filter((n) => !n.isRead).length;
  const color = { info: "bg-accent", success: "bg-ok", warning: "bg-warn", error: "bg-bad" } as const;

  return (
    <div ref={ref} className="relative">
      <button
        aria-label={`Notifications${unread ? `, ${unread} unread` : ""}`}
        title="Notifications"
        onClick={() => {
          setOpen(!open);
          if (!open && unread) void api.markNotificationsRead().then(load);
        }}
        className="relative flex h-8 w-9 items-center justify-center rounded-[var(--radius-ctl)] hover:bg-hover"
      >
        <Bell size={16} strokeWidth={1.75} className="text-fg-2" />
        {unread > 0 && <span className="absolute top-1 right-1.5 h-2 w-2 rounded-full bg-accent" />}
      </button>
      {open && (
        <div className="absolute top-10 right-0 z-30 max-h-[420px] w-[360px] overflow-auto rounded-[var(--radius-surface)] border border-line bg-surface p-1 shadow-xl">
          {items.length === 0 && <p className="p-4 text-[12px] text-fg-3">No notifications.</p>}
          {items.map((n) => (
            <div key={n.id} className="flex gap-3 rounded-[var(--radius-ctl)] px-3 py-2.5 hover:bg-hover">
              <span className={`mt-1.5 h-2 w-2 shrink-0 rounded-full ${color[n.level]}`} />
              <div className="min-w-0 flex-1">
                <p className="truncate text-[13px]">{n.title}</p>
                {n.body && <p className="truncate text-[12px] text-fg-2">{n.body}</p>}
                <p className="text-[11px] text-fg-3">{date(n.createdAt)}</p>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
