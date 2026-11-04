import { FolderOpen } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Button, Field, inputCls } from "../components/ui";
import { api, errorMessage } from "../services/api";
import { useDownloads } from "../stores/downloads";
import { toastError, toastInfo } from "../stores/toast";
import type { AppSettings, BrowserStatus, ConnectionTest, SettingsView } from "../types";
import { date } from "../utils/format";

const TABS = ["General", "Downloads", "Browser", "Network", "Security", "Advanced"] as const;
type Tab = (typeof TABS)[number];

const list = (v: string[]) => v.join("\n");
const unlist = (v: string) => v.split(/[\s,]+/).filter(Boolean);

/** Multi-line list editor that keeps what the user typed (blank lines, trailing Enter) while editing. */
function ListField({ label, hint, value, onChange, rows = 4 }: { label: string; hint: string; value: string[]; onChange: (v: string[]) => void; rows?: number }) {
  const [raw, setRaw] = useState(list(value));
  useEffect(() => {
    if (unlist(raw).join() !== value.join()) setRaw(list(value));
  }, [value]); // raw intentionally omitted: only external changes resync
  return (
    <Field label={label} hint={hint}>
      <textarea
        rows={rows}
        className={`${inputCls} h-auto py-2`}
        value={raw}
        onChange={(e) => {
          setRaw(e.target.value);
          onChange(unlist(e.target.value));
        }}
        spellCheck={false}
      />
    </Field>
  );
}

function Check({ checked, onChange, children, hint }: { checked: boolean; onChange: (v: boolean) => void; children: ReactNode; hint?: string }) {
  return (
    <label className="flex items-start gap-2.5 text-[13px]">
      <input type="checkbox" className="mt-0.5" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>
        {children}
        {hint && <span className="block text-[11px] text-fg-3">{hint}</span>}
      </span>
    </label>
  );
}

