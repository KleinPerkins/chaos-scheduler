import { useEffect, useRef } from "react";
import { useSchedulerStatus } from "../hooks/useSchedulerStatus";
import { triggerWorkflow, quitApp, openDashboard, hidePopup, openRunDetail } from "../lib/commands";
import "./MenuBarPopup.css";

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
  const { status, refresh } = useSchedulerStatus(30000);
  const showTime = useRef(0);

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

  const handleRun = async (workflowId: string) => {
    try {
      await triggerWorkflow(workflowId);
      refresh();
    } catch (e) {
      console.error("Failed to trigger workflow:", e);
    }
  };

  if (!status) {
    return (
      <div className="popup">
        <div className="popup-loading">Loading...</div>
      </div>
    );
  }

  return (
    <div className="popup">
      <div className="popup-header">
        <span className="popup-title">Chaos Labs</span>
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

      <div className="popup-scroll">
        <div className="popup-section">
          <div className="popup-section-title">Next Runs</div>
          {status.next_runs.length === 0 ? (
            <div className="popup-empty">No scheduled workflows</div>
          ) : (
            <div className="popup-list">
              {status.next_runs.map((nr) => (
                <div key={nr.workflow_id} className="popup-item">
                  <div className="popup-item-info">
                    <span className="popup-item-name">{nr.workflow_name}</span>
                    <span className="popup-item-time">
                      in {formatTimeUntil(nr.next_time)}
                    </span>
                  </div>
                  <button
                    className="btn btn-ghost btn-sm"
                    onClick={() => handleRun(nr.workflow_id)}
                    title="Run now"
                  >
                    &#9654;
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="popup-section">
          <div className="popup-section-title">Recent Results</div>
          {status.recent_runs.length === 0 ? (
            <div className="popup-empty">No runs yet</div>
          ) : (
            <div className="popup-list">
              {status.recent_runs.map((run) => (
                <div
                  key={run.id}
                  className="popup-item popup-item-clickable"
                  onClick={() => openRunDetail(run.id, run.workflow_id)}
                >
                  <div className="popup-item-info">
                    <span className={`popup-dot ${run.status}`} />
                    <span className="popup-item-name">{run.workflow_name ?? run.workflow_id.slice(0, 8)}</span>
                    <span className="popup-item-time">
                      {formatTime(run.started_at)}
                    </span>
                  </div>
                  <span className={`status-badge ${run.status}`}>
                    {run.status}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="popup-footer">
        <button className="btn btn-primary btn-sm" onClick={() => openDashboard()}>
          Open Dashboard
        </button>
        <button className="btn btn-ghost btn-sm" onClick={() => quitApp()}>
          Quit
        </button>
      </div>
    </div>
  );
}
