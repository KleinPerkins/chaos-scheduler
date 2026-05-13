import { useEffect, useMemo, useState } from "react";
import {
  cancelQueuedRun,
  listQueuedRuns,
  listQueues,
  updateQueue,
} from "../lib/commands";
import type { QueueInfo, QueuedRun } from "../lib/commands";
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
  const [drafts, setDrafts] = useState<Record<string, QueueDraft>>({});
  const [loading, setLoading] = useState(true);
  const [savingId, setSavingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setError(null);
    try {
      const [queueRows, runRows] = await Promise.all([
        listQueues(),
        listQueuedRuns(50),
      ]);
      setQueues(queueRows);
      setQueuedRuns(runRows);
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

  useEffect(() => {
    load();
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
    try {
      await cancelQueuedRun(id);
      await load();
    } catch (e) {
      setError(String(e));
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

      {error && <div className="queue-error">{error}</div>}

      <section className="queue-section">
        <div className="queue-section-header">
          <h2>Queue Capacity</h2>
          <span>cap order: tag &lt;= queue &lt;= global</span>
        </div>
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
                    <div className="queue-corpus">{queue.corpus}</div>
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
                  <span className={validation ? "queue-validation error" : "queue-validation"}>
                    {validation ?? `Global cap ${queue.global_parallelism_cap}`}
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
                  <small>{run.corpus}</small>
                </span>
                <span>{run.queue_name}</span>
                <span className={`queue-status ${run.status}`}>{run.status}</span>
                <span>{run.priority}</span>
                <span>{formatDate(run.queued_at)}</span>
                <span>
                  {run.status === "queued" && (
                    <button
                      className="btn btn-danger btn-sm"
                      onClick={() => cancelRun(run.id)}
                    >
                      Cancel
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
