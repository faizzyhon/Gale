import { AnimatePresence, motion } from "framer-motion";
import { useCallback, useEffect, useMemo, useState } from "react";
import { DownloadTable } from "../components/DownloadTable";
import { MediaDialog } from "../components/MediaDialog";
import { AddUrlDialog, BatchDialog, PropertiesDialog, RemoveDialog, ShutdownDialog } from "../components/dialogs";
import { Toasts } from "../components/ui";
import { useDownloadActions } from "../hooks/useDownloadActions";
import { Shell } from "../layouts/Shell";
import { DashboardPage } from "../pages/DashboardPage";
import { GrabberPage } from "../pages/GrabberPage";
import { QueuesPage } from "../pages/QueuesPage";
import { api } from "../services/api";
import { toastError } from "../stores/toast";
import { HistoryPage } from "../pages/HistoryPage";
import { SettingsPage } from "../pages/SettingsPage";
import { AboutPage } from "../pages/AboutPage";
import { useDownloads } from "../stores/downloads";
import type { Download } from "../types";
import { STATUS_NAV, matches, type View } from "./views";

export function App() {
  const { ready, loadError, downloads, categories, init } = useDownloads();
  const [view, setView] = useState<View>({ page: "list", nav: STATUS_NAV[0]! });
  const [query, setQuery] = useState("");
  const [adding, setAdding] = useState(false);
  const [addUrl, setAddUrl] = useState<string | null>(null);
  const [batch, setBatch] = useState(false);
  const [props, setProps] = useState<Download | null>(null);
  const [removing, setRemoving] = useState<Download | null>(null);
  const [browserBatch, setBrowserBatch] = useState<string[] | null>(null);
  const [shutdownIn, setShutdownIn] = useState<number | null>(null);
  const [video, setVideo] = useState<{ url: string | null } | null>(null);
  const act = useDownloadActions();

  useEffect(() => {
    // init is async: if we unmount before it resolves (StrictMode), unsubscribe as soon as it does.
    let cleanup: (() => void) | undefined;
    let dead = false;
    init().then((c) => (dead ? c() : (cleanup = c)));
    return () => {
      dead = true;
      cleanup?.();
    };
  }, [init]);

  // Requests from the browser extension and the scheduler.
  useEffect(() => {
    const subs = [
      api.onBrowserBatch((r) => {
        setBrowserBatch(r.urls);
        setBatch(true);
      }),
      api.onBrowserMedia((r) => setVideo({ url: r.url })),
      api.onShutdownCountdown((s) => setShutdownIn(s)),
      api.onShutdownCancelled(() => setShutdownIn(null)),
    ];
    return () => subs.forEach((p) => void p.then((f) => f()));
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
        setAdding(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Paste (Ctrl+V outside a text field) or drop links from the browser: one link opens Add download, several open Add Batch.
  useEffect(() => {
    const links = (text: string) => text.split(/\s+/).filter((t) => /^https?:\/\/\S+$/i.test(t));
    const take = (urls: string[]) => {
      if (urls.length === 1) {
        setAddUrl(urls[0]!);
        setAdding(true);
      } else if (urls.length > 1) {
        setBrowserBatch(urls);
        setBatch(true);
      }
      return urls.length > 0;
    };
    const onPaste = (e: ClipboardEvent) => {
      if ((e.target as HTMLElement).closest?.("input, textarea, select, [contenteditable]")) return;
      if (take(links(e.clipboardData?.getData("text") ?? ""))) e.preventDefault();
    };
    const onDragOver = (e: DragEvent) => {
      if (e.dataTransfer?.types.some((t) => t === "text/uri-list" || t === "text/plain")) e.preventDefault();
    };
    const onDrop = (e: DragEvent) => {
      const d = e.dataTransfer;
      const uriList = (d?.getData("text/uri-list") ?? "").split("\n").filter((l) => !l.startsWith("#")).join(" ");
      if (take(links(uriList || (d?.getData("text/plain") ?? "")))) e.preventDefault();
    };
    window.addEventListener("paste", onPaste);
    window.addEventListener("dragover", onDragOver);
    window.addEventListener("drop", onDrop);
    return () => {
      window.removeEventListener("paste", onPaste);
      window.removeEventListener("dragover", onDragOver);
      window.removeEventListener("drop", onDrop);
    };
  }, []);

  const rows = useMemo(() => {
    if (view.page !== "list") return [];
    return Object.values(downloads)
      .filter((d) => matches(d, view.nav.filter!, categories, query.trim()))
      .sort((a, b) => b.createdAt.localeCompare(a.createdAt) || b.id - a.id);
  }, [downloads, categories, view, query]);

  const onProperties = useCallback((d: Download) => setProps(d), []);
  const onRemove = useCallback((d: Download) => setRemoving(d), []);

  if (!ready) return null;
  if (loadError)
    return (
      <div className="flex h-full items-center justify-center p-8 text-center">
        <div>
          <p className="mb-2 font-display text-[18px] font-semibold">Gale could not open its database</p>
          <p className="text-fg-2 select-text">{loadError}</p>
        </div>
      </div>
    );

  return (
    <>
      <Shell view={view} onNavigate={setView} query={query} onQuery={setQuery} onAdd={() => setAdding(true)} onBatch={() => setBatch(true)} onVideo={() => setVideo({ url: null })}>
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={view.page === "list" ? `list-${view.nav.id}` : view.page}
            className="h-full"
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
          >
            {view.page === "list" && <DownloadTable rows={rows} onProperties={onProperties} onRemove={onRemove} />}
            {view.page === "dashboard" && <DashboardPage />}
            {view.page === "history" && <HistoryPage />}
            {view.page === "settings" && <SettingsPage />}
            {view.page === "about" && <AboutPage />}
            {view.page === "queues" && <QueuesPage />}
            {view.page === "grabber" && <GrabberPage />}
          </motion.div>
        </AnimatePresence>
      </Shell>
      <AddUrlDialog
        open={adding}
        initial={addUrl ? { url: addUrl } : null}
        onVideo={(url) => {
          setAdding(false);
          setVideo({ url });
        }}
        onClose={() => {
          setAdding(false);
          setAddUrl(null);
        }}
      />
      <MediaDialog open={video != null} initialUrl={video?.url} onClose={() => setVideo(null)} />
      <BatchDialog
        open={batch}
        initialUrls={browserBatch}
        onClose={() => {
          setBatch(false);
          setBrowserBatch(null);
        }}
      />
      <ShutdownDialog seconds={shutdownIn} onCancel={() => api.cancelShutdown().then(() => setShutdownIn(null), toastError)} />
      <PropertiesDialog download={props} onClose={() => setProps(null)} />
      <RemoveDialog
        download={removing}
        onClose={() => setRemoving(null)}
        onConfirm={(deleteFile) => {
          if (removing) act.remove(removing, deleteFile);
          setRemoving(null);
        }}
      />
      <Toasts />
    </>
  );
}
