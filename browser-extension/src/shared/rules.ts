// Pure interception logic, unit-tested in tests/rules.test.ts.

export type InterceptMode = "all" | "types";

export interface Settings {
  enabled: boolean;
  mode: InterceptMode;
  /** Lowercase extensions without dots. */
  fileTypes: string[];
  /** 0 = no minimum. Unknown sizes are not blocked by this. */
  minSizeMB: number;
  excludedDomains: string[];
  /** true = open Gale's Add dialog so the user confirms each download. */
  askFirst: boolean;
  sendCookies: boolean;
  /** Epoch ms; interception is paused until then. */
  pausedUntil: number;
  /** Let exactly one next download go through Chrome. */
  bypassNext: boolean;
  showMediaButton: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
  enabled: true,
  mode: "types",
  fileTypes: [
    "zip", "rar", "7z", "tar", "gz", "xz", "iso", "exe", "msi", "msix", "apk", "dmg",
    "mp4", "mkv", "webm", "avi", "mov", "mp3", "m4a", "flac", "wav", "ogg",
    "pdf", "epub", "docx", "xlsx", "pptx", "bin", "img",
  ],
  minSizeMB: 0,
  excludedDomains: [],
  askFirst: false,
  sendCookies: true,
  pausedUntil: 0,
  bypassNext: false,
  showMediaButton: true,
};

/** Rules owned by the desktop app (Settings → Browser). They always win. */
export interface AppRules {
  enabled: boolean;
  excludedDomains: string[];
  excludedTypes: string[];
}

export interface Candidate {
  url: string;
  filename: string;
  mime?: string;
  fileSize?: number; // -1 / undefined = unknown
}

export type Decision = { intercept: false; reason: string } | { intercept: true; silent: boolean };

export function extensionOf(name: string): string {
  const base = name.split(/[\\/]/).pop() ?? "";
  const i = base.lastIndexOf(".");
  return i > 0 ? base.slice(i + 1).toLowerCase() : "";
}

export function hostMatches(host: string, patterns: string[]): boolean {
  const h = host.toLowerCase();
  return patterns.some((p) => {
    const d = p.trim().toLowerCase().replace(/^\*\./, "");
    return d !== "" && (h === d || h.endsWith(`.${d}`));
  });
}

export function decide(c: Candidate, s: Settings, app: AppRules | null, now: number): Decision {
  if (!s.enabled) return { intercept: false, reason: "interception is off" };
  if (app && !app.enabled) return { intercept: false, reason: "browser integration is off in Gale" };
  if (s.pausedUntil > now) return { intercept: false, reason: "paused" };
  if (s.bypassNext) return { intercept: false, reason: "bypass requested" };
  let url: URL;
  try {
    url = new URL(c.url);
  } catch {
    return { intercept: false, reason: "invalid URL" };
  }
  // blob:, data:, filesystem: and similar exist only inside this browser.
  if (url.protocol !== "http:" && url.protocol !== "https:") return { intercept: false, reason: "not an http(s) download" };
  if (hostMatches(url.hostname, [...s.excludedDomains, ...(app?.excludedDomains ?? [])])) return { intercept: false, reason: "excluded site" };
  const ext = extensionOf(c.filename) || extensionOf(url.pathname);
  if (ext && app?.excludedTypes.map((t) => t.toLowerCase()).includes(ext)) return { intercept: false, reason: "excluded file type" };
  if (s.mode === "types" && !s.fileTypes.includes(ext)) return { intercept: false, reason: "file type not selected" };
  if (s.minSizeMB > 0 && c.fileSize != null && c.fileSize >= 0 && c.fileSize < s.minSizeMB * 1024 * 1024)
    return { intercept: false, reason: "smaller than minimum size" };
  return { intercept: true, silent: !s.askFirst };
}

/** Download-looking links on a page (for "download all links"). */
export function isDownloadLink(href: string, types: string[]): boolean {
  try {
    const u = new URL(href);
    return (u.protocol === "http:" || u.protocol === "https:") && types.includes(extensionOf(u.pathname));
  } catch {
    return false;
  }
}