export function SettingsPage() {
  const saved = useDownloads((s) => s.settings);
  const setSettings = useDownloads((s) => s.setSettings);
  const [draft, setDraft] = useState<SettingsView | null>(saved);
  const [tab, setTab] = useState<Tab>("General");
  const [error, setError] = useState<string | null>(null);
  useEffect(() => setDraft(saved), [saved]);
  if (!draft) return null;

  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const set = <K extends keyof AppSettings>(k: K, v: AppSettings[K]) => setDraft({ ...draft, [k]: v });
  const save = async () => {
    setError(null);
    try {
      const { hasProxyPassword: _, ...settings } = draft;
      setSettings(await api.saveSettings(settings));
      toastInfo("Settings saved");
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  return (
    <div className="flex h-full">
      <nav className="w-48 shrink-0 border-r border-line px-3 pt-6" aria-label="Settings sections">
        <h1 className="mb-4 px-2 font-display text-[22px] font-semibold">Settings</h1>
        {TABS.map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            aria-current={tab === t ? "page" : undefined}
            className={`relative mb-0.5 flex h-8 w-full items-center rounded-[var(--radius-ctl)] px-3 text-left text-[13px] ${tab === t ? "bg-hover font-medium" : "hover:bg-hover"}`}
          >
            {tab === t && <span className="absolute top-2 bottom-2 left-0 w-[3px] rounded-full bg-accent" />}
            {t}
          </button>
        ))}
      </nav>
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="min-h-0 flex-1 overflow-auto px-8 py-6">
          <div className="flex max-w-[640px] flex-col gap-5">
            <h2 className="font-display text-[20px] font-semibold">{tab}</h2>
            {tab === "General" && (
              <>
                <Field label="Theme">
                  <select className={inputCls} value={draft.theme} onChange={(e) => set("theme", e.target.value as AppSettings["theme"])}>
                    <option value="system">Use system setting</option>
                    <option value="light">Light</option>
                    <option value="dark">Dark</option>
                  </select>
                </Field>
                <Field label="Default download folder">
                  <div className="flex gap-2">
                    <input className={inputCls} value={draft.downloadDir} onChange={(e) => set("downloadDir", e.target.value)} spellCheck={false} />
                    <Button aria-label="Browse for folder" onClick={async () => (await api.pickFolder(draft.downloadDir).then((p) => p && set("downloadDir", p)))}>
                      <FolderOpen size={15} />
                    </Button>
                  </div>
                </Field>
                <Check checked={draft.categoryFolders} onChange={(v) => set("categoryFolders", v)} hint="Video, Music, Documents… inside the default folder">
                  Sort downloads into category folders
                </Check>
                <Check checked={draft.notifications} onChange={(v) => set("notifications", v)}>
                  Show Windows notifications when downloads finish or fail
                </Check>
                <Check checked={draft.startWithWindows} onChange={(v) => set("startWithWindows", v)} hint="Starts in the tray so scheduled downloads and the browser extension work">
                  Start Gale when I sign in to Windows
                </Check>
                <Check checked={draft.minimizeToTray} onChange={(v) => set("minimizeToTray", v)} hint="Downloads keep running; quit from the tray icon">
                  Closing the window keeps Gale running in the tray
                </Check>
              </>
            )}
            {tab === "Downloads" && (
              <>
                <Field label="Connections per download" hint="Used for new downloads. Servers without range support always use one.">
                  <select className={inputCls} value={draft.connections} onChange={(e) => set("connections", Number(e.target.value) as AppSettings["connections"])}>
                    {[1, 2, 4, 8, 16, 32].map((c) => (
                      <option key={c}>{c}</option>
                    ))}
                  </select>
                </Field>
                <Field label="Simultaneous downloads" hint="Takes effect after Gale restarts.">
                  <input type="number" min={1} max={32} className={inputCls} value={draft.maxSimultaneous} onChange={(e) => set("maxSimultaneous", Number(e.target.value) || 1)} />
                </Field>
                <Field label="Automatic retries per connection" hint="With increasing waits between attempts (0–20).">
                  <input type="number" min={0} max={20} className={inputCls} value={draft.maxRetries} onChange={(e) => set("maxRetries", Number(e.target.value) || 0)} />
                </Field>
                <p className="text-[12px] text-fg-3">Partial data is kept next to each file as “name.gale-part” until it completes, so resumes survive restarts.</p>
              </>
            )}
            {tab === "Browser" && <BrowserTab draft={draft} set={set} />}
            {tab === "Network" && <NetworkTab draft={draft} set={set} onPasswordChange={(has) => setSettings({ ...saved!, hasProxyPassword: has })} />}
            {tab === "Security" && (
              <>
                <Check checked={draft.markOfTheWeb} onChange={(v) => set("markOfTheWeb", v)} hint="Windows SmartScreen and Office Protected View then treat files as coming from the internet">
                  Mark downloaded files as coming from the internet
                </Check>
                <Check checked={draft.defenderScan} onChange={(v) => set("defenderScan", v)} hint="Microsoft Defender quarantines anything it finds">
                  Scan finished downloads with Microsoft Defender
                </Check>
                <Check checked={draft.computeSha256} onChange={(v) => set("computeSha256", v)} hint="Shown in Properties. Downloads with an expected checksum are always verified.">
                  Calculate a SHA-256 checksum for every download
                </Check>
                <ListField label="Blocked file types" hint="Downloads with these extensions are refused. One per line, without dots." value={draft.blockedExtensions} onChange={(v) => set("blockedExtensions", v)} />
                <p className="text-[12px] text-fg-3">Passwords are kept in Windows Credential Manager. Browser cookies are held in memory only and never written to disk or logs. Certificate checks are always on.</p>
              </>
            )}
            {tab === "Advanced" && <AdvancedTab onReset={setSettings} />}
            {error && (
              <p role="alert" className="text-[12px] text-bad">
                {error}
              </p>
            )}
          </div>
        </div>
        {tab !== "Advanced" && (
          <div className="flex gap-2 border-t border-line px-8 py-3">
            <Button variant="accent" disabled={!dirty} onClick={save}>
              Save
            </Button>
            <Button disabled={!dirty} onClick={() => setDraft(saved)}>
              Discard changes
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}

type SetFn = <K extends keyof AppSettings>(k: K, v: AppSettings[K]) => void;

function BrowserTab({ draft, set }: { draft: SettingsView; set: SetFn }) {
  const [status, setStatus] = useState<BrowserStatus | null>(null);
  useEffect(() => void api.browserStatus().then(setStatus, () => {}), []);
  return (
    <>
      <div className="rounded-[var(--radius-surface)] border border-line bg-surface p-4 text-[13px]">
        <p className="font-medium">Chrome / Edge extension</p>
        <p className="mt-1 text-fg-2">
          {status?.lastContact ? `Last download from the browser: ${date(status.lastContact)} · ${status.intercepted} taken over so far` : "No downloads received from the browser yet."}
        </p>
        <p className="mt-2 text-[12px] text-fg-3">
          To connect Chrome or Edge, open “Install browser extension” in the Gale Start menu folder and follow the three steps. Which file types and sizes get taken over is set in the extension's options.
        </p>
      </div>
      <Check checked={draft.browserEnabled} onChange={(v) => set("browserEnabled", v)}>
        Accept downloads from the browser
      </Check>
      <ListField label="Never take over downloads from these sites" hint="One per line. Subdomains are included." value={draft.browserExcludedDomains} onChange={(v) => set("browserExcludedDomains", v)} />
      <ListField label="Never take over these file types" hint="One per line, without dots. The browser keeps these." value={draft.browserExcludedTypes} onChange={(v) => set("browserExcludedTypes", v)} rows={3} />
    </>
  );
}

function NetworkTab({ draft, set, onPasswordChange }: { draft: SettingsView; set: SetFn; onPasswordChange: (has: boolean) => void }) {
  const [pw, setPw] = useState("");
  const [testUrl, setTestUrl] = useState("https://www.microsoft.com/");
  const [test, setTest] = useState<ConnectionTest | null>(null);
  const [testing, setTesting] = useState(false);
  const proxy = draft.proxyKind !== "none";
  return (
    <>
      <Field label="Proxy">
        <select className={inputCls} value={draft.proxyKind} onChange={(e) => set("proxyKind", e.target.value as AppSettings["proxyKind"])}>
          <option value="none">No proxy</option>
          <option value="http">HTTP</option>
          <option value="https">HTTPS</option>
          <option value="socks5">SOCKS5</option>
        </select>
      </Field>
      {proxy && (
        <>
          <div className="grid grid-cols-[1fr_110px] gap-3">
            <Field label="Host">
              <input className={inputCls} value={draft.proxyHost} onChange={(e) => set("proxyHost", e.target.value.trim())} spellCheck={false} />
            </Field>
            <Field label="Port">
              <input type="number" min={1} max={65535} className={inputCls} value={draft.proxyPort} onChange={(e) => set("proxyPort", Number(e.target.value) || 0)} />
            </Field>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <Field label="User name (optional)">
              <input className={inputCls} value={draft.proxyUser} onChange={(e) => set("proxyUser", e.target.value)} autoComplete="off" />
            </Field>
            <Field label={draft.hasProxyPassword ? "Password (saved — type to replace)" : "Password"}>
              <div className="flex gap-2">
                <input type="password" className={inputCls} value={pw} onChange={(e) => setPw(e.target.value)} autoComplete="new-password" />
                <Button
                  disabled={!pw && !draft.hasProxyPassword}
                  onClick={() =>
                    api.setProxyPassword(pw || null).then(() => {
                      onPasswordChange(!!pw);
                      setPw("");
                      toastInfo(pw ? "Password saved to Windows Credential Manager" : "Password removed");
                    }, toastError)
                  }
                >
                  {pw ? "Save" : "Remove"}
                </Button>
              </div>
            </Field>
          </div>
        </>
      )}
      <div className="grid grid-cols-2 gap-3">
        <Field label="Connection timeout (s)">
          <input type="number" min={3} max={300} className={inputCls} value={draft.connectTimeoutSecs} onChange={(e) => set("connectTimeoutSecs", Number(e.target.value) || 15)} />
        </Field>
        <Field label="Read timeout (s)">
          <input type="number" min={5} max={600} className={inputCls} value={draft.readTimeoutSecs} onChange={(e) => set("readTimeoutSecs", Number(e.target.value) || 30)} />
        </Field>
        <Field label="Total speed limit (KB/s, 0 = none)">
          <input type="number" min={0} className={inputCls} value={draft.globalLimitKbps} onChange={(e) => set("globalLimitKbps", Math.max(0, Number(e.target.value) || 0))} />
        </Field>
        <Field label="Per-download limit (KB/s, 0 = none)">
          <input type="number" min={0} className={inputCls} value={draft.perDownloadLimitKbps} onChange={(e) => set("perDownloadLimitKbps", Math.max(0, Number(e.target.value) || 0))} />
        </Field>
        <Field label="Connections per server (0 = no limit)" hint="Across all downloads from the same host.">
          <input type="number" min={0} max={64} className={inputCls} value={draft.perHostConnections} onChange={(e) => set("perHostConnections", Math.max(0, Number(e.target.value) || 0))} />
        </Field>
      </div>
      <Field label="User agent (empty = Gale default)">
        <input className={inputCls} value={draft.userAgent} onChange={(e) => set("userAgent", e.target.value)} spellCheck={false} />
      </Field>
      <div className="rounded-[var(--radius-surface)] border border-line bg-surface p-4">
        <p className="mb-2 text-[13px] font-medium">Connection test</p>
        <p className="mb-3 text-[12px] text-fg-3">Uses your saved settings. Save first if you changed anything above.</p>
        <div className="flex gap-2">
          <input className={inputCls} value={testUrl} onChange={(e) => setTestUrl(e.target.value)} spellCheck={false} aria-label="Test address" />
          <Button
            disabled={testing}
            onClick={() => {
              setTesting(true);
              api.testConnection(testUrl).then(setTest, toastError).finally(() => setTesting(false));
            }}
          >
            {testing ? "Testing…" : "Test"}
          </Button>
        </div>
        {test && (
          <p className={`mt-2 text-[12px] ${test.ok ? "text-ok" : "text-bad"}`}>
            {test.message} · {test.millis} ms{test.ok ? ` · ${test.resumable ? "supports multi-connection" : "single connection only"}` : ""}
          </p>
        )}
      </div>
    </>
  );
}

function AdvancedTab({ onReset }: { onReset: (s: SettingsView) => void }) {
  const [confirmReset, setConfirmReset] = useState(false);
  return (
    <>
      <section className="flex flex-col gap-2">
        <h3 className="text-[14px] font-semibold">Database</h3>
        <p className="text-[12px] text-fg-3">Your download list, history, queues, schedules and settings. Backups are consistent even while downloading.</p>
        <div className="flex gap-2">
          <Button
            onClick={async () => {
              const dest = await api.pickSave(`gale-backup-${new Date().toISOString().slice(0, 10)}.db`);
              if (dest) await api.backupDatabase(dest).then(() => toastInfo("Backup saved"), toastError);
            }}
          >
            Back up…
          </Button>
          <Button
            onClick={async () => {
              const src = await api.pickFile();
              if (src) await api.restoreDatabase(src).then(() => toastInfo("Backup checked. It will be restored the next time Gale starts."), toastError);
            }}
          >
            Restore…
          </Button>
        </div>
      </section>
      <section className="flex flex-col gap-2">
        <h3 className="text-[14px] font-semibold">Logs & diagnostics</h3>
        <p className="text-[12px] text-fg-3">Structured logs, one file per day. They never contain cookies, passwords or proxy credentials.</p>
        <div>
          <Button onClick={() => api.openLogs().catch(toastError)}>Open log folder</Button>
        </div>
      </section>
      <section className="flex flex-col gap-2">
        <h3 className="text-[14px] font-semibold">Updates</h3>
        <p className="text-[12px] text-fg-3">Version {__APP_VERSION__}. Install newer versions with the latest Gale installer; your data is kept.</p>
      </section>
      <section className="flex flex-col gap-2">
        <h3 className="text-[14px] font-semibold">Reset</h3>
        <p className="text-[12px] text-fg-3">Restores every setting to its default. Downloads, history and queues are kept.</p>
        <div className="flex gap-2">
          {confirmReset ? (
            <>
              <Button variant="danger" onClick={() => api.resetSettings().then((s) => (onReset(s), setConfirmReset(false), toastInfo("Settings reset")), toastError)}>
                Yes, reset settings
              </Button>
              <Button onClick={() => setConfirmReset(false)}>Cancel</Button>
            </>
          ) : (
            <Button onClick={() => setConfirmReset(true)}>Reset settings…</Button>
          )}
        </div>
      </section>
    </>
  );
}
