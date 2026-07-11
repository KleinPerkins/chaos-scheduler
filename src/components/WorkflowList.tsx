import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { Clock } from "lucide-react";
import { useWorkflows } from "../hooks/useWorkflows";
import { useEnvironments } from "../hooks/useEnvironments";
import { updateWorkflow, deleteWorkflow, environmentOf } from "../lib/commands";
import type { Workflow } from "../lib/commands";
import { cronToHuman } from "./ScheduleBuilder";
import EnvironmentBadge from "./EnvironmentBadge";
import NoticeBanner from "./NoticeBanner";
import Button from "./Button";
import PageHeader from "./PageHeader";
import {
  formatWorkflowQueueError,
  formatWorkflowQueueOutcome,
  queueWorkflowRun,
} from "../lib/workflowEnqueue";
import "./WorkflowList.css";

interface Props {
  onOpen: (workflow: Workflow) => void;
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

interface DescriptionBlockProps {
  workflowId: string;
  description: string;
  expanded: boolean;
  onToggle: (truncated: boolean) => void;
}

function DescriptionBlock({
  description,
  expanded,
  onToggle,
}: DescriptionBlockProps) {
  const descRef = useRef<HTMLParagraphElement>(null);
  const [truncated, setTruncated] = useState(false);

  useEffect(() => {
    const el = descRef.current;
    if (!el) return;
    const check = () => setTruncated(el.scrollHeight > el.clientHeight + 1);
    check();
    window.addEventListener("resize", check);
    return () => window.removeEventListener("resize", check);
  }, [description]);

  return (
    <div className="wf-card-desc-zone">
      {truncated ? (
        <button
          type="button"
          className="wf-card-desc-button"
          aria-expanded={expanded}
          onClick={() => onToggle(truncated)}
        >
          <span
            ref={descRef}
            className={`wf-card-desc ${expanded ? "wf-card-desc-expanded" : ""}`}
          >
            {description}
          </span>
          <span className="wf-card-desc-hint">
            {expanded ? "Show less" : "Show full description"}
          </span>
        </button>
      ) : (
        <p ref={descRef} className="wf-card-desc wf-card-desc-full">
          {description}
        </p>
      )}
      {expanded && truncated && (
        <div className="wf-card-desc-tooltip">{description}</div>
      )}
    </div>
  );
}

export default function WorkflowList({
  onOpen,
  onEdit,
  onNew,
  onHistory,
}: Props) {
  const { workflows, loading, error, refresh } = useWorkflows();
  const { environments } = useEnvironments();
  const [envFilter, setEnvFilter] = useState<EnvFilter>("all");
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [expandedDescId, setExpandedDescId] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<{
    id: string;
    kind: "toggle" | "delete";
  } | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [runNotice, setRunNotice] = useState<string | null>(null);
  const [pendingEnqueueId, setPendingEnqueueId] = useState<string | null>(null);
  const deleteTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );
  const noticeTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  useEffect(() => {
    return () => {
      clearTimeout(deleteTimerRef.current);
      clearTimeout(noticeTimerRef.current);
    };
  }, []);

  const showRunNotice = (message: string) => {
    setRunNotice(message);
    clearTimeout(noticeTimerRef.current);
    noticeTimerRef.current = setTimeout(() => setRunNotice(null), 5000);
  };

