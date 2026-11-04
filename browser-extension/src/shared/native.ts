// Talks to the desktop app through the native messaging host (protocol v1).

export const HOST = "com.gale.nmh";

export interface NativeResponse<T = unknown> {
  v: number;
  id: string;
  ok: boolean;
  result?: T;
  error?: { code: string; message: string };
}

export async function native<T = unknown>(cmd: string, payload?: unknown): Promise<NativeResponse<T>> {
  const id = crypto.randomUUID();
  const msg: Record<string, unknown> = { v: 1, id, cmd };
  if (payload !== undefined) msg.payload = payload;
  try {
    const r = (await chrome.runtime.sendNativeMessage(HOST, msg)) as NativeResponse<T>;
    if (!r.ok) await recordError(`${cmd}: ${r.error?.message ?? "failed"}`);
    return r;
  } catch (e) {
    // Host missing (Gale not installed) or crashed.
    const message = e instanceof Error ? e.message : String(e);
    await recordError(`${cmd}: ${message}`);
    return { v: 1, id, ok: false, error: { code: "host_unavailable", message } };
  }
}

async function recordError(text: string) {
  await chrome.storage.session.set({ lastError: { text, at: Date.now() } });
}
