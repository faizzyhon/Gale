import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, X } from "lucide-react";
import type { ReactNode } from "react";

/** Title bar for the frameless pop-up windows: drag anywhere on it, minimize, close. */
export function TitleBar({ title, children }: { title: string; children?: ReactNode }) {
  const w = getCurrentWindow();
  return (
    <div data-tauri-drag-region className="flex h-9 shrink-0 items-center gap-2 pl-3 select-none">
      <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden className="pointer-events-none text-accent">
        <path fill="currentColor" d="M3 7h12a3 3 0 1 0-3-3h2a1 1 0 1 1 1 1H3zm0 5h16a3 3 0 1 1-3 3h2a1 1 0 1 0 1-1H3zm0 5h8v2H3z" />
      </svg>
      <span data-tauri-drag-region className="flex-1 truncate text-[12px] text-fg-2">
        {title}
      </span>
      {children}
      <button aria-label="Minimize" onClick={() => w.minimize()} className="flex h-9 w-11 items-center justify-center text-fg-2 hover:bg-hover">
        <Minus size={14} />
      </button>
      <button aria-label="Close" onClick={() => w.close()} className="flex h-9 w-11 items-center justify-center text-fg-2 hover:bg-[#c42b1c] hover:text-white">
        <X size={14} />
      </button>
    </div>
  );
}
