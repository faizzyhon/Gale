// Mirrors the serde types in crates/gale-core and src-tauri. Keep in sync.

export type DownloadStatus =
  | "queued"
  | "probing"
  | "downloading"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled"
  | "scheduled";

export interface Download {
  id: number;
  url: string;
  finalUrl: string | null;
  referrer: string | null;
  fileName: string;
  saveDir: string;
  tempPath: string | null;
  finalPath: string | null;
  mimeType: string | null;
  totalBytes: number | null;
  downloadedBytes: number;
  status: DownloadStatus;
  resumable: boolean;
  connections: number;
  etag: string | null;
  lastModified: string | null;
  sha256: string | null;
  expectedSha256: string | null;
  errorCode: string | null;
  errorMessage: string | null;
  categoryId: number | null;
  source: string;
  createdAt: string;
  startedAt: string | null;
  completedAt: string | null;
}

export interface SegmentState {
  index: number;
  start: number;
  end: number | null;
  written: number;
}

export interface ProgressEvent {
  id: number;
  written: number;
  total: number | null;
  bytesPerSec: number;
  activeConnections: number;
  segments: SegmentState[];
}

export interface Live {
  bytesPerSec: number;
  activeConnections: number;
  segments: SegmentState[];
}

export interface Category {
  id: number;
  name: string;
  extensions: string;
  subfolder: string;
  isBuiltin: boolean;
  sortOrder: number;
}

export interface DashboardStats {
  total: number;
  active: number;
  completed: number;
  failed: number;
  totalDownloadedBytes: number;
  perCategory: [string, number][];
  dailyBytes: [string, number][];
}

export interface HistoryEntry {
  id: number;
  downloadId: number | null;
  url: string;
  fileName: string;
  finalPath: string | null;
  bytes: number;
  outcome: "completed" | "failed" | "cancelled";
  durationMs: number | null;
  completedAt: string;
}

export interface ProbeResult {
  finalUrl: string;
  total: number | null;
  ranges: boolean;
  etag: string | null;
  lastModified: string | null;
  fileName: string;
  mime: string | null;
}

export type ProxyKind = "none" | "http" | "https" | "socks5";

export interface AppSettings {
  theme: "system" | "light" | "dark";
  downloadDir: string;
  categoryFolders: boolean;
  notifications: boolean;
  startWithWindows: boolean;
  minimizeToTray: boolean;
  connections: 1 | 2 | 4 | 8 | 16 | 32;
  maxSimultaneous: number;
  maxRetries: number;
  browserEnabled: boolean;
  browserExcludedDomains: string[];
  browserExcludedTypes: string[];
  proxyKind: ProxyKind;
  proxyHost: string;
  proxyPort: number;
  proxyUser: string;
  connectTimeoutSecs: number;
  readTimeoutSecs: number;
  userAgent: string;
  globalLimitKbps: number;
  perDownloadLimitKbps: number;
  perHostConnections: number;
  markOfTheWeb: boolean;
  defenderScan: boolean;
  computeSha256: boolean;
  blockedExtensions: string[];
}

export interface SettingsView extends AppSettings {
  hasProxyPassword: boolean;
}

export interface Queue {
  id: number;
  name: string;
  priority: number;
  maxConcurrent: number;
  maxConnections: number;
  state: "running" | "stopped" | "paused";
  isDefault: boolean;
  createdAt: string;
}

export interface SchedulerJob {
  id: number;
  queueId: number;
  kind: "start_queue" | "stop_queue";
  runAt: string;
  recurrence: string | null;
  enabled: boolean;
  shutdownAfter: boolean;
  lastRunAt?: string | null;
}

export interface AppNotification {
  id: number;
  level: "info" | "success" | "warning" | "error";
  title: string;
  body: string | null;
  downloadId: number | null;
  isRead: boolean;
  createdAt: string;
}

export interface GrabOptions {
  startUrl: string;
  maxDepth: number;
  sameDomain: boolean;
  extensions: string[];
  include?: string;
  exclude?: string;
  maxPages: number;
  delayMs: number;
}

export interface GrabResource {
  url: string;
  fileName: string;
  extension: string;
  foundOn: string;
}

export interface GrabResult {
  resources: GrabResource[];
  pagesVisited: number;
  pagesBlockedByRobots: number;
}

export interface ConnectionTest {
  ok: boolean;
  millis: number;
  resumable: boolean;
  size: number | null;
  message: string;
}

export interface BrowserStatus {
  lastContact: string | null;
  intercepted: number;
}

export interface BrowserAddRequest {
  pendingId: string;
  url: string;
  fileName: string | null;
  referrer: string | null;
}

export interface BatchResult {
  url: string;
  id: number | null;
  error: string | null;
}

export interface CommandError {
  code: string;
  message: string;
}
