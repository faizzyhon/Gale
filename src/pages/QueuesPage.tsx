import { CalendarPlus, Pause, Play, Plus, Square, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Dialog, Field, inputCls } from "../components/ui";
import { api, errorMessage } from "../services/api";
import { useDownloads } from "../stores/downloads";
import { toastError } from "../stores/toast";
import type { Queue, SchedulerJob } from "../types";
import { date } from "../utils/format";

const DAYS = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"] as const;

export function QueuesPage() {
  const { queues, memberships, downloads, refreshQueues } = useDownloads();
  const [jobs, setJobs] = useState<SchedulerJob[]>([]);
  const [editing, setEditing] = useState<Queue | "new" | null>(null);
  const [jobEdit, setJobEdit] = useState<SchedulerJob | null>(null);
  const loadJobs = () => api.listJobs().then(setJobs, toastError);
  useEffect(() => void loadJobs(), []);

  const itemsByQueue = useMemo(() => {
    const m: Record<number, number[]> = {};
    for (const [d, q] of Object.entries(memberships)) (m[q] ??= []).push(Number(d));
    return m;
  }, [memberships]);
  const act = (id: number, a: "start" | "stop" | "pause") => api.queueAction(id, a).then(refreshQueues, toastError);
  const qName = (id: number) => queues.find((q) => q.id === id)?.name ?? "—";

  return (
    <div className="h-full overflow-auto px-8 py-6">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="font-display text-[28px] font-semibold">Queues & Scheduler</h1>
        <Button onClick={() => setEditing("new")}>
          <Plus size={15} /> New queue
        </Button>
      </div>

      <div className="mb-10 flex flex-col gap-3">
        {queues.map((q) => {
          const ids = itemsByQueue[q.id] ?? [];
          const done = ids.filter((id) => downloads[id]?.status === "completed").length;
          return (
            <section key={q.id} className="rounded-[var(--radius-surface)] border border-line bg-surface">
              <div className="flex items-center gap-3 px-4 py-3">
                <span className={`h-2 w-2 rounded-full ${q.state === "running" ? "bg-ok" : q.state === "paused" ? "bg-warn" : "bg-fg-3"}`} />
                <div className="min-w-0 flex-1">
                  <p className="font-medium">
                    {q.name} {q.isDefault && <span className="text-[11px] text-fg-3">(main)</span>}
                  </p>
                  <p className="text-[12px] text-fg-2">
                    {q.state} · {done}/{ids.length} done · {q.maxConcurrent} at a time · up to {q.maxConnections} connections each
                  </p>
                </div>
                <Button variant="subtle" aria-label="Start queue" title="Start" disabled={q.state === "running"} onClick={() => act(q.id, "start")}>
                  <Play size={15} />
                </Button>
                <Button variant="subtle" aria-label="Pause queue" title="Pause: finish active, start nothing new" disabled={q.state !== "running"} onClick={() => act(q.id, "pause")}>
                  <Pause size={15} />
                </Button>
                <Button variant="subtle" aria-label="Stop queue" title="Stop: pause active downloads" disabled={q.state === "stopped"} onClick={() => act(q.id, "stop")}>
                  <Square size={15} />
                </Button>
                <Button variant="subtle" onClick={() => setEditing(q)}>
                  Edit
                </Button>
              </div>
              {ids.length > 0 && (
                <ul className="border-t border-line text-[12px]">
                  {ids.slice(0, 50).map((id) => (
                    <li key={id} className="flex items-center gap-3 px-4 py-1.5">
                      <span className="flex-1 truncate">{downloads[id]?.fileName ?? `#${id}`}</span>
                      <span className="text-fg-2">{downloads[id]?.status}</span>
                      <button className="text-fg-3 hover:text-bad" onClick={() => api.setDownloadQueue(id, null).then(refreshQueues, toastError)}>
                        Remove from queue
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </section>
          );
        })}
      </div>

      <div className="mb-3 flex items-center justify-between">
        <h2 className="text-[18px] font-semibold">Schedule</h2>
        <Button
          disabled={queues.length === 0}
          onClick={() =>
            setJobEdit({ id: 0, queueId: queues[0]!.id, kind: "start_queue", runAt: nextHour(), recurrence: null, enabled: true, shutdownAfter: false })
          }
        >
          <CalendarPlus size={15} /> Add schedule
        </Button>
      </div>
      {jobs.length === 0 ? (
        <p className="text-[12px] text-fg-3">Start or stop a queue at a set time, once or on repeat. Gale must be running (it can start with Windows).</p>
      ) : (
        <ul className="divide-y divide-line rounded-[var(--radius-surface)] border border-line bg-surface">
          {jobs.map((j) => (
            <li key={j.id} className="flex items-center gap-4 px-4 py-2.5 text-[13px]">
              <span className={j.enabled ? "" : "text-fg-3 line-through"}>
                {j.kind === "start_queue" ? "Start" : "Stop"} “{qName(j.queueId)}”
              </span>
              <span className="text-fg-2">{date(j.runAt)}</span>
              <span className="text-fg-3">{describeRecurrence(j.recurrence)}</span>
              {j.shutdownAfter && <span className="text-[12px] text-warn">then shut down</span>}
              <span className="flex-1" />
              <Button variant="subtle" onClick={() => setJobEdit(j)}>
                Edit
              </Button>
              <Button variant="subtle" aria-label="Delete schedule" onClick={() => api.deleteJob(j.id).then(loadJobs, toastError)}>
                <Trash2 size={15} />
              </Button>
            </li>
          ))}
        </ul>
      )}

      <QueueDialog queue={editing} onClose={() => setEditing(null)} onSaved={refreshQueues} />
      <JobDialog job={jobEdit} queues={queues} onClose={() => setJobEdit(null)} onSaved={loadJobs} />
    </div>
  );
}

function nextHour() {
  const d = new Date();
  d.setHours(d.getHours() + 1, 0, 0, 0);
  return d.toISOString();
}

function describeRecurrence(r: string | null) {
  if (!r) return "once";
  if (r === "daily") return "every day";
  if (r === "weekdays") return "Mon–Fri";
  return `weekly: ${r.slice(7)}`;
}

/** ISO (UTC) ⇄ value for <input type="datetime-local"> in local time. */
const toLocalInput = (iso: string) => {
  const d = new Date(iso);
  return new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
};
const fromLocalInput = (v: string) => new Date(v).toISOString();

function QueueDialog({ queue, onClose, onSaved }: { queue: Queue | "new" | null; onClose: () => void; onSaved: () => void }) {
  const q = queue === "new" ? null : queue;
  const [form, setForm] = useState({ name: "", priority: 0, maxConcurrent: 3, maxConnections: 8 });
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setError(null);
    setForm(q ? { name: q.name, priority: q.priority, maxConcurrent: q.maxConcurrent, maxConnections: q.maxConnections } : { name: "", priority: 0, maxConcurrent: 3, maxConnections: 8 });
  }, [queue]);
  const save = () => api.saveQueue(q?.id ?? null, form).then(() => (onSaved(), onClose()), (e) => setError(errorMessage(e)));
  const del = () => q && api.deleteQueue(q.id).then(() => (onSaved(), onClose()), (e) => setError(errorMessage(e)));
  return (
    <Dialog
      open={!!queue}
      title={q ? "Edit queue" : "New queue"}
      width={460}
      onClose={onClose}
      footer={
        <>
          {q && !q.isDefault && (
            <Button variant="danger" onClick={del} className="mr-auto">
              Delete
            </Button>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="accent" onClick={save} disabled={!form.name.trim()}>
            Save
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <Field label="Name">
          <input autoFocus className={inputCls} value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
        </Field>
        <div className="grid grid-cols-3 gap-3">
          <Field label="At a time">
            <input type="number" min={1} max={32} className={inputCls} value={form.maxConcurrent} onChange={(e) => setForm({ ...form, maxConcurrent: Number(e.target.value) })} />
          </Field>
          <Field label="Connections each">
            <select className={inputCls} value={form.maxConnections} onChange={(e) => setForm({ ...form, maxConnections: Number(e.target.value) })}>
              {[1, 2, 4, 8, 16, 32].map((c) => (
                <option key={c}>{c}</option>
              ))}
            </select>
          </Field>
          <Field label="Priority">
            <input type="number" className={inputCls} value={form.priority} onChange={(e) => setForm({ ...form, priority: Number(e.target.value) })} />
          </Field>
        </div>
        {error && <p role="alert" className="text-[12px] text-bad">{error}</p>}
      </div>
    </Dialog>
  );
}

function JobDialog({ job, queues, onClose, onSaved }: { job: SchedulerJob | null; queues: Queue[]; onClose: () => void; onSaved: () => void }) {
  const [j, setJ] = useState<SchedulerJob | null>(job);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setJ(job);
    setError(null);
  }, [job]);
  if (!j) return <Dialog open={false} title="" onClose={onClose}>{null}</Dialog>;
  const rec = j.recurrence ?? "";
  const repeat = rec === "" ? "once" : rec.startsWith("weekly:") ? "weekly" : rec;
  const days = rec.startsWith("weekly:") ? rec.slice(7).split(",") : [];
  const setDays = (d: string[]) => setJ({ ...j, recurrence: `weekly:${(d.length ? d : ["MO"]).join(",")}` });
  const save = () => api.saveJob(j).then(() => (onSaved(), onClose()), (e) => setError(errorMessage(e)));

  return (
    <Dialog
      open={!!job}
      title={j.id ? "Edit schedule" : "Add schedule"}
      width={480}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="accent" onClick={save}>
            Save
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        <div className="grid grid-cols-2 gap-3">
          <Field label="Action">
            <select className={inputCls} value={j.kind} onChange={(e) => setJ({ ...j, kind: e.target.value as SchedulerJob["kind"] })}>
              <option value="start_queue">Start queue</option>
              <option value="stop_queue">Stop queue</option>
            </select>
          </Field>
          <Field label="Queue">
            <select className={inputCls} value={j.queueId} onChange={(e) => setJ({ ...j, queueId: Number(e.target.value) })}>
              {queues.map((q) => (
                <option key={q.id} value={q.id}>
                  {q.name}
                </option>
              ))}
            </select>
          </Field>
        </div>
        <Field label="When">
          <input type="datetime-local" className={inputCls} value={toLocalInput(j.runAt)} onChange={(e) => e.target.value && setJ({ ...j, runAt: fromLocalInput(e.target.value) })} />
        </Field>
        <Field label="Repeat">
          <select
            className={inputCls}
            value={repeat}
            onChange={(e) => setJ({ ...j, recurrence: e.target.value === "once" ? null : e.target.value === "weekly" ? "weekly:MO" : e.target.value })}
          >
            <option value="once">Once</option>
            <option value="daily">Every day</option>
            <option value="weekdays">Monday to Friday</option>
            <option value="weekly">On selected days</option>
          </select>
        </Field>
        {repeat === "weekly" && (
          <div className="flex gap-1.5">
            {DAYS.map((d) => (
              <button
                key={d}
                onClick={() => setDays(days.includes(d) ? days.filter((x) => x !== d) : DAYS.filter((x) => x === d || days.includes(x)))}
                className={`h-8 w-10 rounded-[var(--radius-ctl)] border text-[12px] ${days.includes(d) ? "border-transparent bg-accent text-accent-fg" : "border-line bg-surface"}`}
              >
                {d}
              </button>
            ))}
          </div>
        )}
        <label className="flex items-center gap-2 text-[13px]">
          <input type="checkbox" checked={j.enabled} onChange={(e) => setJ({ ...j, enabled: e.target.checked })} /> Enabled
        </label>
        <label className="flex items-start gap-2 text-[13px]">
          <input type="checkbox" className="mt-0.5" checked={j.shutdownAfter} onChange={(e) => setJ({ ...j, shutdownAfter: e.target.checked })} />
          <span>
            Shut down Windows {j.kind === "start_queue" ? "when this queue finishes" : "after stopping"}
            <span className="block text-[11px] text-fg-3">You get a 60-second warning with a Cancel button first.</span>
          </span>
        </label>
        {error && <p role="alert" className="text-[12px] text-bad">{error}</p>}
      </div>
    </Dialog>
  );
}
