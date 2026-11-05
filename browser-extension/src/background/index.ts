// Service worker: download interception, context menus, media detection, popup/content routing.
import { mediaBlocked } from "../shared/store";
import { decide, isDownloadLink, type AppRules } from "../shared/rules";
import { type NativeResponse, native } from "../shared/native";
import { cachedAppRules, loadSettings, saveSettings, type MediaItem, type WorkerMessage } from "../shared/settings";

const MENU = {
  link: "gale-link",
  queue: "gale-queue",
  media: "gale-media",
  selection: "gale-selection",
  page: "gale-page",
  copy: "gale-copy",
  pageVideo: "gale-page-video",
} as const;

// ---------------------------------------------------------------------------------------------
// Setup

// Qualities per page, so reopening the menu doesn't run yt-dlp again (and two clicks share one lookup).
const formatCache = new Map<string, { at: number; reply: Promise<NativeResponse> }>();
function probeFormats(url: string, title?: string) {
  const now = Date.now();
  for (const [k, v] of formatCache) if (now - v.at > 120_000) formatCache.delete(k);
  const hit = formatCache.get(url);
  if (hit) return hit.reply;
  const reply = native("probe_media", { url, title: title || undefined });
  formatCache.set(url, { at: now, reply });
  void reply.then((r) => r.ok || formatCache.delete(url)); // retry failures next time
  return reply;
}

chrome.runtime.onInstalled.addListener(() => {
  // Tabs opened before an install/update still run the old (now disconnected) content script: give them the new one.
  void chrome.tabs.query({ url: ["http://*/*", "https://*/*"] }).then((tabs) =>
    tabs.forEach((t) => t.id && chrome.scripting.executeScript({ target: { tabId: t.id }, files: ["content.js"] }).catch(() => {})),
  );
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({ id: MENU.link, title: "Download with Gale", contexts: ["link"] });
    chrome.contextMenus.create({ id: MENU.queue, title: "Add to Gale queue", contexts: ["link"] });
    chrome.contextMenus.create({ id: MENU.media, title: "Download media with Gale", contexts: ["video", "audio", "image"] });
    chrome.contextMenus.create({ id: MENU.selection, title: "Download selected links with Gale", contexts: ["selection"] });
    chrome.contextMenus.create({ id: MENU.page, title: "Download all links on this page with Gale", contexts: ["page"] });
    chrome.contextMenus.create({ id: MENU.pageVideo, title: "Download video on this page with Gale", contexts: ["page", "video"] });
    chrome.contextMenus.create({ id: MENU.copy, title: "Copy download URL", contexts: ["link", "video", "audio"] });
  });
  void refreshAppRules();
});
chrome.runtime.onStartup.addListener(() => void refreshAppRules());

async function refreshAppRules(): Promise<AppRules | null> {
  const r = await native<AppRules>("get_rules");
  if (r.ok && r.result) {
    await chrome.storage.local.set({ appRules: r.result, appSeenAt: Date.now() });
    return r.result;
  }
  return cachedAppRules();
}

async function cookieHeader(url: string): Promise<string | undefined> {
  try {
    const cookies = await chrome.cookies.getAll({ url });
    return cookies.length ? cookies.map((c) => `${c.name}=${c.value}`).join("; ") : undefined;
  } catch {
    return undefined;
  }
}

async function sendDownload(url: string, opts: { filename?: string; referrer?: string; pageUrl?: string; size?: number; mime?: string; silent: boolean; enqueue?: boolean }) {
  const s = await loadSettings();
  return native("add_download", {
    url,
    filename: opts.filename || undefined,
    referrer: opts.referrer || undefined,
    pageUrl: opts.pageUrl || undefined,
    fileSize: opts.size != null && opts.size >= 0 ? opts.size : undefined,
    mime: opts.mime || undefined,
    cookies: s.sendCookies ? await cookieHeader(url) : undefined,
    userAgent: navigator.userAgent,
    silent: opts.silent,
    enqueue: opts.enqueue ?? false,
  });
}

// ---------------------------------------------------------------------------------------------
// Interception. onDeterminingFilename fires after response headers arrive, so size and MIME are known.

