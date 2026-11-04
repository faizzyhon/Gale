import { HOST, type NativeResponse } from "../shared/native";
import { loadSettings, saveSettings } from "../shared/settings";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const send = <T>(msg: object) => chrome.runtime.sendMessage(msg) as Promise<NativeResponse<T>>;

interface Recent {
  id: number;
  fileName: string;
  status: string;
  progress: number | null;
}

async function render() {
  const s = await loadSettings();
  $<HTMLInputElement>("enabled").checked = s.enabled;
  const paused = s.pausedUntil > Date.now();
  $("note").textContent = paused
    ? `Paused until ${new Date(s.pausedUntil).toLocaleTimeString()}`
    : s.bypassNext
      ? "The next download will go to Chrome."
      : "";
  $<HTMLButtonElement>("pause").textContent = paused ? "Resume now" : "Pause 10 minutes";

  const ping = await send<{ version: string }>({ type: "status" });
  $("dot").classList.toggle("on", ping.ok);
  $("status").textContent = ping.ok
    ? `Connected · v${ping.result?.version ?? "?"}`
    : ping.error?.code === "host_unavailable"
      ? "Gale is not installed"
      : "Gale is not running";

  const ul = $("recent");
  const r = ping.ok ? await send<Recent[]>({ type: "recent" }) : null;
  ul.replaceChildren();
  for (const d of r?.result ?? []) {
    const li = document.createElement("li");
    const name = document.createElement("span");
    name.textContent = d.fileName;
    name.title = d.fileName;
    const st = document.createElement("span");
    st.className = "muted";
    st.textContent = d.status === "downloading" && d.progress != null ? `${Math.floor(d.progress * 100)}%` : d.status;
    li.append(name, st);
    ul.append(li);
  }
  if (!ul.children.length) ul.innerHTML = `<li class="muted"><span>${ping.ok ? "No downloads yet" : "—"}</span></li>`;
}

$<HTMLInputElement>("enabled").onchange = async (e) => {
  await saveSettings({ enabled: (e.target as HTMLInputElement).checked });
  void render();
};
$("pause").onclick = async () => {
  const s = await loadSettings();
  await saveSettings({ pausedUntil: s.pausedUntil > Date.now() ? 0 : Date.now() + 10 * 60_000 });
  void render();
};
$("bypass").onclick = async () => {
  await saveSettings({ bypassNext: true });
  void render();
};
$("open").onclick = async () => {
  await send({ type: "show-app" });
  window.close();
};
$("options").onclick = () => chrome.runtime.openOptionsPage();
$("diag").onclick = async () => {
  const { lastError } = await chrome.storage.session.get("lastError");
  const { appSeenAt } = await chrome.storage.local.get("appSeenAt");
  const ping = await send<{ version: string }>({ type: "status" });
  const out = $("diagOut");
  out.hidden = false;
  out.textContent = [
    `Extension ${chrome.runtime.getManifest().version} (${chrome.runtime.id})`,
    `Native host: ${HOST}`,
    `Ping: ${ping.ok ? "ok" : `${ping.error?.code} — ${ping.error?.message}`}`,
    `Last contact: ${appSeenAt ? new Date(appSeenAt as number).toLocaleString() : "never"}`,
    `Last error: ${lastError ? `${(lastError as { text: string }).text}` : "none"}`,
  ].join("\n");
};

void render();
