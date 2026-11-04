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
  | { type: "download-media"; url: string; filename?: string };

export interface MediaItem {
  url: string;
  mime: string;
  size: number | null;
  /** "direct" = downloadable file; "stream" = HLS/DASH playlist, not supported. */
  kind: "direct" | "stream";
}
