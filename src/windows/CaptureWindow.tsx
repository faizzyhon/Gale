import { getCurrentWindow } from "@tauri-apps/api/window";
import { motion } from "framer-motion";
import { Clock, Download as DownloadIcon, FolderOpen } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { categorize } from "../app/categorize";
import { categoryLabel } from "../app/views";
import { Button, Field, inputCls } from "../components/ui";
import { api, errorMessage } from "../services/api";
import { applyTheme } from "../stores/downloads";
import type { Category, ProbeResult, Queue, SettingsView } from "../types";
import { bytes } from "../utils/format";
import { TitleBar } from "./TitleBar";

const join = (dir: string, sub: string) => `${dir.replace(/[\\/]+$/, "")}${dir.includes("\\") ? "\\" : "/"}${sub}`;

/** IDM-style "Download File Info": confirm a download the browser handed over. */
export function CaptureWindow({ pendingId }: { pendingId: string }) {
  const win = getCurrentWindow();
  const [info, setInfo] = useState<{ url: string; fileName: string | null; referrer: string | null; fileSize: number | null } | null>(null);
  const [probe, setProbe] = useState<ProbeResult | null>(null);
  const [probing, setProbing] = useState(true);
  const [settings, setSettings] = useState<SettingsView | null>(null);
  const [cats, setCats] = useState<Category[]>([]);
  const [queues, setQueues] = useState<Queue[]>([]);
  const [catId, setCatId] = useState<number | null>(null);
  const [dir, setDir] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const touchedName = useRef(false);

  useEffect(() => {
    (async () => {
      try {
        const [i, s, c, q] = await Promise.all([api.getPending(pendingId), api.getSettings(), api.categories(), api.listQueues()]);
        applyTheme(s.theme);
        setInfo(i);
        setSettings(s);
        setCats(c);
        setQueues(q);
        const n = i.fileName || decodeURIComponent(new URL(i.url).pathname.split("/").pop() || "download");
        setName(n);
        const cat = categorize(c, n);
        setCatId(cat?.id ?? null);
        setDir(s.categoryFolders && cat ? join(s.downloadDir, cat.subfolder) : s.downloadDir);
      } catch (e) {
        setError(errorMessage(e));
      }
      await win.show();
      await win.setFocus();
      // Ask the server for size/resume support in the background, with the browser's cookies.
      api.probePending(pendingId).then(
        (p) => {
          setProbe(p);
          if (!touchedName.current) setName(p.fileName);
        },
        () => {},
      ).finally(() => setProbing(false));
    })();
  }, [pendingId]); // runs once per pop-up

  const onCategory = (id: number) => {
    setCatId(id);
    const c = cats.find((x) => x.id === id);
    if (settings?.categoryFolders && c) setDir(join(settings.downloadDir, c.subfolder));
  };

  const submit = async (later: boolean) => {
    if (!info) return;
    setBusy(true);
    setError(null);
    try {
      const main = queues.find((q) => q.isDefault);
      await api.addDownload(
        { url: info.url, saveDir: dir.trim(), fileName: name.trim() || undefined, referrer: info.referrer ?? undefined, start: !later, queueId: later ? main?.id : undefined },
        pendingId,
      );
      await win.close();
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  };

  const size = probe?.total ?? (info?.fileSize && info.fileSize > 0 ? info.fileSize : null);

  return (
    <div className="flex h-full flex-col overflow-hidden border border-line bg-app">
      <TitleBar title="Download File Info" />
      <motion.div className="flex flex-1 flex-col gap-3 px-5 pb-4" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.22, ease: "easeOut" }}>
        <Field label="Address">
          <input className={`${inputCls} text-fg-2`} value={info?.url ?? ""} readOnly spellCheck={false} />
        </Field>
        <div className="grid grid-cols-[180px_1fr] gap-3">
          <Field label="Category">
            <select className={inputCls} value={catId ?? ""} onChange={(e) => onCategory(Number(e.target.value))}>
              {cats.map((c) => (
                <option key={c.id} value={c.id}>
                  {categoryLabel(c.name)}
                </option>
              ))}
            </select>
          </Field>
          <Field label="Save as">
            <input className={inputCls} value={name} onChange={(e) => ((touchedName.current = true), setName(e.target.value))} spellCheck={false} autoFocus />
          </Field>
        </div>
        <Field label="Folder">
          <div className="flex gap-2">
            <input className={inputCls} value={dir} onChange={(e) => setDir(e.target.value)} spellCheck={false} />
            <Button aria-label="Browse for folder" onClick={async () => (await api.pickFolder(dir || undefined).then((p) => p && setDir(p)))}>
              <FolderOpen size={15} />
            </Button>
          </div>
        </Field>
        <p className="text-[12px] text-fg-2">
          {probing && size == null ? (
            <span className="inline-flex items-center gap-2">
              <span className="h-3 w-3 animate-spin rounded-full border-2 border-accent border-t-transparent" /> Checking file…
            </span>
          ) : (
            <>
              Size: <span className="tabular">{size != null ? bytes(size) : "unknown"}</span>
              {info?.url.startsWith("magnet:") ? <> · BitTorrent (the size is known once peers are found)</> : probe && <> · {probe.ranges ? "resumable, multi-connection" : "not resumable (single connection)"}</>}
            </>
          )}
        </p>
        {error && (
          <p role="alert" className="text-[12px] text-bad">
            {error}
          </p>
        )}
        <div className="mt-auto flex items-center gap-2">
          <Button variant="subtle" onClick={() => api.showMain()}>
            Open Gale
          </Button>
          <span className="flex-1" />
          <Button onClick={() => win.close()} disabled={busy}>
            Cancel
          </Button>
          <Button onClick={() => submit(true)} disabled={busy || !info || !dir.trim()} title="Add to the main queue">
            <Clock size={15} /> Download later
          </Button>
          <Button variant="accent" onClick={() => submit(false)} disabled={busy || !info || !dir.trim()}>
            <DownloadIcon size={15} /> Start download
          </Button>
        </div>
      </motion.div>
    </div>
  );
}
