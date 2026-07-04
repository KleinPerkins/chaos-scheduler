import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { useWorkflows } from "../hooks/useWorkflows";
import { useEnvironments } from "../hooks/useEnvironments";
import {
  triggerWorkflow,
  updateWorkflow,
  deleteWorkflow,
  environmentOf,
} from "../lib/commands";
import type { Workflow } from "../lib/commands";
import { cronToHuman } from "./ScheduleBuilder";
import EnvironmentBadge from "./EnvironmentBadge";
import "./WorkflowList.css";

interface Props {
  onEdit: (workflow: Workflow) => void;
  onNew: () => void;
  onHistory: (workflow: Workflow) => void;
}

type FrequencyGroup = "Hourly" | "Daily" | "Weekly" | "Monthly";
type EnvFilter = string; // "all" or an environment name

const GROUP_ORDER: FrequencyGroup[] = ["Hourly", "Daily", "Weekly", "Monthly"];

function getFrequencyGroup(cronSchedule: string): FrequencyGroup {
  if (cronSchedule.includes(";")) {
    const groups = cronSchedule
      .split(";")
      .map((c) => c.trim())
      .filter(Boolean)
      .map((c) => getFrequencyGroup(c));
    const priority: FrequencyGroup[] = ["Hourly", "Daily", "Weekly", "Monthly"];
    return priority.find((p) => groups.includes(p)) || "Daily";
  }

  const parts = cronSchedule.trim().split(/\s+/);
  let hour: string, dom: string, dow: string;

  if (parts.length >= 7) {
    [, , hour, dom, , dow] = parts;
  } else if (parts.length === 6) {
    [, , hour, dom, , dow] = parts;
  } else if (parts.length === 5) {
    [, hour, dom, , dow] = parts;
  } else {
    return "Daily";
  }

  if (hour.startsWith("*/")) return "Hourly";
  if (dom !== "*") return "Monthly";
  if (dow !== "*" && dow !== "Mon-Fri") return "Weekly";
  return "Daily";
}

