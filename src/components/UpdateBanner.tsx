import { useEffect, useState } from "react";
import { api, errorMessage } from "../services/api";
import type { UpdateInfo } from "../types";
import { Button } from "./ui";

/** Shown at the top of the window when GitHub has a newer Gale (checked once at start, or from About). */
export function UpdateBanner() {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const un = api.onUpdate(setInfo);
    return () => void un.then((f) => f());
  }, []);
  if (!info) return null;
  return (
    <div className="flex items-center gap-3 border-b border-line bg-accent/10 px-4 py-2 text-[13px]">
      <span className="flex-1 text-fg">
        Gale {info.latest} is available (you have {info.current}).
        {info.canInstall ? " Updating replaces this version and keeps your downloads and settings." : ""}
        {error && <span className="ml-2 text-bad">{error}</span>}
      </span>
      <Button onClick={() => void api.openUrl(info.page)}>What's new</Button>
      {info.canInstall ? (
        <Button
          variant="accent"
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            setError(null);
            try {
              await api.installUpdate(info.installer!);
            } catch (e) {
              setError(errorMessage(e));
              setBusy(false);
            }
          }}
        >
          {busy ? "Downloading…" : "Update now"}
        </Button>
      ) : (
        <Button variant="accent" onClick={() => void api.openUrl(info.installer ?? info.page)}>
          Download
        </Button>
      )}
      <button className="px-1 text-fg-3 hover:text-fg" aria-label="Later" onClick={() => setInfo(null)}>
        ×
      </button>
    </div>
  );
}

/** "Check for updates" on the About page: same banner, or a short answer. */
export function useUpdateCheck() {
  const [state, setState] = useState<string | null>(null);
  const check = async () => {
    setState("Checking…");
    try {
      const u = await api.checkUpdate();
      if (u.newer) {
        api.emitUpdate(u);
        setState(`Gale ${u.latest} is available.`);
      } else setState(`You have the newest version (${u.current}).`);
    } catch (e) {
      setState(errorMessage(e));
    }
  };
  return { state, check };
}
