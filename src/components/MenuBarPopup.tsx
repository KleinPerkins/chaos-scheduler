import { useEffect, useMemo, useRef, useState } from "react";
import { Plus } from "lucide-react";
import { useSchedulerStatus } from "../hooks/useSchedulerStatus";
import { useAppUpdate } from "../hooks/useAppUpdate";
import {
  quitApp,
  openDashboard,
  hidePopup,
  openRunDetail,
  environmentOf,
} from "../lib/commands";
import {
  queueWorkflowRun,
  formatWorkflowQueueOutcome,
  formatWorkflowQueueError,
} from "../lib/workflowEnqueue";
import Button from "./Button";
import StatusBadge from "./StatusBadge";
import type { NextRun } from "../lib/commands";
import { PRODUCT_SHORT_NAME } from "../lib/branding";
import { formatRunStatusLabel } from "../lib/runStatus";
import "./MenuBarPopup.css";

function envLabel(name: string): string {
  return name.charAt(0).toUpperCase() + name.slice(1);
}

function formatTimeUntil(isoTime: string): string {
  const diff = new Date(isoTime).getTime() - Date.now();
  if (diff < 0) return "overdue";
  const mins = Math.floor(diff / 60000);
  const hours = Math.floor(mins / 60);
  const days = Math.floor(hours / 24);
  if (days > 0) return `${days}d ${hours % 24}h`;
  if (hours > 0) return `${hours}h ${mins % 60}m`;
  return `${mins}m`;
}

function formatTime(isoTime: string): string {
  return new Date(isoTime).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export default function MenuBarPopup() {
  const { status, error, refresh } = useSchedulerStatus(30000);
  const { snapshot: updateSnapshot, install: installUpdate } = useAppUpdate();
  const showTime = useRef(0);
  const [action, setAction] = useState<{
    text: string;
    type: "success" | "error";
  } | null>(null);
  const [runningWorkflowId, setRunningWorkflowId] = useState<string | null>(
    null,
  );
  const [updating, setUpdating] = useState(false);

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
    setAction(null);
    setUpdating(true);
    try {
      await installUpdate(updateSnapshot?.latest_version ?? undefined);
    } catch (e) {
      setAction({ text: `Update failed: ${String(e)}`, type: "error" });
    } finally {
      setUpdating(false);
    }
  };

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
    setAction(null);
    setRunningWorkflowId(workflowId);
    try {
      const outcome = await queueWorkflowRun(workflowId);
      setAction({
        text: formatWorkflowQueueOutcome(workflowName, outcome),
        type: "success",
      });
      await refresh();
    } catch (e) {
      setAction({
        text: formatWorkflowQueueError(workflowName, e),
        type: "error",
      });
    } finally {
      setRunningWorkflowId(null);
    }
  };

  // Group upcoming runs by environment dynamically.
  const groupedNextRuns = useMemo(() => {
    const groups = new Map<string, NextRun[]>();
    for (const nr of status?.next_runs ?? []) {
      const env = environmentOf(nr);
      if (!groups.has(env)) groups.set(env, []);
      groups.get(env)!.push(nr);
    }
    return Array.from(groups.entries()).sort((a, b) =>
      a[0].localeCompare(b[0]),
    );
  }, [status?.next_runs]);

  if (!status) {
    return (
      <main className="popup" aria-labelledby="popup-title">
        <h1 id="popup-title" className="sr-only">
          {PRODUCT_SHORT_NAME}
        </h1>
        <div className={error ? "popup-loading popup-error" : "popup-loading"}>
          {error ? `Status failed to load: ${error}` : "Loading..."}
        </div>
      </main>
    );
  }

  return (
    <main className="popup" aria-labelledby="popup-title">
      <div className="popup-header">
        <h1 id="popup-title" className="popup-title">
          {PRODUCT_SHORT_NAME}
        </h1>
        <span className="popup-meta">
          {status.active_workflows} active
          {status.running_count > 0 && (
            <span className="running-indicator">
              {" "}
              &middot; {status.running_count} running
            </span>
          )}
        </span>
      </div>
      {error && (
        <div className="popup-inline-error">Status refresh failed: {error}</div>
      )}
      {action && (
        <div
          className={
            action.type === "error"
              ? "popup-inline-error"
              : "popup-inline-notice"
          }
          role={action.type === "error" ? "alert" : "status"}
        >
          {action.text}
        </div>
      )}

      <div className="popup-scroll">
        <div className="popup-section">
          <div className="popup-section-title">Next Runs</div>
          {status.next_runs.length === 0 ? (
            <div className="popup-empty">No scheduled workflows</div>
          ) : (
            <>
              {groupedNextRuns.map(([env, runs]) => (
                <div key={env} className="popup-env-group">
                  <div className="popup-env-title">
                    {envLabel(env)} Workflows
                    <span>{runs.length}</span>
                  </div>
                  <div className="popup-list">
                    {runs.map((nr) => (
                      <div key={nr.workflow_id} className="popup-item">
                        <div className="popup-item-info">
                          <span className="popup-item-name">
                            {nr.workflow_name}
                          </span>
                          <span className="popup-item-time">
                            in {formatTimeUntil(nr.next_time)}
                          </span>
                        </div>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() =>
                            handleQueue(nr.workflow_id, nr.workflow_name)
                          }
                          disabled={runningWorkflowId === nr.workflow_id}
                          title="Queue run"
                          aria-label={`Queue ${nr.workflow_name}`}
                        >
                          {runningWorkflowId === nr.workflow_id ? (
                            "..."
                          ) : (
                            <Plus
                              size={12}
                              strokeWidth={2.5}
                              aria-hidden="true"
                            />
                          )}
                        </Button>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </>
          )}
        </div>

        <div className="popup-section">
          <div className="popup-section-title">Recent Results</div>
          {status.recent_runs.length === 0 ? (
            <div className="popup-empty">No runs yet</div>
          ) : (
            <div className="popup-list">
              {status.recent_runs.map((run) => (
                <button
                  key={run.id}
                  className="popup-item popup-item-clickable"
                  onClick={() => openRunDetail(run.id, run.workflow_id)}
                >
                  <div className="popup-item-info">
                    <span className={`popup-dot ${run.status}`} />
                    <span className="popup-item-name">
                      {run.workflow_name ?? run.workflow_id.slice(0, 8)}
                    </span>
                    <span className="popup-item-time">
                      {formatTime(run.started_at)}
                    </span>
                  </div>
                  <StatusBadge status={run.status}>
                    {formatRunStatusLabel(run.status)}
                  </StatusBadge>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {showUpdateRow && (
        <div
          className="popup-update-row"
          aria-busy={updateDownloading || undefined}
        >
          <span className="popup-update-text">
            {updateDownloading
              ? "Updating…"
              : `Update available: v${updateSnapshot?.latest_version ?? "?"}`}
          </span>
          <Button
            variant="primary"
            size="sm"
            disabled={updateDownloading || updating}
            onClick={handleUpdate}
          >
            {updateDownloading ? "Updating…" : "Update"}
          </Button>
        </div>
      )}

      <div className="popup-footer">
        <Button variant="primary" size="sm" onClick={() => openDashboard()}>
          Open Mission Control
        </Button>
        <Button variant="ghost" size="sm" onClick={() => quitApp()}>
          Quit
        </Button>
      </div>
    </main>
  );
}
