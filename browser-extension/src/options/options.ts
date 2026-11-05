import type { InterceptMode } from "../shared/rules";
import { loadSettings, saveSettings } from "../shared/settings";

const form = document.getElementById("form") as HTMLFormElement;
const el = <T extends Element>(name: string) => form.elements.namedItem(name) as unknown as T;

async function fill() {
  const s = await loadSettings();
  el<HTMLInputElement>("enabled").checked = s.enabled;
  el<HTMLSelectElement>("mode").value = s.mode;
  el<HTMLTextAreaElement>("fileTypes").value = s.fileTypes.join(" ");
  el<HTMLInputElement>("minSizeMB").value = String(s.minSizeMB);
  el<HTMLTextAreaElement>("excludedDomains").value = s.excludedDomains.join("\n");
  el<HTMLInputElement>("askFirst").checked = s.askFirst;
  el<HTMLInputElement>("sendCookies").checked = s.sendCookies;
  el<HTMLInputElement>("showMediaButton").checked = s.showMediaButton;
  el<HTMLInputElement>("catchTorrents").checked = s.catchTorrents;
}

form.onsubmit = async (e) => {
  e.preventDefault();
  const words = (v: string) => v.split(/[\s,]+/).map((x) => x.trim().toLowerCase().replace(/^\./, "")).filter(Boolean);
  await saveSettings({
    enabled: el<HTMLInputElement>("enabled").checked,
    mode: el<HTMLSelectElement>("mode").value as InterceptMode,
    fileTypes: [...new Set(words(el<HTMLTextAreaElement>("fileTypes").value))],
    minSizeMB: Math.max(0, Number(el<HTMLInputElement>("minSizeMB").value) || 0),
    excludedDomains: [...new Set(words(el<HTMLTextAreaElement>("excludedDomains").value))],
    askFirst: el<HTMLInputElement>("askFirst").checked,
    sendCookies: el<HTMLInputElement>("sendCookies").checked,
    showMediaButton: el<HTMLInputElement>("showMediaButton").checked,
    catchTorrents: el<HTMLInputElement>("catchTorrents").checked,
  });
  const saved = document.getElementById("saved")!;
  saved.textContent = "Saved";
  setTimeout(() => (saved.textContent = ""), 2000);
};

void fill();
