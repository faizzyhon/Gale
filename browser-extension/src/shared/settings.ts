import { DEFAULT_SETTINGS, type AppRules, type Settings } from "./rules";

export async function loadSettings(): Promise<Settings> {
  const { settings } = await chrome.storage.sync.get("settings");
  return { ...DEFAULT_SETTINGS, ...(settings as Partial<Settings> | undefined) };
}

export async function saveSettings(patch: Partial<Settings>): Promise<Settings> {
  const next = { ...(await loadSettings()), ...patch };
  await chrome.storage.sync.set({ settings: next });
  return next;
}

export async function cachedAppRules(): Promise<AppRules | null> {
  const { appRules } = await chrome.storage.local.get("appRules");
  return (appRules as AppRules | undefined) ?? null;
}

/** Messages exchanged between popup/options/content and the service worker. */
export type WorkerMessage =
  | { type: "status" }
  | { type: "recent" }
  | { type: "show-app" }
  | { type: "media-list" }
  | { type: "download-media"; url: string; filename?: string }
  | { type: "download-page-video" }
  | { type: "media-formats" }
  | { type: "download-media-format"; title?: string; choice: MediaChoice; kind: string; estimatedSize?: number | null };

/** Gale's quality choice for a video page (mirrors gale-core MediaChoice). */
export type MediaChoice = { type: "video"; maxHeight: number | null } | { type: "audio"; mp3: boolean };

/** One entry of the quality menu, as returned by Gale's probe_media. */
export interface QualityOption {
  label: string;
  choice: MediaChoice;
  kind: string;
  estimatedSize: number | null;
  detail: string;
}

export interface MediaItem {
  url: string;
  mime: string;
  size: number | null;
  /** "direct" = plain media file (direct engine); "stream" = HLS/DASH playlist (Gale's video pipeline). */
  kind: "direct" | "stream";
}