export default function WorkflowList({ onEdit, onNew, onHistory }: Props) {
  const { workflows, loading, refresh } = useWorkflows();
  const { environments } = useEnvironments();
  const [envFilter, setEnvFilter] = useState<EnvFilter>("all");
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [hoveredDescId, setHoveredDescId] = useState<string | null>(null);
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  useEffect(() => {
    return () => {
      clearTimeout(deleteTimerRef.current);
    };
  }, []);

  // Environment options are sourced from the environments backend and unioned
  // with any environments observed on the current workflows (so a workflow in
  // an env that was since removed still surfaces a filter).
  const envCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const workflow of workflows) {
      const env = environmentOf(workflow);
      counts.set(env, (counts.get(env) ?? 0) + 1);
    }
    return counts;
  }, [workflows]);

  const envOptions = useMemo(() => {
    const names = new Set<string>();
    for (const env of environments) names.add(env.name);
    for (const name of envCounts.keys()) names.add(name);
    return Array.from(names).sort((a, b) => a.localeCompare(b));
  }, [environments, envCounts]);

  const visibleWorkflows = useMemo(() => {
    if (envFilter === "all") return workflows;
    return workflows.filter(
      (workflow) => environmentOf(workflow) === envFilter,
    );
  }, [workflows, envFilter]);

  const groupedWorkflows = useMemo(() => {
    const groups = new Map<FrequencyGroup, Workflow[]>();
    for (const w of visibleWorkflows) {
      const group = getFrequencyGroup(w.cron_schedule);
      if (!groups.has(group)) groups.set(group, []);
      groups.get(group)!.push(w);
    }
    return GROUP_ORDER.filter((g) => groups.has(g)).map((g) => ({
      group: g,
      workflows: groups.get(g)!,
    }));
  }, [visibleWorkflows]);

  const handleToggle = async (w: Workflow) => {
    await updateWorkflow({
      id: w.id,
      name: w.name,
      description: w.description ?? undefined,
      scriptPath: w.script_path,
      cronSchedule: w.cron_schedule,
      enabled: !w.enabled,
      asyncMode: w.async_mode,
      emailOnFailure: w.email_on_failure,
      timezone: w.timezone,
      corpus: w.corpus ?? "source",
      domain: w.domain,
    });
    refresh();
  };

  const handleDelete = async (w: Workflow) => {
    if (pendingDeleteId !== w.id) {
      setPendingDeleteId(w.id);
      clearTimeout(deleteTimerRef.current);
      deleteTimerRef.current = setTimeout(() => setPendingDeleteId(null), 3000);
      return;
    }
    clearTimeout(deleteTimerRef.current);
    setPendingDeleteId(null);
    await deleteWorkflow(w.id);
    refresh();
  };

  const handleRun = async (w: Workflow) => {
    await triggerWorkflow(w.id);
    refresh();
  };

  const handleDescEnter = useCallback(
    (e: React.MouseEvent<HTMLDivElement>, wId: string) => {
      const el = e.currentTarget.querySelector(".wf-card-desc") as HTMLElement;
      if (el && el.scrollHeight > el.clientHeight + 1) {
        setHoveredDescId(wId);
      }
    },
    [],
  );

  const handleDescLeave = useCallback(() => {
    setHoveredDescId(null);
  }, []);

  if (loading) {
    return <div className="wf-loading">Loading workflows...</div>;
  }

  const renderCard = (w: Workflow) => (
    <div key={w.id} className={`wf-card ${!w.enabled ? "disabled" : ""}`}>
      <div className="wf-card-header">
        <div className="wf-card-title-row">
          <div className="wf-card-title">{w.name}</div>
          <EnvironmentBadge
            environment={environmentOf(w)}
            managed={w.managed_externally}
            size="sm"
          />
        </div>
        <label className="wf-toggle">
          <input
            type="checkbox"
            checked={w.enabled}
            onChange={() => handleToggle(w)}
            aria-label={`${w.enabled ? "Disable" : "Enable"} ${w.name}`}
          />
          <span className="wf-toggle-track" aria-hidden="true" />
        </label>
      </div>
      {w.description && (
        <div
          className="wf-card-desc-zone"
          onMouseEnter={(e) => handleDescEnter(e, w.id)}
          onMouseLeave={handleDescLeave}
        >
          <div
            className={`wf-card-desc ${hoveredDescId === w.id ? "wf-card-desc-active" : ""}`}
          >
            {w.description}
          </div>
          {hoveredDescId === w.id && (
            <div className="wf-card-desc-tooltip">{w.description}</div>
          )}
        </div>
      )}
      <div className="wf-card-meta">
        <span className="wf-card-schedule">
          {cronToHuman(w.cron_schedule, w.timezone)}
        </span>
        <span className="wf-card-script">{w.script_path}</span>
      </div>
      <div className="wf-card-actions">
        <button
          className="btn btn-ghost btn-sm"
          onClick={() => handleRun(w)}
          title="Run now"
        >
          &#9654; Run
        </button>
        <button className="btn btn-ghost btn-sm" onClick={() => onHistory(w)}>
          History
        </button>
        <button className="btn btn-ghost btn-sm" onClick={() => onEdit(w)}>
          Edit
        </button>
        <button
          className={`btn btn-sm ${pendingDeleteId === w.id ? "btn-danger-confirm" : "btn-danger"}`}
          onClick={() => handleDelete(w)}
        >
          {pendingDeleteId === w.id ? "Confirm?" : "Delete"}
        </button>
      </div>
    </div>
  );

  return (
    <div>
      <div className="page-header">
        <div>
          <h1 className="page-title">Workflows</h1>
          <p className="page-subtitle">
            {workflows.length} workflow{workflows.length !== 1 ? "s" : ""}{" "}
            configured
          </p>
        </div>
        <button className="btn btn-primary" onClick={onNew}>
          + Add Workflow
        </button>
      </div>

      {workflows.length === 0 ? (
        <div className="wf-empty">
          <p className="wf-empty-title">No workflows yet</p>
          <p className="wf-empty-sub">
            Create your first workflow to start automating PM tasks.
          </p>
          <button className="btn btn-primary" onClick={onNew}>
            + Add Workflow
          </button>
        </div>
      ) : (
        <>
          <div
            className="wf-corpus-filter"
            role="group"
            aria-label="Filter workflows by environment"
          >
            <button
              className={`wf-corpus-pill ${envFilter === "all" ? "active" : ""}`}
              onClick={() => setEnvFilter("all")}
              aria-pressed={envFilter === "all"}
            >
              All
              <span>{workflows.length}</span>
            </button>
            {envOptions.map((name) => (
              <button
                key={name}
                className={`wf-corpus-pill ${envFilter === name ? "active" : ""}`}
                onClick={() => setEnvFilter(name)}
                aria-pressed={envFilter === name}
              >
                {name.charAt(0).toUpperCase() + name.slice(1)}
                <span>{envCounts.get(name) ?? 0}</span>
              </button>
            ))}
          </div>

          {visibleWorkflows.length === 0 ? (
            <div className="wf-empty wf-empty-compact">
              <p className="wf-empty-title">No workflows in {envFilter}</p>
              <p className="wf-empty-sub">
                No workflows are assigned to this environment yet.
              </p>
            </div>
          ) : (
            <div className="wf-groups">
              {groupedWorkflows.map(({ group, workflows: groupWfs }) => (
                <div key={group} className="wf-group">
                  <div className="wf-group-header">
                    <span className="wf-group-label">{group}</span>
                    <span className="wf-group-count">{groupWfs.length}</span>
                    <span className="wf-group-divider" />
                  </div>
                  <div className="wf-grid">{groupWfs.map(renderCard)}</div>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