const recentlySent = new Map<string, number>();

chrome.downloads.onDeterminingFilename.addListener((item, suggest) => {
  void handleDownload(item).finally(() => {
    try {
      suggest();
    } catch {
      /* already cancelled */
    }
  });
  return true; // we call suggest asynchronously
});

async function handleDownload(item: chrome.downloads.DownloadItem): Promise<void> {
  if (item.byExtensionId === chrome.runtime.id) return; // our own fallbacks
  const s = await loadSettings();
  const url = item.finalUrl || item.url;
  const d = decide({ url, filename: item.filename, mime: item.mime, fileSize: item.fileSize }, s, await cachedAppRules(), Date.now());
  if (s.bypassNext) await saveSettings({ bypassNext: false });
  if (!d.intercept) return;

  // The same click can surface twice (redirects, double clicks): send once.
  const now = Date.now();
  const last = recentlySent.get(url);
  recentlySent.set(url, now);
  for (const [k, t] of recentlySent) if (now - t > 10_000) recentlySent.delete(k);
  if (last && now - last < 5_000) {
    await cancelInChrome(item.id);
    return;
  }

  const r = await sendDownload(url, { filename: item.filename, referrer: item.referrer, size: item.fileSize, mime: item.mime, silent: d.silent });
  if (r.ok) {
    await cancelInChrome(item.id);
  } else {
    // Gale missing, closed, disabled or the URL was a duplicate: Chrome keeps the download.
    recentlySent.delete(url);
  }
}

async function cancelInChrome(id: number) {
  try {
    await chrome.downloads.cancel(id);
    await chrome.downloads.erase({ id });
  } catch {
    /* finished or gone */
  }
}

// ---------------------------------------------------------------------------------------------
// Context menus

chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  const pageUrl = info.pageUrl ?? tab?.url;
  const s = await loadSettings();
  switch (info.menuItemId) {
    case MENU.link:
      if (info.linkUrl) await sendDownload(info.linkUrl, { referrer: pageUrl, pageUrl, silent: !s.askFirst });
      break;
    case MENU.queue:
      if (info.linkUrl) await sendDownload(info.linkUrl, { referrer: pageUrl, pageUrl, silent: true, enqueue: true });
      break;
    case MENU.media:
      if (info.srcUrl) await sendDownload(info.srcUrl, { referrer: pageUrl, pageUrl, silent: !s.askFirst });
      break;
    case MENU.selection:
    case MENU.page: {
      if (!tab?.id) return;
      const all = info.menuItemId === MENU.page;
      const [res] = await chrome.scripting.executeScript({ target: { tabId: tab.id }, func: collectLinks, args: [all] });
      let urls = [...new Set((res?.result as string[] | undefined) ?? [])];
      if (all) urls = urls.filter((u) => isDownloadLink(u, s.fileTypes));
      if (urls.length === 0) return;
      // Gale shows the batch for review before anything starts.
      await native("add_batch", { items: urls.slice(0, 2000).map((url) => ({ url })), referrer: pageUrl, pageUrl });
      break;
    }
    case MENU.pageVideo:
      if (tab?.url && !mediaBlocked(tab.url)) await native("add_media", { url: tab.url, title: tab.title || undefined });
      break;
    case MENU.copy: {
      const u = info.linkUrl ?? info.srcUrl;
      if (u && tab?.id) await chrome.scripting.executeScript({ target: { tabId: tab.id }, func: (t: string) => navigator.clipboard.writeText(t), args: [u] });
      break;
    }
  }
});

