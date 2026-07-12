import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useAppUpdate } from "../hooks/useAppUpdate";
import {
  quitApp,
  openDashboard,
  hidePopup,
  openRunDetail,
  getMissionControlSnapshot,
  listQueuedRuns,
} from "../lib/commands";
import type { MissionControlSnapshot, QueuedRun } from "../lib/commands";
import {
  queueWorkflowRun,
  formatWorkflowQueueOutcome,
  formatWorkflowQueueError,
} from "../lib/workflowEnqueue";
import Button from "./Button";
import BrandMark from "./BrandMark";
import StatusBadge from "./StatusBadge";
import StatusDot from "./StatusDot";
import { PRODUCT_SHORT_NAME } from "../lib/branding";
import { formatRunStatusLabel, statusKey } from "../lib/runStatus";
import { formatDuration } from "../lib/duration";
import { runningRows } from "./missionControl/agentActivityData";
import "./MenuBarPopup.css";

function formatClockTime(isoTime: string): string {
  return new Date(isoTime).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** `in 3h 0m`, or `due now` when the next fire time is at/behind `nowMs`. */
function formatEta(isoTime: string, nowMs: number): string {
  const diff = new Date(isoTime).getTime() - nowMs;
  return diff <= 0 ? "due now" : `in ${formatDuration(diff)}`;
}

/**
 * Poll the two EXISTING read models that back the mini-dashboard: the Mission
 * Control snapshot (running/failed tallies, live activity, upcoming runs, recent
 * runs) and the live queue depth. The snapshot drives the connection/loading
 * state; the queue list is best-effort so a transient queue read never blanks
 * an otherwise-healthy glance. No new IPC/DB query is introduced.
 */
function usePopupData(pollInterval = 30000) {
  const [snapshot, setSnapshot] = useState<MissionControlSnapshot | null>(null);
  const [queuedRuns, setQueuedRuns] = useState<QueuedRun[]>([]);
  const [error, setError] = useState<string | null>(null);
  const latestRequestId = useRef(0);

  const refresh = useCallback(async () => {
    const requestId = latestRequestId.current + 1;
    latestRequestId.current = requestId;
    try {
      const snap = await getMissionControlSnapshot();
      if (requestId !== latestRequestId.current) return;
      setSnapshot(snap);
      setError(null);
    } catch (e) {
      if (requestId !== latestRequestId.current) return;
      setError(String(e));
      return;
    }
    try {
      const queued = await listQueuedRuns();
      if (requestId !== latestRequestId.current) return;
      setQueuedRuns(queued);
    } catch {
      // Best-effort: keep the last known queue depth on a transient failure.
    }
  }, []);

  useEffect(() => {
    const initial = setTimeout(refresh, 0);
    const id = setInterval(refresh, pollInterval);
    return () => {
      clearTimeout(initial);
      clearInterval(id);
    };
  }, [refresh, pollInterval]);

  return { snapshot, queuedRuns, error, refresh };
}

export default function MenuBarPopup() {
  const { snapshot, queuedRuns, error, refresh } = usePopupData(30000);
  const {
    snapshot: updateSnapshot,
    install: installUpdate,
    skipVersion,
  } = useAppUpdate();
  const showTime = useRef(0);
  const [actionFeedback, setActionFeedback] = useState<{
    text: string;
    type: "success" | "error";
  } | null>(null);
  const [queueingWorkflowId, setQueueingWorkflowId] = useState<string | null>(
    null,
  );
  const [updating, setUpdating] = useState(false);
  const [skipping, setSkipping] = useState(false);
  // Read the clock in a deferred effect (never during render — Date.now() is
  // impure) so relative time labels stay deterministic under the frozen test
  // clock. Mirrors Mission Control's Agent Activity view.
  const [clockNow, setClockNow] = useState<number | null>(null);

  const updatePhase = updateSnapshot?.phase;
  const updateDownloading =
    updatePhase === "downloading" || updatePhase === "ready_to_restart";
  // Instant skip feedback — see UpdateBanner for why `phase` alone lags.
  const updateJustSkipped =
    updatePhase === "available" &&
    !!updateSnapshot?.latest_version &&
    updateSnapshot.latest_version === updateSnapshot.skipped_version;
  const showUpdateRow =
    !updateJustSkipped &&
    (updatePhase === "available" ||
      updatePhase === "downloading" ||
      updatePhase === "ready_to_restart");

  const handleUpdate = async () => {
    setActionFeedback(null);
    setUpdating(true);
    try {
      await installUpdate(updateSnapshot?.latest_version ?? undefined);
    } catch (e) {
      setActionFeedback({
        text: `Install failed: ${String(e)}`,
        type: "error",
      });
    } finally {
      setUpdating(false);
    }
  };

  const handleSkip = async () => {
    const version = updateSnapshot?.latest_version;
    if (!version) return;
    setActionFeedback(null);
    setSkipping(true);
    try {
      await skipVersion(version);
    } catch (e) {
      setActionFeedback({
        text: `Could not skip v${version}: ${String(e)}`,
        type: "error",
      });
    } finally {
      setSkipping(false);
    }
  };

  useEffect(() => {
    const id = setTimeout(() => setClockNow(Date.now()), 0);
    return () => clearTimeout(id);
  }, []);

  useEffect(() => {
    const onFocus = () => {
      showTime.current = Date.now();
    };
    const onBlur = () => {
      if (Date.now() - showTime.current > 600) {
        hidePopup();
      }
    };
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    showTime.current = Date.now();
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, []);

  const handleQueue = async (workflowId: string, workflowName: string) => {
    setActionFeedback(null);
    setQueueingWorkflowId(workflowId);
    try {
      const outcome = await queueWorkflowRun(workflowId);
      setActionFeedback({
        text: formatWorkflowQueueOutcome(workflowName, outcome),
        type: "success",
      });
      await refresh();
    } catch (e) {
      setActionFeedback({
        text: formatWorkflowQueueError(workflowName, e),
        type: "error",
      });
    } finally {
      setQueueingWorkflowId(null);
    }
  };

  const now = clockNow ?? 0;

  // Active = the currently-running slice of live activity (shared Agent Activity
  // transform, so the popup reads identically to Mission Control).
  const activeRuns = useMemo(
    () => runningRows(snapshot?.live_activity ?? [], now),
    [snapshot?.live_activity, now],
  );

  // Upcoming = the snapshot's fixed-time triggers, soonest first — the surface
  // for the preserved queue-run affordance.
  const upcomingRuns = useMemo(
    () =>
      (snapshot?.upcoming_runs ?? [])
        .slice()
        .sort(
          (a, b) =>
            new Date(a.next_time).getTime() - new Date(b.next_time).getTime(),
        ),
    [snapshot?.upcoming_runs],
  );

  if (!snapshot) {
    return (
      <main className="popup" aria-labelledby="popup-title">
        <h1 id="popup-title" className="sr-only">
          {PRODUCT_SHORT_NAME}
        </h1>
        <div
          className={error ? "popup-loading popup-error" : "popup-loading"}
          role={error ? "alert" : undefined}
        >
          {error ? `Status failed to load: ${error}` : "Loading..."}
        </div>
      </main>
    );
  }

  const runningCount = snapshot.header.running_count;
  const queuedCount = queuedRuns.length;
  const failedCount = snapshot.header.recent_failures;
  const connected = !error;

  return (
    <main className="popup" aria-labelledby="popup-title">
      <div className="popup-header">
        <div className="popup-brand">
          <BrandMark size={22} title="" className="popup-brand-mark" />
          <h1 id="popup-title" className="popup-title">
            {PRODUCT_SHORT_NAME}
          </h1>
        </div>
        <span
          className={`popup-conn ${connected ? "is-connected" : "is-offline"}`}
        >
          <StatusDot
            variant="mc-dot"
            status={connected ? "success" : "failed"}
          />
          {connected ? "Connected" : "Offline"}
        </span>
      </div>

      <section className="popup-chips" role="group" aria-label="Run summary">
        <div className="popup-chip">
          <StatusDot variant="mc-dot" status="running" />
          <span className="popup-chip-count">{runningCount}</span>
          <span className="popup-chip-label">Running</span>
        </div>
        <div className="popup-chip">
          <StatusDot variant="mc-dot" status="queued" />
          <span className="popup-chip-count">{queuedCount}</span>
          <span className="popup-chip-label">Queued</span>
        </div>
        <div className="popup-chip">
          <StatusDot variant="mc-dot" status="failed" />
          <span className="popup-chip-count">{failedCount}</span>
          <span className="popup-chip-label">Failed</span>
        </div>
      </section>

      {error && (
        <div className="popup-inline-error" role="alert">
          Status refresh failed: {error}
        </div>
      )}
      {actionFeedback && (
        <div
          className={`popup-inline-message ${actionFeedback.type}`}
          role={actionFeedback.type === "error" ? "alert" : "status"}
        >
          {actionFeedback.text}
        </div>
      )}

      <div className="popup-scroll">
        <section className="popup-section" aria-labelledby="popup-active-title">
          <h2 className="popup-section-title" id="popup-active-title">
            Active
          </h2>
          {activeRuns.length === 0 ? (
            <div className="popup-empty">No active runs</div>
          ) : (
            <div className="popup-list">
              {activeRuns.map((run) => (
                <button
                  key={run.key}
                  className="popup-item popup-item-clickable"
                  onClick={() => openRunDetail(run.runId, run.workflowId)}
                >
                  <div className="popup-item-info">
                    <StatusDot
                      variant="mc-dot"
                      status={statusKey(run.status)}
                    />
                    <span className="popup-item-text">
                      <span className="popup-item-name">{run.name}</span>
                      <span className="popup-item-sub">{run.sub}</span>
                    </span>
                  </div>
                  <span className="popup-item-time">{run.timeLabel}</span>
                </button>
              ))}
            </div>
          )}
        </section>

        <section
          className="popup-section"
          aria-labelledby="popup-upcoming-title"
        >
          <h2 className="popup-section-title" id="popup-upcoming-title">
            Upcoming
          </h2>
          {upcomingRuns.length === 0 ? (
            <div className="popup-empty">No upcoming runs</div>
          ) : (
            <div className="popup-list">
              {upcomingRuns.map((nr) => (
                <div
                  key={`${nr.workflow_id}-${nr.trigger_label}`}
                  className="popup-item"
                >
                  <div className="popup-item-info">
                    <span className="popup-item-text">
                      <span className="popup-item-name">
                        {nr.workflow_name}
                      </span>
                      <span className="popup-item-sub">
                        {formatEta(nr.next_time, now)}
                      </span>
                    </span>
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() =>
                      handleQueue(nr.workflow_id, nr.workflow_name)
                    }
                    disabled={queueingWorkflowId === nr.workflow_id}
                    title="Queue run"
                    aria-label={`Queue run ${nr.workflow_name}`}
                  >
                    {queueingWorkflowId === nr.workflow_id
                      ? "Queueing…"
                      : "Queue run"}
                  </Button>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="popup-section" aria-labelledby="popup-recent-title">
          <h2 className="popup-section-title" id="popup-recent-title">
            Recent
          </h2>
          {snapshot.recent_runs.length === 0 ? (
            <div className="popup-empty">No recent runs</div>
          ) : (
            <div className="popup-list">
              {snapshot.recent_runs.map((run) => (
                <button
                  key={run.id}
                  className="popup-item popup-item-clickable"
                  onClick={() => openRunDetail(run.id, run.workflow_id)}
                >
                  <div className="popup-item-info">
                    <StatusDot
                      variant="mc-dot"
                      status={statusKey(run.status)}
                    />
                    <span className="popup-item-text">
                      <span className="popup-item-name">
                        {run.workflow_name ?? run.workflow_id.slice(0, 8)}
                      </span>
                      <span className="popup-item-sub">
                        {formatClockTime(run.started_at)}
                      </span>
                    </span>
                  </div>
                  <StatusBadge status={run.status}>
                    {formatRunStatusLabel(run.status)}
                  </StatusBadge>
                </button>
              ))}
            </div>
          )}
        </section>
      </div>

      {showUpdateRow && (
        <aside
          className="popup-update-row"
          aria-label="Application update"
          aria-busy={updateDownloading || undefined}
        >
          <span className="popup-update-text">
            {updateDownloading
              ? "Installing…"
              : `Update available: v${updateSnapshot?.latest_version ?? "?"}`}
          </span>
          <div className="popup-update-actions">
            {!updateDownloading && (
              <Button
                variant="ghost"
                size="sm"
                disabled={skipping}
                onClick={handleSkip}
              >
                Skip
              </Button>
            )}
            <Button
              variant="primary"
              size="sm"
              disabled={updateDownloading || updating}
              onClick={handleUpdate}
            >
              {updateDownloading ? "Installing…" : "Install"}
            </Button>
          </div>
        </aside>
      )}

      <footer className="popup-footer">
        <Button variant="primary" size="sm" onClick={() => openDashboard()}>
          Open Mission Control
        </Button>
        <div className="popup-footer-secondary">
          {/* Settings lives in the main window; a route-level deep-link would
              need a new IPC/event, so this opens the app where Settings is. */}
          <Button variant="ghost" size="sm" onClick={() => openDashboard()}>
            Settings
          </Button>
          <Button variant="ghost" size="sm" onClick={() => quitApp()}>
            Quit
          </Button>
        </div>
      </footer>
    </main>
  );
}
