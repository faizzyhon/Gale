import { MotionConfig } from "framer-motion";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import "./styles/index.css";
import { CaptureWindow } from "./windows/CaptureWindow";
import { DownloadWindow } from "./windows/DownloadWindow";

// The native context menu (Back/Reload/Inspect) breaks the desktop illusion; inputs keep theirs.
document.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement;
  if (!t.closest("input, textarea")) e.preventDefault();
});

// One frontend, three kinds of window: routed by the hash Rust opens them with.
const download = location.hash.match(/^#\/download\/(\d+)$/);
const capture = location.hash.match(/^#\/capture\/([\w-]+)$/);
const root = download ? <DownloadWindow id={Number(download[1])} /> : capture ? <CaptureWindow pendingId={capture[1]!} /> : <App />;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {/* Respects Windows' "Animation effects" setting. */}
    <MotionConfig reducedMotion="user">{root}</MotionConfig>
  </StrictMode>,
);
