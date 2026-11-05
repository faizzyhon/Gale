import { create } from "zustand";
import { api } from "../services/api";
import type { AppSettings, Category, Download, Live, Queue, SettingsView } from "../types";

interface DownloadsState {
  ready: boolean;
  loadError: string | null;
  downloads: Record<number, Download>;
  live: Record<number, Live>;
  categories: Category[];
  settings: SettingsView | null;
  queues: Queue[];
  memberships: Record<number, number>;
  refreshQueues: () => Promise<void>;
  init: () => Promise<() => void>;
  refresh: () => Promise<void>;
  drop: (id: number) => void;
  setSettings: (s: SettingsView) => void;
}

const ACTIVE = new Set(["probing", "downloading"]);

export const useDownloads = create<DownloadsState>((set, get) => ({
  ready: false,
  loadError: null,
  downloads: {},
  live: {},
  categories: [],
  settings: null,
  queues: [],
  memberships: {},

  refreshQueues: async () => {
    const [queues, m] = await Promise.all([api.listQueues(), api.queueMemberships()]);
    set({ queues, memberships: Object.fromEntries(m) });
  },

  init: async () => {
    // Subscribe first so no event between the initial load and the listener is lost.
    const unProgress = await api.onProgress((e) => {
      const d = get().downloads[e.id];
      set((s) => ({
        live: { ...s.live, [e.id]: { bytesPerSec: e.bytesPerSec, activeConnections: e.activeConnections, segments: e.segments, stage: e.stage } },
        downloads: d
          ? { ...s.downloads, [e.id]: { ...d, downloadedBytes: e.written, totalBytes: e.total ?? d.totalBytes } }
          : s.downloads,
      }));
    });
    const unChanged = await api.onChanged((d) => {
      set((s) => {
        const live = { ...s.live };
        if (!ACTIVE.has(d.status)) delete live[d.id];
        return { downloads: { ...s.downloads, [d.id]: d }, live };
      });
      void get().refreshQueues(); // queue state follows download state
    });
    try {
      const [list, categories, settings] = await Promise.all([api.listDownloads(), api.categories(), api.getSettings()]);
      set({ downloads: Object.fromEntries(list.map((d) => [d.id, d])), categories, settings, ready: true });
      await get().refreshQueues();
      applyTheme(settings.theme);
    } catch (e) {
      set({ loadError: String((e as { message?: string }).message ?? e), ready: true });
    }
    return () => {
      unProgress();
      unChanged();
    };
  },

  refresh: async () => {
    const list = await api.listDownloads();
    set({ downloads: Object.fromEntries(list.map((d) => [d.id, d])) });
  },

  drop: (id) =>
    set((s) => {
      const downloads = { ...s.downloads };
      const live = { ...s.live };
      delete downloads[id];
      delete live[id];
      return { downloads, live };
    }),

  setSettings: (settings) => {
    set({ settings });
    applyTheme(settings.theme);
  },
}));

export function applyTheme(theme: AppSettings["theme"]) {
  if (theme === "system") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.setAttribute("data-theme", theme);
}
