import { create } from "zustand";

export interface Toast {
  id: number;
  kind: "info" | "error";
  text: string;
}

let next = 1;

export const useToasts = create<{ toasts: Toast[]; push: (kind: Toast["kind"], text: string) => void; dismiss: (id: number) => void }>(
  (set) => ({
    toasts: [],
    push: (kind, text) => {
      const id = next++;
      set((s) => ({ toasts: [...s.toasts.slice(-3), { id, kind, text }] }));
      setTimeout(() => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })), kind === "error" ? 7000 : 3500);
    },
    dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  }),
);

export const toastError = (e: unknown) =>
  useToasts.getState().push("error", typeof e === "object" && e && "message" in e ? String((e as { message: unknown }).message) : String(e));
export const toastInfo = (text: string) => useToasts.getState().push("info", text);
