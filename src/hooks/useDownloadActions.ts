import { api } from "../services/api";
import { useDownloads } from "../stores/downloads";
import { toastError, toastInfo } from "../stores/toast";
import type { Download } from "../types";

const run = (p: Promise<unknown>) => p.catch(toastError);

export function canPause(d: Download) {
  return ["queued", "probing", "downloading"].includes(d.status);
}
export function canResume(d: Download) {
  return ["paused", "failed", "cancelled", "scheduled"].includes(d.status);
}

export function useDownloadActions() {
  const drop = useDownloads((s) => s.drop);
  return {
    pause: (d: Download) => run(api.pause(d.id)),
    resume: (d: Download) => run(api.resume(d.id)),
    restart: (d: Download) => run(api.restart(d.id)),
    cancel: (d: Download) => run(api.cancel(d.id)),
    open: (d: Download) => run(api.open(d.id)),
    reveal: (d: Download) => run(api.reveal(d.id)),
    copyUrl: (d: Download) =>
      run(navigator.clipboard.writeText(d.url).then(() => toastInfo("Download URL copied"))),
    remove: (d: Download, deleteFile: boolean) =>
      run(api.remove(d.id, deleteFile).then(() => drop(d.id))),
    recheck: (d: Download) =>
      run(
        api.recheck(d.id).then((p) =>
          toastInfo(
            `Available: ${p.fileName}${p.total != null ? ` · ${(p.total / 1048576).toFixed(1)} MB` : ""} · ${
              p.ranges ? "resumable, multi-connection" : "single connection, not resumable"
            }`,
          ),
        ),
      ),
  };
}
