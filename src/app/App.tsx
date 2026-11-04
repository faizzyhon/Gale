import { useCallback, useEffect, useMemo, useState } from "react";
import { DownloadTable } from "../components/DownloadTable";
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
import { useDownloads } from "../stores/downloads";
import type { BrowserAddRequest, Download } from "../types";
import { STATUS_NAV, matches, type View } from "./views";

export function App() {
  const { ready, loadError, downloads, categories, init } = useDownloads();
  const [view, setView] = useState<View>({ page: "list", nav: STATUS_NAV[0]! });
  const [query, setQuery] = useState("");
  const [adding, setAdding] = useState(false);
  const [batch, setBatch] = useState(false);
  const [props, setProps] = useState<Download | null>(null);
  const [removing, setRemoving] = useState<Download | null>(null);
  const [browserAdd, setBrowserAdd] = useState<BrowserAddRequest | null>(null);
  const [browserBatch, setBrowserBatch] = useState<string[] | null>(null);
  const [shutdownIn, setShutdownIn] = useState<number | null>(null);
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
      api.onBrowserAdd((r) => {
        setBrowserAdd(r);
        setAdding(true);
      }),
      api.onBrowserBatch((r) => {
        setBrowserBatch(r.urls);
        setBatch(true);
      }),
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
      <Shell view={view} onNavigate={setView} query={query} onQuery={setQuery} onAdd={() => setAdding(true)} onBatch={() => setBatch(true)}>
        {view.page === "list" && <DownloadTable rows={rows} onProperties={onProperties} onRemove={onRemove} />}
        {view.page === "dashboard" && <DashboardPage />}
        {view.page === "history" && <HistoryPage />}
        {view.page === "settings" && <SettingsPage />}
        {view.page === "queues" && <QueuesPage />}
        {view.page === "grabber" && <GrabberPage />}
      </Shell>
      <AddUrlDialog
        open={adding}
        initial={browserAdd}
        onClose={() => {
          setAdding(false);
          setBrowserAdd(null);
        }}
      />
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
