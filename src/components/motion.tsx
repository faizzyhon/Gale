// Animated building blocks shared by the main window and the progress windows.
import { animate, motion, useMotionValue, useReducedMotion, useSpring, useTransform } from "framer-motion";
import { useEffect, useMemo, useState } from "react";
import type { SegmentState } from "../types";

/** Progress bar that glides to new values; sheen while active, sweeping bar when size is unknown. */
export function ProgressBar({ value, status, height = 4 }: { value: number | null; status: string; height?: number }) {
  const active = status === "downloading" || status === "probing" || status === "queued";
  const color = status === "failed" ? "bg-bad" : status === "paused" ? "bg-warn" : status === "completed" ? "bg-ok" : "bg-accent";
  const spring = useSpring(0, { stiffness: 120, damping: 24 });
  const width = useTransform(spring, (v) => `${Math.min(100, Math.max(0, v * 100))}%`);
  useEffect(() => spring.set(value ?? 0), [value, spring]);

  return (
    <div className="relative w-full overflow-hidden rounded-full bg-line" style={{ height }}>
      {value == null ? (
        active ? <div className={`gale-indeterminate absolute inset-y-0 w-1/3 rounded-full ${color}`} /> : null
      ) : (
        <motion.div className={`relative h-full overflow-hidden rounded-full ${color} ${status === "downloading" ? "gale-sheen" : ""}`} style={{ width }} />
      )}
    </div>
  );
}

/** Number that counts smoothly to its new value (used for %, speed). */
export function AnimatedNumber({ value, format }: { value: number; format: (v: number) => string }) {
  const reduce = useReducedMotion();
  const mv = useMotionValue(value);
  const [text, setText] = useState(format(value));
  useEffect(() => {
    if (reduce) {
      setText(format(value));
      return;
    }
    const c = animate(mv, value, { duration: 0.45, ease: "easeOut", onUpdate: (v) => setText(format(v)) });
    return () => c.stop();
  }, [value, reduce]); // format is a pure formatter; re-running for it would restart the tween
  return <span className="tabular">{text}</span>;
}

/** Live speed history as a filled curve. */
export function SpeedGraph({ samples, height = 70 }: { samples: number[]; height?: number }) {
  const W = 480;
  const path = useMemo(() => {
    const n = 60;
    const data = [...Array(Math.max(0, n - samples.length)).fill(0), ...samples.slice(-n)];
    const max = Math.max(1, ...data) * 1.15;
    const pts = data.map((v, i) => [(i / (n - 1)) * W, height - (v / max) * height] as const);
    // Smooth curve through the points (Catmull-Rom → Bézier).
    let d = `M ${pts[0]![0]} ${pts[0]![1]}`;
    for (let i = 1; i < pts.length; i++) {
      const [x0, y0] = pts[i - 1]!;
      const [x1, y1] = pts[i]!;
      const cx = (x0 + x1) / 2;
      d += ` C ${cx} ${y0}, ${cx} ${y1}, ${x1} ${y1}`;
    }
    return { line: d, area: `${d} L ${W} ${height} L 0 ${height} Z` };
  }, [samples, height]);
  return (
    <svg viewBox={`0 0 ${W} ${height}`} preserveAspectRatio="none" className="block w-full" style={{ height }} role="img" aria-label="Download speed over the last minute">
      <defs>
        <linearGradient id="gale-speed-fill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.35" />
          <stop offset="100%" stopColor="var(--accent)" stopOpacity="0" />
        </linearGradient>
      </defs>
      <motion.path d={path.area} fill="url(#gale-speed-fill)" animate={{ d: path.area }} transition={{ duration: 0.4 }} />
      <motion.path d={path.line} fill="none" stroke="var(--accent)" strokeWidth="1.75" vectorEffect="non-scaling-stroke" animate={{ d: path.line }} transition={{ duration: 0.4 }} />
    </svg>
  );
}

/** One bar per connection: where its range sits in the file and how much of it is done. */
export function SegmentBars({ total, segments }: { total: number; segments: SegmentState[] }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="relative h-2.5 w-full overflow-hidden rounded-[3px] bg-line" aria-label="File map">
        {segments.map((s) => (
          <motion.div
            key={s.start}
            className="absolute top-0 h-full bg-accent"
            style={{ left: `${(s.start / total) * 100}%` }}
            animate={{ width: `${(s.written / total) * 100}%` }}
            transition={{ type: "spring", stiffness: 140, damping: 26 }}
          />
        ))}
      </div>
      <div className="grid grid-cols-2 gap-x-4 gap-y-1">
        {segments.slice(0, 16).map((s, i) => {
          const len = (s.end ?? total) - s.start;
          const frac = len > 0 ? s.written / len : 1;
          return (
            <div key={s.start} className="flex items-center gap-2 text-[11px]">
              <span className="tabular w-5 text-right text-fg-3">{i + 1}</span>
              <div className="h-1 flex-1 overflow-hidden rounded-full bg-line">
                <motion.div className={`h-full ${frac >= 1 ? "bg-ok" : "bg-accent"}`} animate={{ width: `${Math.min(100, frac * 100)}%` }} transition={{ type: "spring", stiffness: 140, damping: 26 }} />
              </div>
              <span className="tabular w-9 text-right text-fg-3">{Math.floor(frac * 100)}%</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/** Check mark that draws itself. */
export function DrawnCheck({ size = 56 }: { size?: number }) {
  return (
    <motion.svg width={size} height={size} viewBox="0 0 56 56" initial={{ scale: 0.6, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ type: "spring", stiffness: 260, damping: 18 }}>
      <circle cx="28" cy="28" r="27" fill="var(--ok)" opacity="0.15" />
      <motion.circle cx="28" cy="28" r="26" fill="none" stroke="var(--ok)" strokeWidth="2.5" initial={{ pathLength: 0 }} animate={{ pathLength: 1 }} transition={{ duration: 0.5, ease: "easeOut" }} />
      <motion.path d="M17 29 l7 7 l15 -16" fill="none" stroke="var(--ok)" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" initial={{ pathLength: 0 }} animate={{ pathLength: 1 }} transition={{ duration: 0.35, delay: 0.35, ease: "easeOut" }} />
    </motion.svg>
  );
}
