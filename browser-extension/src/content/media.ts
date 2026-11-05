// Floating "Download" button on video/audio elements. Protected (EME/DRM) media is never offered.
import type { MediaItem, QualityOption } from "../shared/settings";
import { loadSettings } from "../shared/settings";
import { mediaBlocked } from "../shared/store";

const host = document.createElement("gale-media-ui");
const shadow = host.attachShadow({ mode: "closed" });
shadow.innerHTML = `
<style>
  :host { all: initial; }
  .btn { position: fixed; z-index: 2147483646; display: none; align-items: center; gap: 6px; height: 30px; padding: 0 12px;
         border: 0; border-radius: 4px; background: #005fb8; color: #fff; font: 600 12px "Segoe UI", system-ui, sans-serif;
         cursor: pointer; box-shadow: 0 2px 8px rgba(0,0,0,.3); }
  .btn:hover { background: #0a6cc9; }
  .panel { position: fixed; z-index: 2147483647; display: none; width: 340px; max-height: 360px; overflow: auto; padding: 12px;
           border-radius: 8px; background: #fff; color: #1b1b1b; font: 13px "Segoe UI", system-ui, sans-serif;
           box-shadow: 0 8px 32px rgba(0,0,0,.28); }
  .title { font-weight: 600; margin: 0 0 8px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row { display: flex; align-items: center; gap: 8px; padding: 8px 0; border-top: 1px solid #eee; }
  .meta { flex: 1; min-width: 0; color: #5c5c5c; font-size: 12px; }
  .row button { height: 26px; padding: 0 10px; border: 0; border-radius: 4px; background: #005fb8; color: #fff; cursor: pointer; font: inherit; }
  .note { color: #5c5c5c; font-size: 12px; margin: 6px 0 0; }
  .note.err { color: #b3261e; }
  .head { margin: 10px 0 4px; font-size: 11px; font-weight: 600; letter-spacing: .04em; text-transform: uppercase; color: #5c5c5c; }
  .opt { display: grid; grid-template-columns: 128px minmax(0, 1fr) auto; width: 100%; align-items: center; gap: 10px; padding: 8px 8px; margin: 0; border: 0; border-radius: 6px;
         background: transparent; color: inherit; font: inherit; text-align: left; cursor: pointer; }
  .opt:hover:not(:disabled), .opt:focus-visible { background: #e8f1fb; outline: none; }
  .opt:disabled { cursor: default; opacity: .55; }
  .opt.done { opacity: 1; background: #e7f5ea; }
  .opt b { font-weight: 600; white-space: nowrap; }
  .opt span { flex: 1; min-width: 0; color: #5c5c5c; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .opt small { color: #5c5c5c; font-size: 12px; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .spin { display: inline-block; width: 12px; height: 12px; margin-right: 6px; vertical-align: -2px; border: 2px solid #c9d6e6;
          border-top-color: #005fb8; border-radius: 50%; animation: s .8s linear infinite; }
  @keyframes s { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spin { animation: none; } }
</style>
<button class="btn" part="btn">⭳ Download ▾</button>
<div class="panel" role="dialog" aria-label="Download media"></div>`;
const btn = shadow.querySelector<HTMLButtonElement>(".btn")!;
const panel = shadow.querySelector<HTMLDivElement>(".panel")!;
let current: HTMLMediaElement | null = null;
let hideTimer = 0;

void loadSettings().then((s) => {
  if (!s.showMediaButton || mediaBlocked(location.href)) return;
  // After an extension update the old copy of this script is orphaned: one button per page, the newest wins.
  document.querySelectorAll("gale-media-ui").forEach((e) => e.remove());
  document.documentElement.appendChild(host);
});

type Reply = { ok: boolean; error?: { message: string } };
/** Message the background; an orphaned script (extension reloaded/updated) removes its UI instead of throwing. */
async function send<T>(msg: object, fallback: T): Promise<T> {
  try {
    if (chrome.runtime?.id) return (await chrome.runtime.sendMessage(msg)) as T;
  } catch {
    /* "Extension context invalidated" */
  }
  host.remove();
  return fallback;
}
const RELOAD: Reply = { ok: false, error: { message: "Gale's extension was updated. Reload this page." } };

function place(el: HTMLElement) {
  const r = el.getBoundingClientRect();
  if (r.width < 160 || r.height < 60) return false;
  btn.style.top = `${Math.max(8, r.top + 8)}px`;
  btn.style.left = `${Math.min(window.innerWidth - 120, r.right - 120)}px`;
  return true;
}

document.addEventListener(
  "mouseover",
  (e) => {
    if (!chrome.runtime?.id) return host.remove(); // orphaned by an extension update
    if (!host.isConnected) return;
    // Players like YouTube cover the <video> with control layers, so look at everything under the cursor.
    const el = ((e.target as Element | null)?.closest?.("video, audio") ??
      document.elementsFromPoint(e.clientX, e.clientY).find((x) => x instanceof HTMLMediaElement)) as HTMLMediaElement | null;
    if (!el || !place(el)) return;
    current = el;
    clearTimeout(hideTimer);
    btn.style.display = "flex";
    hideTimer = window.setTimeout(() => (btn.style.display = "none"), 4000);
  },
  { passive: true },
);

const fmt = (n: number | null) => (n == null ? "size unknown" : n > 1 << 30 ? `${(n / 2 ** 30).toFixed(2)} GB` : `${(n / 2 ** 20).toFixed(1)} MB`);
const extOf = (mime: string, url: string) => (url.match(/\.([a-z0-9]{2,5})(\?|$)/i)?.[1] ?? mime.split("/")[1] ?? "").toUpperCase();

