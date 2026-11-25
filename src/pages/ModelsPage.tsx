import { Cpu, Lock, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, inputCls } from "../components/ui";
import { api, errorMessage } from "../services/api";
import { useDownloads } from "../stores/downloads";
import { toastInfo } from "../stores/toast";
import type { ModelInfo, ModelSearchHit } from "../types";
import { bytes } from "../utils/format";

const looksLikeModel = (s: string) => /^(https?:\/\/)?((www\.)?huggingface\.co|hf\.co|hf-mirror\.com)\//.test(s) || /^[\w.-]+\/[\w.-]+$/.test(s.trim());

const count = (n: number) => (n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 1000 ? `${Math.round(n / 1000)}k` : String(n));

/** Hugging Face models: search or paste a model, pick a GGUF size or a weights format, download with SHA-256 checks. */
export function ModelsPage({ onOpenSettings }: { onOpenSettings: () => void }) {
  const { settings } = useDownloads();
  const [input, setInput] = useState("");
  const [hits, setHits] = useState<ModelSearchHit[] | null>(null);
  const [model, setModel] = useState<ModelInfo | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [folders, setFolders] = useState<{ downloads: string; lmStudio: string | null } | null>(null);
  const [toLmStudio, setToLmStudio] = useState(false);

  useEffect(() => {
    api.hfFolders().then((f) => {
      setFolders(f);
      setToLmStudio(f.lmStudio != null);
    }, () => {});
  }, [settings?.downloadDir]);

  const go = async (value = input) => {
    const q = value.trim();
    if (!q) return;
    setBusy(true);
    setError(null);
    try {
      if (looksLikeModel(q)) {
        const m = await api.hfList(q);
        setModel(m);
        setPicked(new Set(m.recommended));
        setHits(null);
      } else {
        setHits(await api.hfSearch(q));
        setModel(null);
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const open = (id: string) => {
    setInput(id);
    void go(id);
  };

  // Picking a GGUF size or weights format replaces the other choice of the same kind.
  const choose = (name: string) => {
    if (!model) return;
    const g = model.groups.find((x) => x.name === name)!;
    const n = new Set(picked);
    const on = g.files.every((f) => n.has(f));
    if (g.kind !== "extra" && !on) {
      for (const other of model.groups.filter((x) => x.kind === g.kind)) other.files.forEach((f) => n.delete(f));
    }
    g.files.forEach((f) => (on ? n.delete(f) : n.add(f)));
    setPicked(n);
  };

  const total = useMemo(() => model?.files.filter((f) => picked.has(f.path)).reduce((a, f) => a + f.size, 0) ?? 0, [model, picked]);
  const folder = toLmStudio && folders?.lmStudio ? folders.lmStudio : folders?.downloads;

  const download = async () => {
    if (!model || !folder) return;
    try {
      const r = await api.hfDownload(model.repo, model.revision, [...picked], folder);
      toastInfo(
        r.added === 0 && r.skipped > 0
          ? "You already have these files"
          : `Downloading ${r.added} file${r.added === 1 ? "" : "s"}${r.skipped ? ` (${r.skipped} already there)` : ""}`,
      );
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  const gguf = model?.groups.filter((g) => g.kind === "gguf") ?? [];
  const weights = model?.groups.filter((g) => g.kind === "weights") ?? [];

  return (
    <div className="flex h-full flex-col">
      <div className="shrink-0 px-8 pt-6 pb-4">
        <h1 className="mb-1 font-display text-[28px] font-semibold">AI Models</h1>
        <p className="mb-4 text-[13px] text-fg-2">Download models from Hugging Face. Each large file is checked against its SHA-256 when it finishes.</p>
        <form
          className="grid max-w-[900px] grid-cols-[1fr_auto] gap-3"
          onSubmit={(e) => {
            e.preventDefault();
            void go();
          }}
        >
          <input
            className={inputCls}
            placeholder="Search, or paste a model: Qwen/Qwen3-8B-GGUF"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            spellCheck={false}
            aria-label="Model name, link or search"
          />
          <Button variant="accent" type="submit" disabled={!input.trim() || busy}>
            <Search size={15} /> {looksLikeModel(input) ? "Show files" : "Search"}
          </Button>
        </form>
        {!settings?.hasHfToken && (
          <p className="mt-2 text-[12px] text-fg-3">
            Gated models (Llama, Gemma and others) need your access token:{" "}
            <button className="text-accent underline" onClick={onOpenSettings}>
              add it in Settings
            </button>
            .
          </p>
        )}
        {error && (
          <p role="alert" className="mt-2 text-[12px] text-bad">
            {error}
          </p>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-auto border-t border-line px-8 pt-4">
        {busy && <p className="text-[13px] text-fg-2">Asking Hugging Face…</p>}
        {!busy && !hits && !model && (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-fg-3">
            <Cpu size={28} strokeWidth={1.25} />
            <p className="text-[12px]">Search for a model, or paste its name or link.</p>
          </div>
        )}

        {!busy && hits && (
          <ul className="max-w-[900px] text-[13px]">
            {hits.length === 0 && <li className="text-fg-2">No models found.</li>}
            {hits.map((h) => (
              <li key={h.id}>
                <button className="flex w-full items-center gap-3 border-b border-line py-2 text-left hover:bg-hover" onClick={() => open(h.id)}>
                  <span className="flex-1 truncate font-medium">{h.id}</span>
                  {h.gated && <Lock size={13} className="text-fg-3" aria-label="Gated" />}
                  <span className="w-28 text-right text-[12px] text-fg-3">{count(h.downloads)} downloads</span>
                </button>
              </li>
            ))}
          </ul>
        )}

        {!busy && model && (
          <div className="max-w-[900px]">
            <h2 className="mb-3 text-[15px] font-semibold">{model.repo}</h2>

            {gguf.length > 0 && (
              <section className="mb-4">
                <h3 className="mb-2 text-[12px] text-fg-2">GGUF version (one is enough; Q4_K_M is a good balance of size and quality)</h3>
                <div className="flex flex-wrap gap-2">
                  {gguf.map((g) => {
                    const on = g.files.every((f) => picked.has(f));
                    return (
                      <button
                        key={g.name}
                        onClick={() => choose(g.name)}
                        aria-pressed={on}
                        className={`rounded border px-3 py-1.5 text-left text-[12px] ${on ? "border-accent bg-accent text-accent-fg" : "border-line hover:bg-hover"}`}
                      >
                        <span className="font-medium">{g.name}</span>
                        <span className="ml-2 opacity-75">{bytes(g.size)}</span>
                      </button>
                    );
                  })}
                </div>
              </section>
            )}

            {weights.length > 0 && (
              <section className="mb-4">
                <h3 className="mb-2 text-[12px] text-fg-2">Weights</h3>
                <div className="flex flex-wrap gap-2">
                  {weights.map((g) => {
                    const on = g.files.every((f) => picked.has(f));
                    return (
                      <button
                        key={g.name}
                        onClick={() => choose(g.name)}
                        aria-pressed={on}
                        className={`rounded border px-3 py-1.5 text-[12px] ${on ? "border-accent bg-accent text-accent-fg" : "border-line hover:bg-hover"}`}
                      >
                        {g.name === "pytorch" ? "PyTorch .bin" : "safetensors"} · {bytes(g.size)}
                      </button>
                    );
                  })}
                </div>
              </section>
            )}

            <h3 className="mb-1 text-[12px] text-fg-2">Files</h3>
            <ul className="mb-4 text-[13px]">
              {model.files.map((f) => (
                <li key={f.path} className="flex items-center gap-3 border-b border-line py-1.5">
                  <input
                    type="checkbox"
                    aria-label={f.path}
                    checked={picked.has(f.path)}
                    onChange={(e) => {
                      const n = new Set(picked);
                      if (e.target.checked) n.add(f.path);
                      else n.delete(f.path);
                      setPicked(n);
                    }}
                  />
                  <span className="flex-1 truncate select-text">{f.path}</span>
                  <span className="w-20 text-right text-[12px] text-fg-3">{bytes(f.size)}</span>
                </li>
              ))}
            </ul>

            <div className="sticky bottom-0 flex flex-wrap items-center gap-3 border-t border-line bg-app pt-3 pb-4 text-[12px]">
              {folders?.lmStudio && (
                <label className="flex items-center gap-2">
                  <input type="checkbox" checked={toLmStudio} onChange={(e) => setToLmStudio(e.target.checked)} /> Save to LM Studio's models folder
                </label>
              )}
              <span className="min-w-0 flex-1 truncate text-fg-3" title={folder}>
                {folder}
              </span>
              <span className="text-fg-2">
                {picked.size} {picked.size === 1 ? "file" : "files"} · {bytes(total)}
              </span>
              <Button variant="accent" disabled={picked.size === 0} onClick={download}>
                Download
              </Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
