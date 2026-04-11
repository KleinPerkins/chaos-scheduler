import { useState, useEffect } from "react";
import { getRunLog, openUrl } from "../lib/commands";
import type { Run } from "../lib/commands";
import "./LogViewer.css";

interface Props {
  runId: string;
  onBack: () => void;
}

export default function LogViewer({ runId, onBack }: Props) {
  const [run, setRun] = useState<Run | null>(null);
  const [tab, setTab] = useState<"stdout" | "stderr">("stdout");

  useEffect(() => {
    getRunLog(runId).then(setRun);
  }, [runId]);

  if (!run) {
    return <div className="log-loading">Loading...</div>;
  }

  const content = tab === "stdout" ? run.stdout : run.stderr;

  return (
    <div>
      <div className="page-header">
        <div>
          <h1 className="page-title">Run Log</h1>
          <p className="page-subtitle">
            <span className={`status-badge ${run.status}`}>{run.status}</span>
            {" "}
            {run.exit_code !== null && (
              <span className="log-exit">exit {run.exit_code}</span>
            )}
          </p>
        </div>
        <button className="btn btn-ghost" onClick={onBack}>
          &larr; Back
        </button>
      </div>

      <div className="log-tabs">
        <button
          className={`log-tab ${tab === "stdout" ? "active" : ""}`}
          onClick={() => setTab("stdout")}
        >
          stdout
        </button>
        <button
          className={`log-tab ${tab === "stderr" ? "active" : ""}`}
          onClick={() => setTab("stderr")}
        >
          stderr
        </button>
      </div>

      <pre className="log-output">
        {content || <span className="log-empty">(empty)</span>}
      </pre>

      {run.result_url && (
        <div className="log-result">
          <span>Result: </span>
          <button className="btn btn-ghost btn-sm" onClick={() => openUrl(run.result_url!)}>
            {run.result_url}
          </button>
        </div>
      )}
    </div>
  );
}