btn.addEventListener("click", async () => {
  const el = current;
  if (!el) return;
  panel.replaceChildren();
  const title = document.createElement("p");
  title.className = "title";
  title.textContent = document.title || location.hostname;
  panel.append(title);

  const items: MediaItem[] = [];
  const src = el.currentSrc;
  const isPlaylist = /\.(m3u8|mpd)(\?|$)/i.test(src);
  if (/^https?:/.test(src) && !el.mediaKeys && !isPlaylist) items.push({ url: src, mime: el instanceof HTMLVideoElement ? "video/*" : "audio/*", size: null, kind: "direct" });
  const seen = await send<MediaItem[]>({ type: "media-list" }, []);
  for (const m of seen) {
    const same = items.find((i) => i.url === m.url);
    if (same) Object.assign(same, { size: same.size ?? m.size, mime: m.mime }); // network saw the real size/type
    else items.push(m);
  }
  const direct = el.mediaKeys ? [] : items.filter((m) => m.kind === "direct");
  const quality = el instanceof HTMLVideoElement && el.videoHeight ? `${el.videoHeight}p` : "";
  for (const m of direct) {
    row(`${[extOf(m.mime, m.url), m.url === src ? quality : "", fmt(m.size)].filter(Boolean).join(" · ")}`, "Download", async (b) => {
      const r = await send<Reply>({ type: "download-media", url: m.url }, RELOAD);
      b.textContent = r.ok ? "Sent to Gale" : "Failed";
      if (!r.ok) note(r.error?.message ?? "Gale did not accept the download.");
    });
  }
  if (!host.isConnected) return; // orphaned: nothing can be sent from this copy
  const r = btn.getBoundingClientRect();
  panel.style.top = `${r.bottom + 6}px`;
  panel.style.left = `${Math.max(8, r.right - 340)}px`;
  panel.style.display = "block";
  // Streamed (HLS/DASH), MSE blob: players and sites like YouTube: Gale lists the qualities right here.
  if (direct.length === 0 || isPlaylist || items.some((m) => m.kind === "stream") || src.startsWith("blob:")) await qualityMenu();
});

type Formats = { ok: boolean; result?: { title: string; isLive: boolean; drmOnly: boolean; options: QualityOption[] }; error?: { code: string; message: string } };

async function qualityMenu() {
  const head = document.createElement("p");
  head.className = "head";
  head.textContent = "Choose quality";
  const wait = document.createElement("p");
  wait.className = "note";
  wait.innerHTML = '<span class="spin"></span>Finding qualities…';
  panel.append(head, wait);
  const r = await send<Formats>({ type: "media-formats" }, RELOAD as Formats);
  wait.remove();
  const info = r.result;
  if (!r.ok || !info) {
    note(r.error?.code === "host_unavailable" ? "Gale isn't running. Start Gale and try again." : (r.error?.message ?? "Gale couldn't read this video."), true);
    row("Pick the quality in Gale instead", "Open in Gale", async (b) => {
      const o = await send<Reply>({ type: "download-page-video" }, RELOAD);
      b.textContent = o.ok ? "Opened in Gale" : "Failed";
    });
    return;
  }
  if (info.drmOnly) return note("This video is DRM-protected and can't be downloaded.", true);
  if (info.isLive) return note("Live streams can't be downloaded.", true);
  if (!info.options.length) return note("Gale found no downloadable formats on this page.", true);
  const buttons: HTMLButtonElement[] = [];
  for (const o of info.options) {
    const b = document.createElement("button");
    b.className = "opt";
    const label = document.createElement("b");
    label.textContent = o.label;
    const detail = document.createElement("span");
    detail.textContent = o.detail;
    const size = document.createElement("small");
    size.textContent = o.estimatedSize ? `~${fmt(o.estimatedSize)}` : "";
    b.append(label, detail, size);
    b.onclick = async () => {
      buttons.forEach((x) => (x.disabled = true));
      size.textContent = "Starting…";
      const res = await send<Reply>({ type: "download-media-format", title: info.title, choice: o.choice, kind: o.kind, estimatedSize: o.estimatedSize }, RELOAD);
      if (res.ok) {
        b.classList.add("done");
        size.textContent = "✓ Downloading";
        setTimeout(() => (panel.style.display = "none"), 1500);
      } else {
        buttons.forEach((x) => (x.disabled = false));
        size.textContent = o.estimatedSize ? `~${fmt(o.estimatedSize)}` : "";
        note(res.error?.message ?? "Gale did not accept the download.", true);
      }
    };
    buttons.push(b);
    panel.append(b);
  }
}

function row(meta: string, label: string, onClick: (b: HTMLButtonElement) => Promise<void>) {
  const r = document.createElement("div");
  r.className = "row";
  const m = document.createElement("div");
  m.className = "meta";
  m.textContent = meta;
  const b = document.createElement("button");
  b.textContent = label;
  b.onclick = () => {
    b.disabled = true;
    void onClick(b);
  };
  r.append(m, b);
  panel.append(r);
}

function note(text: string, error = false) {
  const p = document.createElement("p");
  p.className = error ? "note err" : "note";
  p.textContent = text;
  panel.append(p);
}

document.addEventListener("mousedown", (e) => {
  if (!e.composedPath().includes(host)) panel.style.display = "none";
});
