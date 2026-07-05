import { useEffect, useMemo, useState } from "react";
import {
  acknowledgeDeadLetter,
  cancelQueuedRun,
  cleanupRetention,
  dispatchBackfill,
  listDeadLetters,
  listQueuedRuns,
  listQueues,
  planBackfill,
  recoverDeadLetter,
  updateQueue,
} from "../lib/commands";
import { environmentOf } from "../lib/commands";
import type {
  BackfillPlan,
  QueueInfo,
  QueuedRun,
  RetentionPreview,
  SchedulerDeadLetter,
} from "../lib/commands";
import Notice from "./ui/Notice";
import { formatRunStatusLabel } from "../lib/runStatus";
import "./QueueView.css";

interface QueueDraft {
  capacity: string;
  tagCap: string;
  maxQueued: string;
}

function draftFromQueue(queue: QueueInfo): QueueDraft {
  return {
    capacity: String(queue.capacity),
    tagCap: queue.tag_cap == null ? "" : String(queue.tag_cap),
    maxQueued: queue.max_queued == null ? "" : String(queue.max_queued),
  };
}

function parseOptionalInt(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  if (!/^\d+$/.test(trimmed)) return Number.NaN;
  const parsed = Number.parseInt(trimmed, 10);
  return Number.isSafeInteger(parsed) ? parsed : Number.NaN;
}

function validateDraft(queue: QueueInfo, draft: QueueDraft): string | null {
  const capacityText = draft.capacity.trim();
  if (!/^\d+$/.test(capacityText)) {
    return "Queue capacity must be a whole number.";
  }
  const capacity = Number.parseInt(capacityText, 10);
  const tagCap = parseOptionalInt(draft.tagCap);
  const maxQueued = parseOptionalInt(draft.maxQueued);
  if (!Number.isSafeInteger(capacity) || capacity < 1) {
    return "Queue capacity must be at least 1.";
  }
  if (capacity > queue.global_parallelism_cap) {
    return `Queue capacity must be <= global cap (${queue.global_parallelism_cap}).`;
  }
  if (tagCap != null && (!Number.isFinite(tagCap) || tagCap < 1)) {
    return "Tag cap must be blank or at least 1.";
  }
  if (tagCap != null && tagCap > capacity) {
    return "Tag cap must be <= queue capacity.";
  }
  if (maxQueued != null && (!Number.isFinite(maxQueued) || maxQueued < 0)) {
    return "Max queued must be blank or at least 0.";
  }
  return null;
}