  const isPending = (w: Workflow, kind: "toggle" | "delete") =>
    pendingAction?.id === w.id && pendingAction.kind === kind;

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
    setActionError(null);
    setPendingAction({ id: w.id, kind: "toggle" });
    try {
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
        environment: w.environment,
        domain: w.domain,
      });
      await refresh();
    } catch (e) {
      setActionError(
        `Failed to ${w.enabled ? "disable" : "enable"} ${w.name}: ${e}`,
      );
    } finally {
      setPendingAction(null);
    }
  };

  const handleDelete = async (w: Workflow) => {
    if (pendingDeleteId !== w.id) {
      setPendingDeleteId(w.id);
      clearTimeout(deleteTimerRef.current);
      deleteTimerRef.current = setTimeout(() => setPendingDeleteId(null), 3000);
      return;
    }
    setActionError(null);
    setPendingAction({ id: w.id, kind: "delete" });
    clearTimeout(deleteTimerRef.current);
    setPendingDeleteId(null);
    try {
      await deleteWorkflow(w.id);
      await refresh();
    } catch (e) {
      setActionError(`Failed to delete ${w.name}: ${e}`);
    } finally {
      setPendingAction(null);
    }
  };

  const handleEnqueue = async (w: Workflow) => {
    setActionError(null);
    setPendingEnqueueId(w.id);
    try {
      const outcome = await queueWorkflowRun(w.id);
      showRunNotice(formatWorkflowQueueOutcome(w.name, outcome));
      await refresh();
    } catch (e) {
      setActionError(formatWorkflowQueueError(w.name, e));
    } finally {
      setPendingEnqueueId(null);
    }
  };

  const toggleDescription = useCallback((wId: string, truncated: boolean) => {
    if (!truncated) return;
    setExpandedDescId((current) => (current === wId ? null : wId));
  }, []);

  if (loading && workflows.length === 0 && !error) {
    return <div className="wf-loading">Loading workflows...</div>;
  }

  if (error && workflows.length === 0) {
    return (
      <div>
        <PageHeader
          title="Workflows"
          subtitle="Could not load workflows"
          actions={
            <Button variant="primary" onClick={onNew}>
              + Add Workflow
            </Button>
          }
        />
        <div className="wf-error">
          <span>{error}</span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void refresh()}
            disabled={loading}
          >
            Retry
          </Button>
        </div>
      </div>
    );
  }

  const renderCard = (w: Workflow) => (
    <div key={w.id} className={`wf-card ${!w.enabled ? "disabled" : ""}`}>
      <div className="wf-card-header">
        <div className="wf-card-title-row">
          <button
            type="button"
            className="wf-card-title wf-card-title-btn"
            onClick={() => onOpen(w)}
            title={`Open ${w.name}`}
          >
            {w.name}
          </button>
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
            onChange={() => void handleToggle(w)}
            disabled={isPending(w, "toggle") || isPending(w, "delete")}
            aria-label={`${w.enabled ? "Disable" : "Enable"} ${w.name}`}
          />
          <span className="wf-toggle-track" aria-hidden="true" />
        </label>
      </div>
      {w.description && (
        <DescriptionBlock
          workflowId={w.id}
          description={w.description}
          expanded={expandedDescId === w.id}
          onToggle={(truncated) => toggleDescription(w.id, truncated)}
        />
      )}
      <div className="wf-card-meta">
        <span className="wf-card-schedule">
          <Clock size={12} strokeWidth={2} aria-hidden="true" />
          {cronToHuman(w.cron_schedule, w.timezone)}
        </span>
        <span className="wf-card-script">{w.script_path}</span>
      </div>
      <div className="wf-card-actions">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => void handleEnqueue(w)}
          disabled={
            isPending(w, "toggle") ||
            isPending(w, "delete") ||
            pendingEnqueueId === w.id
          }
          title="Queue through scheduler admission control"
          aria-label={`Queue run for ${w.name}`}
        >
          {pendingEnqueueId === w.id ? "Submitting…" : "Queue run"}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => onOpen(w)}
        >
          Details
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => onHistory(w)}
        >
          History
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => onEdit(w)}
        >
          Edit
        </Button>
        <Button
          type="button"
          size="sm"
          className={
            pendingDeleteId === w.id ? "btn-danger-confirm" : "btn-danger"
          }
          onClick={() => void handleDelete(w)}
          disabled={isPending(w, "toggle")}
        >
          {isPending(w, "delete")
            ? "Deleting…"
            : pendingDeleteId === w.id
              ? "Confirm?"
              : "Delete"}
        </Button>
      </div>
    </div>
  );

  return (
    <div>
      {runNotice && (
        <NoticeBanner
          message={runNotice}
          tone="success"
          onDismiss={() => setRunNotice(null)}
        />
      )}
      {actionError && (
        <NoticeBanner
          message={actionError}
          tone="error"
          onDismiss={() => setActionError(null)}
        />
      )}
      {error && workflows.length > 0 && (
        <NoticeBanner
          message={`Workflow list may be stale: ${error}`}
          tone="error"
          onDismiss={() => void refresh()}
        />
      )}
      <PageHeader
        title="Workflows"
        subtitle={
          <>
            {workflows.length} workflow{workflows.length !== 1 ? "s" : ""}{" "}
            configured
          </>
        }
        actions={
          <Button variant="primary" onClick={onNew}>
            + Add Workflow
          </Button>
        }
      />

      {workflows.length === 0 ? (
        <div className="wf-empty">
          <p className="wf-empty-title">No workflows yet</p>
          <p className="wf-empty-sub">
            Create your first workflow to start automating PM tasks.
          </p>
          <Button variant="primary" onClick={onNew}>
            + Add Workflow
          </Button>
        </div>
      ) : (
        <>
          <div
            className="wf-env-filter"
            role="group"
            aria-label="Filter workflows by environment"
          >
            <button
              className={`wf-env-pill ${envFilter === "all" ? "active" : ""}`}
              onClick={() => setEnvFilter("all")}
              aria-pressed={envFilter === "all"}
            >
              All
              <span>{workflows.length}</span>
            </button>
            {envOptions.map((name) => (
              <button
                key={name}
                className={`wf-env-pill ${envFilter === name ? "active" : ""}`}
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
