// The only module that talks to the Rust side.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  AppNotification,
  AppSettings,
  BatchResult,
  BrowserStatus,
  Category,
  CommandError,
  ConnectionTest,
  DashboardStats,
  Download,
  GrabOptions,
  GrabResult,
  HistoryEntry,
  MediaChoice,
  MediaInfo,
  MediaToolsInfo,
  ProbeResult,
  ProgressEvent,
  Queue,
  SchedulerJob,
  SegmentState,
  SettingsView,
} from "../types";

export interface AddOptions {
  url: string;
  saveDir: string;
  fileName?: string;
  connections?: number;
  expectedSha256?: string;
  referrer?: string;
  queueId?: number;
  start: boolean;
  media?: { choice: MediaChoice; kind: string; title?: string; expectedBytes?: number };
}

const on = <T>(event: string, cb: (p: T) => void): Promise<UnlistenFn> => listen<T>(event, (e) => cb(e.payload));

export const api = {
  listDownloads: () => invoke<Download[]>("list_downloads"),
  addDownload: (opts: AddOptions, pendingId?: string) => invoke<number>("add_download", { opts, pendingId }),
  addBatch: (urls: string[], saveDir: string, start: boolean, queueId?: number, source?: string) =>
    invoke<BatchResult[]>("add_batch", { urls, saveDir, start, queueId, source }),
  resume: (id: number) => invoke<void>("resume_download", { id }),
  pause: (id: number) => invoke<void>("pause_download", { id }),
  cancel: (id: number) => invoke<void>("cancel_download", { id }),
  restart: (id: number) => invoke<void>("restart_download", { id }),
  remove: (id: number, deleteFile: boolean) => invoke<void>("remove_download", { id, deleteFile }),
  recheck: (id: number) => invoke<ProbeResult>("recheck_download", { id }),
  open: (id: number) => invoke<void>("open_download", { id }),
  reveal: (id: number) => invoke<void>("reveal_download", { id }),
  segments: (id: number) => invoke<SegmentState[]>("get_download_segments", { id }),
  dashboard: () => invoke<DashboardStats>("dashboard"),
  categories: () => invoke<Category[]>("list_categories"),
  history: (limit?: number) => invoke<HistoryEntry[]>("list_history", { limit }),

  getSettings: () => invoke<SettingsView>("get_settings"),
  saveSettings: (settings: AppSettings) => invoke<SettingsView>("save_settings", { settings }),
  resetSettings: () => invoke<SettingsView>("reset_settings"),
  setProxyPassword: (password: string | null) => invoke<void>("set_proxy_password", { password }),
  testConnection: (url: string) => invoke<ConnectionTest>("test_connection", { url }),
  backupDatabase: (dest: string) => invoke<void>("backup_database", { dest }),
  restoreDatabase: (source: string) => invoke<void>("restore_database", { source }),
  openLogs: () => invoke<void>("open_logs"),
  browserStatus: () => invoke<BrowserStatus>("browser_status"),
  quit: () => invoke<void>("quit_app"),

  listQueues: () => invoke<Queue[]>("list_queues"),
  saveQueue: (id: number | null, queue: { name: string; priority: number; maxConcurrent: number; maxConnections: number }) =>
    invoke<number>("save_queue", { id, queue }),
  deleteQueue: (id: number) => invoke<void>("delete_queue", { id }),
  queueAction: (id: number, action: "start" | "stop" | "pause") => invoke<void>("queue_action", { id, action }),
  queueMemberships: () => invoke<[number, number][]>("queue_memberships"),
  setDownloadQueue: (downloadId: number, queueId: number | null) => invoke<void>("set_download_queue", { downloadId, queueId }),
  listJobs: () => invoke<SchedulerJob[]>("list_jobs"),
  saveJob: (job: SchedulerJob) => invoke<number>("save_job", { job }),
  deleteJob: (id: number) => invoke<void>("delete_job", { id }),
  cancelShutdown: () => invoke<void>("cancel_shutdown"),

  startGrab: (grabId: string, opts: GrabOptions) => invoke<GrabResult>("start_grab", { grabId, opts }),
  cancelGrab: (grabId: string) => invoke<void>("cancel_grab", { grabId }),

  getDownload: (id: number) => invoke<Download>("get_download", { id }),
  getPending: (pendingId: string) => invoke<{ url: string; fileName: string | null; referrer: string | null; fileSize: number | null; mime: string | null }>("get_pending", { pendingId }),
  probePending: (pendingId: string) => invoke<ProbeResult>("probe_pending", { pendingId }),
  showMain: () => invoke<void>("show_main_window"),
  /** Opens one of the About page links by name (the app holds the URLs). */
  openLink: (name: "website" | "github" | "linkedin" | "repo" | "issues" | "releases" | "privacy" | "terms" | "license" | "notices") =>
    invoke<void>("open_link", { name }),
  probeMedia: (url: string) => invoke<MediaInfo>("probe_media", { url }),
  mediaTools: () => invoke<MediaToolsInfo>("media_tools_info"),
  updateYtdlp: () => invoke<string>("update_ytdlp"),

  notifications: () => invoke<AppNotification[]>("list_notifications"),
  markNotificationsRead: () => invoke<void>("mark_notifications_read"),

  pickFolder: async (defaultPath?: string) => {
    const r = await open({ directory: true, multiple: false, defaultPath });
    return typeof r === "string" ? r : null;
  },
  pickFile: async () => {
    const r = await open({ multiple: false, filters: [{ name: "Gale backup", extensions: ["db", "sqlite"] }] });
    return typeof r === "string" ? r : null;
  },
  /** A local .torrent file, returned as a file: URL (the form the backend accepts). */
  pickTorrent: async () => {
    const r = await open({ multiple: false, filters: [{ name: "Torrent", extensions: ["torrent"] }] });
    return typeof r === "string" ? "file:///" + encodeURI(r.replace(/\\/g, "/").replace(/^\//, "")).replace(/#/g, "%23").replace(/\?/g, "%3F") : null;
  },
  pickSave: (defaultPath: string) => save({ defaultPath, filters: [{ name: "Gale backup", extensions: ["db"] }] }),

  onProgress: (cb: (e: ProgressEvent) => void) => on("download://progress", cb),
  onChanged: (cb: (d: Download) => void) => on("download://changed", cb),
  onBrowserBatch: (cb: (r: { urls: string[]; referrer: string | null }) => void) => on("browser://batch", cb),
  onBrowserMedia: (cb: (r: { url: string; title: string | null }) => void) => on("browser://media", cb),
  onNotification: (cb: () => void) => on("notification://new", cb),
  onShutdownCountdown: (cb: (seconds: number) => void) => on("system://shutdown-countdown", cb),
  onShutdownCancelled: (cb: () => void) => on("system://shutdown-cancelled", cb),
};

export function errorMessage(e: unknown): string {
  if (typeof e === "object" && e && "message" in e) return String((e as CommandError).message);
  return String(e);
}
