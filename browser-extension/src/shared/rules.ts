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
  /** Send magnet links and .torrent files to Gale, whatever the file type list says. */
  catchTorrents: boolean;
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
  askFirst: true,
  sendCookies: true,
  pausedUntil: 0,
  bypassNext: false,
  showMediaButton: true,
  catchTorrents: true,
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

/** Switches that apply to every kind of capture; null = go ahead. */
function gate(s: Settings, app: AppRules | null, now: number): Decision | null {
  if (!s.enabled) return { intercept: false, reason: "interception is off" };
  if (app && !app.enabled) return { intercept: false, reason: "browser integration is off in Gale" };
  if (s.pausedUntil > now) return { intercept: false, reason: "paused" };
  return null;
}

export function isTorrent(c: Pick<Candidate, "filename" | "mime" | "url">): boolean {
  let path = "";
  try {
    path = new URL(c.url).pathname;
  } catch {
    /* checked by the caller */
  }
  return c.mime === "application/x-bittorrent" || extensionOf(c.filename) === "torrent" || extensionOf(path) === "torrent";
}

/** A click on a magnet: link on `pageUrl`. */
export function decideMagnet(pageUrl: string, s: Settings, app: AppRules | null, now: number): Decision {
  const g = gate(s, app, now);
  if (g) return g;
  if (!s.catchTorrents) return { intercept: false, reason: "torrents are off" };
  try {
    if (hostMatches(new URL(pageUrl).hostname, [...s.excludedDomains, ...(app?.excludedDomains ?? [])])) return { intercept: false, reason: "excluded site" };
  } catch {
    /* no page address: nothing to exclude */
  }
  return { intercept: true, silent: !s.askFirst };
}

export function decide(c: Candidate, s: Settings, app: AppRules | null, now: number): Decision {
  const g = gate(s, app, now);
  if (g) return g;
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
  const torrent = s.catchTorrents && isTorrent(c);
  if (s.mode === "types" && !s.fileTypes.includes(ext) && !torrent) return { intercept: false, reason: "file type not selected" };
  if (!torrent && s.minSizeMB > 0 && c.fileSize != null && c.fileSize >= 0 && c.fileSize < s.minSizeMB * 1024 * 1024)
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

// Pages that are one video on sites people download from. The quality menu is prepared as soon as such a page
// opens; elsewhere only when the pointer reaches a video, so news-site autoplay clips don't start yt-dlp.
const VIDEO_PAGES: [RegExp, RegExp][] = [
  [/(^|\.)youtube\.com$/, /^\/(watch|shorts\/|live\/|embed\/)/],
  [/^youtu\.be$/, /^\/[\w-]{6,}/],
  [/(^|\.)vimeo\.com$/, /^\/(\d+|video\/\d+)/],
  [/(^|\.)dailymotion\.com$/, /^\/video\//],
  [/(^|\.)twitch\.tv$/, /^\/(videos\/\d+|\w+\/clip\/)/],
  [/(^|\.)facebook\.com$/, /\/(videos|reel)\/|^\/watch/],
  [/(^|\.)instagram\.com$/, /^\/(reel|p)\//],
  [/(^|\.)tiktok\.com$/, /\/video\/\d+/],
  [/(^|\.)(x|twitter)\.com$/, /\/status\/\d+/],
  [/(^|\.)reddit\.com$/, /\/comments\//],
  [/(^|\.)bilibili\.com$/, /^\/video\//],
  [/(^|\.)rumble\.com$/, /^\/v[\w-]+/],
];

export function videoPageKind(url: string): "video" | "site" | "other" {
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return "other";
  }
  const site = VIDEO_PAGES.find(([h]) => h.test(u.hostname.toLowerCase()));
  if (!site) return "other";
  return site[1].test(u.pathname) ? "video" : "site";
}