/** Runs in the page: links inside the current selection, or every link on the page. */
function collectLinks(all: boolean): string[] {
  const anchors = Array.from(document.querySelectorAll<HTMLAnchorElement>("a[href]"));
  if (all) return anchors.map((a) => a.href);
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0) return [];
  const out: string[] = [];
  for (let i = 0; i < sel.rangeCount; i++) {
    const r = sel.getRangeAt(i);
    for (const a of anchors) if (r.intersectsNode(a)) out.push(a.href);
  }
  // Plain-text URLs in the selection count too.
  for (const m of sel.toString().matchAll(/https?:\/\/[^\s"'<>]+/g)) out.push(m[0]);
  return out;
}

// ---------------------------------------------------------------------------------------------
// Media detection from network responses (works for players that use <video src> and XHR/fetch)

const media = new Map<number, MediaItem[]>();
const STREAM_TYPES = ["application/vnd.apple.mpegurl", "application/x-mpegurl", "audio/mpegurl", "application/dash+xml"];

chrome.webRequest.onHeadersReceived.addListener(
  (d) => {
    if (d.tabId < 0) return;
    const h = (n: string) => d.responseHeaders?.find((x) => x.name.toLowerCase() === n)?.value ?? "";
    const mime = h("content-type").split(";")[0]!.trim().toLowerCase();
    const isStream = STREAM_TYPES.includes(mime) || /\.(m3u8|mpd)(\?|$)/i.test(d.url);
    const isDirect = /^(video|audio)\//.test(mime) && mime !== "video/mp2t" && mime !== "video/iso.segment";
    if (!isStream && !isDirect) return;
    const total = h("content-range").split("/")[1];
    const size = total && total !== "*" ? Number(total) : Number(h("content-length")) || null;
    if (isDirect && size != null && size < 256 * 1024) return; // segments, previews, notification sounds
    const url = d.url;
    const list = media.get(d.tabId) ?? [];
    if (!list.some((m) => m.url === url)) {
      list.push({ url, mime: mime || "application/octet-stream", size, kind: isStream ? "stream" : "direct" });
      media.set(d.tabId, list.slice(-30));
      void chrome.action.setBadgeText({ tabId: d.tabId, text: String(list.filter((m) => m.kind === "direct").length || "") });
      void chrome.action.setBadgeBackgroundColor({ color: "#005fb8" });
    }
  },
  { urls: ["http://*/*", "https://*/*"], types: ["media", "xmlhttprequest", "other"] },
  ["responseHeaders"],
);

chrome.tabs.onUpdated.addListener((tabId, change) => {
  if (change.url) media.delete(tabId);
});
chrome.tabs.onRemoved.addListener((tabId) => media.delete(tabId));

// ---------------------------------------------------------------------------------------------
// Messages from popup, options and content script

chrome.runtime.onMessage.addListener((msg: WorkerMessage, sender, reply) => {
  (async () => {
    switch (msg.type) {
      case "status": {
        const r = await native<{ version: string }>("ping");
        if (r.ok) void refreshAppRules();
        return r;
      }
      case "recent":
        return native("recent", { limit: 8 });
      case "show-app":
        return native("show_app");
      case "media-list":
        return mediaBlocked(sender.tab?.url) ? [] : (media.get(sender.tab?.id ?? -1) ?? []);
      case "download-page-video":
        if (mediaBlocked(sender.tab?.url)) return { ok: false, error: { code: "unsupported", message: "Not available on this site." } };
        // Gale reads the page with yt-dlp and lets the user pick a quality (HLS/DASH, YouTube, …).
        return sender.tab?.url ? native("add_media", { url: sender.tab.url, title: sender.tab.title || undefined }) : { ok: false };
      case "media-formats": {
        const url = sender.tab?.url;
        if (!url || mediaBlocked(url)) return { ok: false, error: { code: "unsupported", message: "Not available on this site." } };
        return probeFormats(url, sender.tab?.title);
      }
      case "download-media-format": {
        const url = sender.tab?.url;
        if (!url || mediaBlocked(url)) return { ok: false, error: { code: "unsupported", message: "Not available on this site." } };
        return native("download_media", { url, title: msg.title || undefined, choice: msg.choice, kind: msg.kind, estimatedSize: msg.estimatedSize ?? undefined });
      }
      case "download-media":
        if (mediaBlocked(sender.tab?.url) || mediaBlocked(msg.url)) return { ok: false, error: { code: "unsupported", message: "Not available on this site." } };
        return sendDownload(msg.url, { filename: msg.filename, referrer: sender.tab?.url, pageUrl: sender.tab?.url, silent: true });
    }
  })().then(reply, (e) => reply({ ok: false, error: { code: "internal", message: String(e) } }));
  return true;
});
