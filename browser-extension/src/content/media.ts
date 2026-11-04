// Floating "Download" button on video/audio elements. Protected (EME/DRM) media is never offered.
import type { MediaItem } from "../shared/settings";
import { loadSettings } from "../shared/settings";

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
</style>
<button class="btn" part="btn">⭳ Download</button>
<div class="panel" role="dialog" aria-label="Download media"></div>`;
const btn = shadow.querySelector<HTMLButtonElement>(".btn")!;
const panel = shadow.querySelector<HTMLDivElement>(".panel")!;
let current: HTMLMediaElement | null = null;
let hideTimer = 0;

void loadSettings().then((s) => {
  if (s.showMediaButton) document.documentElement.appendChild(host);
});

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
    const el = (e.target as Element | null)?.closest?.("video, audio") as HTMLMediaElement | null;
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

  if (el.mediaKeys) {
    note("This media is protected (DRM) and can't be downloaded.");
  } else {
    const items: MediaItem[] = [];
    const src = el.currentSrc;
    if (/^https?:/.test(src)) items.push({ url: src, mime: el instanceof HTMLVideoElement ? "video/*" : "audio/*", size: null, kind: "direct" });
    const seen = (await chrome.runtime.sendMessage({ type: "media-list" })) as MediaItem[];
    for (const m of seen) {
      const same = items.find((i) => i.url === m.url);
      if (same) Object.assign(same, { size: same.size ?? m.size, mime: m.mime }); // network saw the real size/type
      else items.push(m);
    }
    const direct = items.filter((m) => m.kind === "direct");
    const quality = el instanceof HTMLVideoElement && el.videoHeight ? `${el.videoHeight}p` : "";
    for (const m of direct) {
      const row = document.createElement("div");
      row.className = "row";
      const meta = document.createElement("div");
      meta.className = "meta";
      meta.textContent = [extOf(m.mime, m.url), m.url === src ? quality : "", fmt(m.size)].filter(Boolean).join(" · ");
      const b = document.createElement("button");
      b.textContent = "Download";
      b.onclick = async () => {
        b.disabled = true;
        const r = (await chrome.runtime.sendMessage({ type: "download-media", url: m.url })) as { ok: boolean; error?: { message: string } };
        b.textContent = r.ok ? "Sent to Gale" : "Failed";
        if (!r.ok) note(r.error?.message ?? "Gale did not accept the download.");
      };
      row.append(meta, b);
      panel.append(row);
    }
    if (direct.length === 0) {
      note(
        items.some((m) => m.kind === "stream") || src.startsWith("blob:")
          ? "This video is streamed in pieces (HLS/DASH). Gale downloads direct media files only."
          : "No downloadable media found yet. Start playback, then try again.",
      );
    }
  }
  const r = btn.getBoundingClientRect();
  panel.style.top = `${r.bottom + 6}px`;
  panel.style.left = `${Math.max(8, r.right - 340)}px`;
  panel.style.display = "block";
});

function note(text: string) {
  const p = document.createElement("p");
  p.className = "note";
  p.textContent = text;
  panel.append(p);
}

document.addEventListener("mousedown", (e) => {
  if (!e.composedPath().includes(host)) panel.style.display = "none";
});
