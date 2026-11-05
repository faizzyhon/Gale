import { getVersion } from "@tauri-apps/api/app";
import { ExternalLink } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../services/api";
import { toastError } from "../stores/toast";

type LinkName = Parameters<typeof api.openLink>[0];

function Link({ name, children }: { name: LinkName; children: string }) {
  return (
    <button onClick={() => api.openLink(name).catch(toastError)} className="inline-flex items-center gap-1 text-accent hover:underline">
      {children}
      <ExternalLink size={12} aria-hidden />
    </button>
  );
}

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="rounded-[var(--radius-surface)] border border-line bg-surface p-5">
      <h2 className="mb-3 text-[14px] font-semibold">{title}</h2>
      {children}
    </section>
  );
}

export function AboutPage() {
  const [version, setVersion] = useState("");
  useEffect(() => void getVersion().then(setVersion, () => {}), []);

  return (
    <div className="h-full overflow-auto">
      <div className="mx-auto flex max-w-[760px] flex-col gap-4 px-8 py-6">
        <div className="flex items-center gap-4">
          <div className="flex h-14 w-14 items-center justify-center rounded-xl bg-accent/12 text-accent">
            <svg width="30" height="30" viewBox="0 0 24 24" aria-hidden>
              <path fill="currentColor" d="M3 7h12a3 3 0 1 0-3-3h2a1 1 0 1 1 1 1H3zm0 5h16a3 3 0 1 1-3 3h2a1 1 0 1 0 1-1H3zm0 5h8v2H3z" />
            </svg>
          </div>
          <div>
            <h1 className="font-display text-[28px] leading-tight font-semibold">Gale Download Manager</h1>
            <p className="text-fg-2">Version {version || "…"} · Free and open source (MIT) · Windows 10 and 11</p>
          </div>
        </div>

        <Card title="About Gale">
          <p className="leading-relaxed text-fg-2">
            Gale is a free download manager for Windows. It splits each download into several connections to use your full bandwidth, resumes interrupted
            downloads where they stopped, and checks every finished file before marking it complete. It catches downloads from Chrome and Edge, saves
            videos from YouTube and other sites in the quality you choose, and organises everything into categories, queues and schedules. There are no ads,
            no accounts and no tracking.
          </p>
        </Card>

        <Card title="Developer">
          <p className="font-medium">Muhammad Faizan</p>
          <p className="mb-3 text-fg-2">Full-stack developer · Madinah, Saudi Arabia</p>
          <div className="flex flex-wrap gap-x-5 gap-y-2 text-[13px]">
            <Link name="website">faizzyhon.com</Link>
            <Link name="github">GitHub</Link>
            <Link name="linkedin">LinkedIn</Link>
          </div>
        </Card>

        <Card title="Project">
          <div className="flex flex-wrap gap-x-5 gap-y-2 text-[13px]">
            <Link name="repo">Source code</Link>
            <Link name="releases">Releases</Link>
            <Link name="issues">Report a problem</Link>
          </div>
        </Card>

        <Card title="Legal">
          <p className="mb-3 text-fg-2">
            Gale keeps your download list, history and settings on this PC only and sends nothing to the developer. Download only content you have the right to
            download.
          </p>
          <div className="flex flex-wrap gap-x-5 gap-y-2 text-[13px]">
            <Link name="privacy">Privacy policy</Link>
            <Link name="terms">Terms of use</Link>
            <Link name="license">MIT License</Link>
            <Link name="notices">Third-party notices</Link>
          </div>
          <p className="mt-4 text-[12px] text-fg-3">© 2026 Muhammad Faizan. Gale includes yt-dlp (Unlicense), FFmpeg (LGPL) and librqbit (Apache 2.0).</p>
        </Card>
      </div>
    </div>
  );
}