function formatDate(value?: string | null): string {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

interface QueueViewProps {
  onBack?: () => void;
}

export default function QueueView({ onBack }: QueueViewProps) {
  const [queues, setQueues] = useState<QueueInfo[]>([]);
  const [queuedRuns, setQueuedRuns] = useState<QueuedRun[]>([]);
  const [deadLetters, setDeadLetters] = useState<SchedulerDeadLetter[]>([]);
  const [drafts, setDrafts] = useState<Record<string, QueueDraft>>({});
  const [backfillWorkflowId, setBackfillWorkflowId] = useState("");
  const [backfillSince, setBackfillSince] = useState("");
  const [backfillUntil, setBackfillUntil] = useState("");
  const [backfillMaxRuns, setBackfillMaxRuns] = useState("10");
  const [backfillPlan, setBackfillPlan] = useState<BackfillPlan | null>(null);
  const [deadLetterReason, setDeadLetterReason] = useState<
    Record<string, string>
  >({});
  const [retentionDays, setRetentionDays] = useState("90");
  const [retentionPreview, setRetentionPreview] =
    useState<RetentionPreview | null>(null);
  const [loading, setLoading] = useState(true);
  const [savingId, setSavingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [backfillBusy, setBackfillBusy] = useState(false);
  const [retentionBusy, setRetentionBusy] = useState(false);
  const [deadLetterBusyId, setDeadLetterBusyId] = useState<string | null>(null);
  const [cancelBusyId, setCancelBusyId] = useState<string | null>(null);
  const [retentionMessage, setRetentionMessage] = useState<string | null>(null);

  const load = async () => {
    setError(null);
    try {
      const [queueRows, runRows, deadLetterRows] = await Promise.all([
        listQueues(),
        listQueuedRuns(50),
        listDeadLetters(false, 50),
      ]);
      setQueues(queueRows);
      setQueuedRuns(runRows);
      setDeadLetters(deadLetterRows);
      setDrafts((current) => {
        const next = { ...current };
        for (const queue of queueRows) {
          const key = `${queue.corpus}/${queue.name}`;
          if (!next[key]) next[key] = draftFromQueue(queue);
        }
        return next;
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  // Defer the initial load to a macrotask so load()'s synchronous
  // setError(null) does not run inside the effect body (avoids
  // react-hooks/set-state-in-effect). Mirrors useSchedulerStatus.
  useEffect(() => {
    const id = setTimeout(() => void load(), 0);
    return () => clearTimeout(id);
  }, []);

  const validationByQueue = useMemo(() => {
    const result: Record<string, string | null> = {};
    for (const queue of queues) {
      const key = `${queue.corpus}/${queue.name}`;
      result[key] = validateDraft(queue, drafts[key] ?? draftFromQueue(queue));
    }
    return result;
  }, [drafts, queues]);

  const saveQueue = async (queue: QueueInfo) => {
    const key = `${queue.corpus}/${queue.name}`;
    const draft = drafts[key] ?? draftFromQueue(queue);
    const validation = validateDraft(queue, draft);
    if (validation) {
      setError(validation);
      return;
    }
    setSavingId(key);
    setError(null);
    try {
      await updateQueue(
        queue.name,
        queue.corpus,
        Number.parseInt(draft.capacity, 10),
        parseOptionalInt(draft.tagCap),
        parseOptionalInt(draft.maxQueued),
      );
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setSavingId(null);
    }
  };

  const cancelRun = async (id: string) => {
    setError(null);
    setCancelBusyId(id);
    try {
      await cancelQueuedRun(id);
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setCancelBusyId(null);
    }
  };

  const maxRunsValue = () => {
    const parsed = parseOptionalInt(backfillMaxRuns);
    return parsed == null || !Number.isFinite(parsed) ? null : parsed;
  };

  const previewBackfill = async () => {
    setError(null);
    setBackfillBusy(true);
    try {
      const plan = await planBackfill(
        backfillWorkflowId.trim(),
        backfillSince,
        backfillUntil,
        maxRunsValue(),
      );
      setBackfillPlan(plan);
    } catch (e) {
      setError(String(e));
    } finally {
      setBackfillBusy(false);
    }
  };

  const dispatchBackfillPlan = async () => {
    setError(null);
    setBackfillBusy(true);
    try {
      const result = await dispatchBackfill(
        backfillWorkflowId.trim(),
        backfillSince,
        backfillUntil,
        maxRunsValue(),
        false,
      );
      setBackfillPlan(result.plan);
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setBackfillBusy(false);
    }
  };

  const acknowledge = async (id: string) => {
    const reason = (deadLetterReason[id] ?? "").trim();
    if (!reason) {
      setError("Acknowledgement reason is required.");
      return;
    }
    setError(null);
    setDeadLetterBusyId(id);
    try {
      await acknowledgeDeadLetter(id, reason, "scheduler-ui", false);
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setDeadLetterBusyId(null);
    }
  };

  const recover = async (id: string) => {
    setError(null);
    setDeadLetterBusyId(id);
    try {
      await recoverDeadLetter(id, true);
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setDeadLetterBusyId(null);
    }
  };

  const runRetention = async (dryRun: boolean) => {
    const days = Number.parseInt(retentionDays, 10);
    if (!Number.isSafeInteger(days) || days < 1) {
      setError("Retention days must be a positive whole number.");
      return;
    }
    setError(null);
    setRetentionBusy(true);
    try {
      const result = await cleanupRetention(days, dryRun);
      setRetentionPreview(result);
      setRetentionMessage(
        dryRun
          ? `Dry run: ${result.candidate_runs} run(s) would be deleted; ${result.preserved_dead_letter_runs} dead-letter run(s) preserved.`
          : `Deleted ${result.deleted_runs ?? result.candidate_runs} run(s); ${result.preserved_dead_letter_runs} dead-letter run(s) preserved.`,
      );
      if (!dryRun) await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setRetentionBusy(false);
    }
  };

  if (loading) {
    return <div className="queue-loading">Loading queues...</div>;
  }

  return (
    <div className="queue-view">
      <div className="page-header">
        <div>
          <h1 className="page-title">Queues</h1>
          <p className="page-subtitle">
            Capacity, tag caps, and queued workflow administration
          </p>
        </div>
        <div className="queue-actions">
          {onBack && (
            <button className="btn btn-ghost" onClick={onBack}>
              Back to Mission Control
            </button>
          )}
          <button className="btn btn-ghost" onClick={load}>
            Refresh
          </button>
        </div>
      </div>

      {error && (
        <Notice variant="error" assertive>
          {error}
        </Notice>
      )}

      <section className="queue-section">
        <div className="queue-section-header">
          <h2>Queue Capacity</h2>
          <span>cap order: tag &lt;= queue &lt;= global</span>
        </div>
        {queues.length === 0 ? (
          <div className="queue-empty">No queues configured yet.</div>
        ) : (
          <div className="queue-grid">
            {queues.map((queue) => {
              const key = `${queue.corpus}/${queue.name}`;
              const draft = drafts[key] ?? draftFromQueue(queue);
              const validation = validationByQueue[key];
              return (
                <div key={key} className="queue-card">
                  <div className="queue-card-header">
                    <div>
                      <div className="queue-name">{queue.name}</div>
                      <div className="queue-corpus">{environmentOf(queue)}</div>
                    </div>
                    <div className="queue-counts">
                      <span>{queue.active_count} active</span>
                      <span>{queue.queued_count} queued</span>
                    </div>
                  </div>

                  <div className="queue-fields">
                    <label>
                      Capacity
                      <input
                        type="number"
                        inputMode="numeric"
                        pattern="\d+"
                        min={1}
                        max={queue.global_parallelism_cap}
                        value={draft.capacity}
                        onChange={(e) =>
                          setDrafts((d) => ({
                            ...d,
                            [key]: { ...draft, capacity: e.target.value },
                          }))
                        }
                      />
                    </label>
                    <label>
                      Tag cap
                      <input
                        type="number"
                        inputMode="numeric"
                        pattern="\d+"
                        min={1}
                        value={draft.tagCap}
                        placeholder="inherits queue"
                        onChange={(e) =>
                          setDrafts((d) => ({
                            ...d,
                            [key]: { ...draft, tagCap: e.target.value },
                          }))
                        }
                      />
                    </label>
                    <label>
                      Max queued
                      <input
                        type="number"
                        inputMode="numeric"
                        pattern="\d+"
                        min={0}
                        value={draft.maxQueued}
                        placeholder="unbounded"
                        onChange={(e) =>
                          setDrafts((d) => ({
                            ...d,
                            [key]: { ...draft, maxQueued: e.target.value },
                          }))
                        }
                      />
                    </label>
                  </div>

                  <div className="queue-card-footer">
                    <span
                      className={
                        validation
                          ? "queue-validation error"
                          : "queue-validation"
                      }
                    >
                      {validation ??
                        `Global cap ${queue.global_parallelism_cap}`}
                    </span>
                    <button
                      className="btn btn-primary btn-sm"
                      disabled={!!validation || savingId === key}
                      onClick={() => saveQueue(queue)}
                    >
                      {savingId === key ? "Saving..." : "Save"}
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      <section className="queue-section">
        <div className="queue-section-header">
          <h2>Backfill Dispatch</h2>
          <span>Historical runs use normal queue and dependency admission</span>
        </div>
        <div className="queue-card">
          <div className="queue-fields">
            <label>
              Workflow ID
              <input
                value={backfillWorkflowId}
                placeholder="daily-digest"
                onChange={(e) => setBackfillWorkflowId(e.target.value)}
              />
            </label>
            <label>
              Since
              <input
                value={backfillSince}
                placeholder="2026-05-01T00:00:00Z"
                onChange={(e) => setBackfillSince(e.target.value)}
              />
            </label>
            <label>
              Until
              <input
                value={backfillUntil}
                placeholder="2026-05-03T00:00:00Z"
                onChange={(e) => setBackfillUntil(e.target.value)}
              />
            </label>
            <label>
              Max runs
              <input
                value={backfillMaxRuns}
                inputMode="numeric"
                pattern="\d+"
                onChange={(e) => setBackfillMaxRuns(e.target.value)}
              />
            </label>
          </div>
          <div className="queue-card-footer">
            <span className="queue-validation">
              {backfillPlan
                ? `${backfillPlan.count} logical slot(s), chain suppression on`
                : "Preview before dispatching."}
            </span>
            <button
              className="btn btn-ghost btn-sm"
              disabled={
                backfillBusy ||
                !backfillWorkflowId ||
                !backfillSince ||
                !backfillUntil
              }
              onClick={previewBackfill}
            >
              {backfillBusy ? "Working…" : "Preview"}
            </button>
            <button
              className="btn btn-primary btn-sm"
              disabled={
                backfillBusy || !backfillPlan || backfillPlan.count === 0
              }
              onClick={dispatchBackfillPlan}
            >
              {backfillBusy ? "Dispatching…" : "Dispatch"}
            </button>
          </div>
        </div>
      </section>

      <section className="queue-section">
        <div className="queue-section-header">
          <h2>Dead Letters</h2>
          <span>Acknowledge or recover failed task runs with audit state</span>
        </div>
        {deadLetters.length === 0 ? (
          <div className="queue-empty">No unacknowledged dead-letter rows.</div>
        ) : (
          <div className="queue-run-table">
            <div className="queue-run-row header">
              <span>Workflow</span>
              <span>Task</span>
              <span>Failure</span>
              <span>Reason</span>
              <span />
            </div>
            {deadLetters.map((row) => (
              <div key={row.id} className="queue-run-row">
                <span>
                  <strong>{row.workflow_name ?? row.workflow_id}</strong>
                  <small>{row.run_status ?? "unknown"}</small>
                </span>
                <span>{row.task_id ?? "-"}</span>
                <span>{formatDate(row.last_failure_at)}</span>
                <span>
                  <input
                    value={deadLetterReason[row.id] ?? ""}
                    placeholder="ack reason"
                    onChange={(e) =>
                      setDeadLetterReason((current) => ({
                        ...current,
                        [row.id]: e.target.value,
                      }))
                    }
                  />
                </span>
                <span>
                  <button
                    className="btn btn-ghost btn-sm"
                    onClick={() => acknowledge(row.id)}
                    disabled={deadLetterBusyId === row.id}
                  >
                    {deadLetterBusyId === row.id ? "…" : "Ack"}
                  </button>
                  <button
                    className="btn btn-primary btn-sm"
                    onClick={() => recover(row.id)}
                    disabled={deadLetterBusyId === row.id}
                  >
                    {deadLetterBusyId === row.id ? "…" : "Recover"}
                  </button>
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="queue-section">
        <div className="queue-section-header">
          <h2>Retention Cleanup</h2>
          <span>Dry-run first; dead-letter evidence is preserved</span>
        </div>
        <div className="queue-card">
          <div className="queue-fields">
            <label>
              Delete runs older than days
              <input
                value={retentionDays}
                inputMode="numeric"
                pattern="\d+"
                onChange={(e) => setRetentionDays(e.target.value)}
              />
            </label>
          </div>
          <div className="queue-card-footer">
            <span className="queue-validation">
              {retentionMessage ??
                (retentionPreview
                  ? `${retentionPreview.candidate_runs} candidate run(s), ${retentionPreview.preserved_dead_letter_runs} dead-letter run(s) preserved`
                  : "Retention cleanup never deletes scheduler_dead_letters evidence.")}
            </span>
            <button
              className="btn btn-ghost btn-sm"
              onClick={() => runRetention(true)}
              disabled={retentionBusy}
            >
              {retentionBusy ? "Working…" : "Dry Run"}
            </button>
            <button
              className="btn btn-danger btn-sm"
              disabled={
                retentionBusy ||
                !retentionPreview ||
                retentionPreview.candidate_runs === 0
              }
              onClick={() => runRetention(false)}
            >
              {retentionBusy ? "Applying…" : "Apply Cleanup"}
            </button>
          </div>
        </div>
      </section>

      <section className="queue-section">
        <div className="queue-section-header">
          <h2>Queued Runs</h2>
          <span>Cancel waiting runs without touching completed history</span>
        </div>

        {queuedRuns.length === 0 ? (
          <div className="queue-empty">No queued runs.</div>
        ) : (
          <div className="queue-run-table">
            <div className="queue-run-row header">
              <span>Workflow</span>
              <span>Queue</span>
              <span>Status</span>
              <span>Priority</span>
              <span>Queued</span>
              <span />
            </div>
            {queuedRuns.map((run) => (
              <div key={run.id} className="queue-run-row">
                <span>
                  <strong>{run.workflow_name ?? run.workflow_id}</strong>
                  <small>{environmentOf(run)}</small>
                </span>
                <span>{run.queue_name}</span>
                <span className={`queue-status ${run.status}`}>
                  {formatRunStatusLabel(run.status)}
                </span>
                <span>{run.priority}</span>
                <span>{formatDate(run.queued_at)}</span>
                <span>
                  {run.status === "queued" && (
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={() => cancelRun(run.id)}
                      disabled={cancelBusyId === run.id}
                    >
                      {cancelBusyId === run.id ? "…" : "Cancel"}
                    </button>
                  )}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
