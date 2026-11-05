// Small Fluent-style primitives shared by every page.
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import { useEffect, type ButtonHTMLAttributes, type ReactNode } from "react";
import { useToasts } from "../stores/toast";

type BtnProps = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "accent" | "standard" | "subtle" | "danger" };

export function Button({ variant = "standard", className = "", ...p }: BtnProps) {
  const v = {
    accent: "bg-accent text-accent-fg hover:opacity-90 border-transparent",
    standard: "bg-surface text-fg border-line hover:bg-hover",
    subtle: "bg-transparent text-fg border-transparent hover:bg-hover",
    danger: "bg-bad text-white border-transparent hover:opacity-90",
  }[variant];
  return (
    <button
      {...p}
      className={`inline-flex h-8 items-center justify-center gap-2 rounded-[var(--radius-ctl)] border px-3 text-[13px] transition-colors disabled:pointer-events-none disabled:opacity-40 ${v} ${className}`}
    />
  );
}

export function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-[12px] text-fg-2">{label}</span>
      {children}
      {hint && <span className="text-[11px] text-fg-3">{hint}</span>}
    </label>
  );
}

export const inputCls =
  "h-8 w-full rounded-[var(--radius-ctl)] border border-line border-b-fg-3 bg-surface px-2.5 text-[13px] text-fg outline-none focus:border-b-2 focus:border-b-accent";

export function Dialog({
  open,
  title,
  onClose,
  children,
  footer,
  width = 520,
}: {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 z-40 flex items-center justify-center bg-black/30"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12 }}
          onMouseDown={(e) => e.target === e.currentTarget && onClose()}
        >
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-label={title}
            style={{ width }}
            className="max-h-[85vh] overflow-hidden rounded-[var(--radius-surface)] border border-line bg-app shadow-2xl"
            initial={{ scale: 0.97, y: 6 }}
            animate={{ scale: 1, y: 0 }}
            exit={{ scale: 0.97, y: 6 }}
            transition={{ type: "spring", stiffness: 500, damping: 36 }}
          >
            <div className="flex items-center justify-between px-6 pt-5 pb-3">
              <h2 className="font-display text-[20px] font-semibold">{title}</h2>
              <button aria-label="Close" onClick={onClose} className="rounded p-1 text-fg-2 hover:bg-hover">
                <X size={16} />
              </button>
            </div>
            <div className="max-h-[60vh] overflow-auto px-6 pb-5">{children}</div>
            {footer && <div className="flex justify-end gap-2 border-t border-line bg-pane px-6 py-4">{footer}</div>}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export function Toasts() {
  const { toasts, dismiss } = useToasts();
  return (
    <div className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-[360px] flex-col gap-2">
      <AnimatePresence>
        {toasts.map((t) => (
          <motion.div
            key={t.id}
            layout
            initial={{ opacity: 0, x: 24 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 24 }}
            role={t.kind === "error" ? "alert" : "status"}
            className={`pointer-events-auto flex items-start gap-3 rounded-[var(--radius-surface)] border border-line bg-surface p-3 shadow-lg ${
              t.kind === "error" ? "border-l-4 border-l-bad" : "border-l-4 border-l-accent"
            }`}
          >
            <p className="flex-1 text-[13px] leading-snug">{t.text}</p>
            <button aria-label="Dismiss" onClick={() => dismiss(t.id)} className="text-fg-3 hover:text-fg">
              <X size={14} />
            </button>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
