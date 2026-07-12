import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { useWorkflows } from "../hooks/useWorkflows";
import { useEnvironments } from "../hooks/useEnvironments";
import {
  updateWorkflow,
  deleteWorkflow,
  environmentOf,
  listQueuedRuns,
} from "../lib/commands";
import type { Workflow } from "../lib/commands";
import { cronToHuman } from "./ScheduleBuilder";
import NoticeBanner from "./NoticeBanner";
import Button from "./Button";
import PageHeader from "./PageHeader";
import Select from "./Select";
import WorkflowCard from "./WorkflowCard";
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

type EnvFilter = string; // "all" or an environment name
type StatusFilter = "all" | "enabled" | "disabled";

export default function WorkflowList({
  onOpen,
  onEdit,
  onNew,
  onHistory,
}: Props) {
  const { workflows, loading, error, refresh } = useWorkflows();
  const { environments } = useEnvironments();
  const [envFilter, setEnvFilter] = useState<EnvFilter>("all");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [query, setQuery] = useState("");
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<{
    id: string;
    kind: "toggle" | "delete";
  } | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [runNotice, setRunNotice] = useState<string | null>(null);
  const [pendingEnqueueId, setPendingEnqueueId] = useState<string | null>(null);
  const [waitingWorkflowIds, setWaitingWorkflowIds] = useState<Set<string>>(
    () => new Set(),
  );
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

  const refreshQueueActivity = useCallback(async () => {
    try {
      const rows = await listQueuedRuns(50);
      setWaitingWorkflowIds(
        new Set(
          rows
            .filter((row) => row.status.toLowerCase() === "queued")
            .map((row) => row.workflow_id),
        ),
      );
    } catch {
      // Queue activity is positive-only enhancement data. An unavailable
      // snapshot must never invent an idle/running state or block the list.
    }
  }, []);

  useEffect(() => {
    const initial = setTimeout(() => void refreshQueueActivity(), 0);
    const interval = setInterval(() => void refreshQueueActivity(), 15_000);
    return () => {
      clearTimeout(initial);
      clearInterval(interval);
    };
  }, [refreshQueueActivity]);

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
    const normalizedQuery = query.trim().toLowerCase();
    return workflows.filter((workflow) => {
      if (envFilter !== "all" && environmentOf(workflow) !== envFilter) {
        return false;
      }
      if (
        statusFilter !== "all" &&
        workflow.enabled !== (statusFilter === "enabled")
      ) {
        return false;
      }
      if (!normalizedQuery) return true;
      return [
        workflow.name,
        workflow.description,
        workflow.script_path,
        workflow.cron_schedule,
        environmentOf(workflow),
        workflow.kind,
      ]
        .filter(Boolean)
        .some((value) => String(value).toLowerCase().includes(normalizedQuery));
    });
  }, [workflows, envFilter, statusFilter, query]);

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
      if (
        outcome.status === "queued" ||
        (outcome.status === "duplicate" && outcome.queued_run_id)
      ) {
        setWaitingWorkflowIds((current) => new Set(current).add(w.id));
      } else {
        setWaitingWorkflowIds((current) => {
          const next = new Set(current);
          next.delete(w.id);
          return next;
        });
      }
      await refresh();
    } catch (e) {
      setActionError(formatWorkflowQueueError(w.name, e));
    } finally {
      setPendingEnqueueId(null);
    }
  };

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

  const renderCard = (workflow: Workflow) => {
    const activity =
      pendingEnqueueId === workflow.id
        ? "submitting"
        : waitingWorkflowIds.has(workflow.id)
          ? "waiting"
          : "none";
    const deleteState = isPending(workflow, "delete")
      ? "deleting"
      : pendingDeleteId === workflow.id
        ? "armed"
        : "idle";

    return (
      <WorkflowCard
        key={workflow.id}
        name={workflow.name}
        environment={environmentOf(workflow)}
        schedule={`${cronToHuman(workflow.cron_schedule)} · ${workflow.timezone}`}
        description={workflow.description}
        enabled={workflow.enabled}
        activity={activity}
        managedExternally={workflow.managed_externally}
        actionBusy={
          pendingAction?.id === workflow.id || pendingEnqueueId === workflow.id
        }
        deleteState={deleteState}
        onOpen={() => onOpen(workflow)}
        onQueue={() => void handleEnqueue(workflow)}
        onToggleEnabled={() => void handleToggle(workflow)}
        onHistory={() => onHistory(workflow)}
        onEdit={() => onEdit(workflow)}
        onDelete={() => void handleDelete(workflow)}
      />
    );
  };

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
        subtitle="Search, filter, and manage registered schedules. Manual execution always enters scheduler admission control."
        actions={
          <Button variant="primary" className="wf-add-button" onClick={onNew}>
            Add workflow
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
          <div className="wf-filter-bar">
            <label className="sr-only" htmlFor="workflow-search">
              Search workflows
            </label>
            <input
              id="workflow-search"
              type="search"
              value={query}
              placeholder="Search workflows…"
              onChange={(event) => setQuery(event.target.value)}
            />

            <label className="sr-only" htmlFor="workflow-environment-filter">
              Environment
            </label>
            <Select
              id="workflow-environment-filter"
              value={envFilter}
              onChange={(event) => setEnvFilter(event.target.value)}
            >
              <option value="all">All environments</option>
              {envOptions.map((name) => (
                <option key={name} value={name}>
                  {name.charAt(0).toUpperCase() + name.slice(1)} (
                  {envCounts.get(name) ?? 0})
                </option>
              ))}
            </Select>

            <label className="sr-only" htmlFor="workflow-status-filter">
              Status
            </label>
            <Select
              id="workflow-status-filter"
              value={statusFilter}
              onChange={(event) =>
                setStatusFilter(event.target.value as StatusFilter)
              }
            >
              <option value="all">All statuses</option>
              <option value="enabled">Enabled</option>
              <option value="disabled">Disabled</option>
            </Select>
          </div>
          <p className="wf-result-count" aria-live="polite">
            {visibleWorkflows.length} workflow
            {visibleWorkflows.length !== 1 ? "s" : ""} · flat results
          </p>

          {visibleWorkflows.length === 0 ? (
            <div className="wf-empty wf-empty-compact">
              <p className="wf-empty-title">No workflows match these filters</p>
              <p className="wf-empty-sub">
                Clear search or choose a different environment or status.
              </p>
            </div>
          ) : (
            <div className="wf-grid">{visibleWorkflows.map(renderCard)}</div>
          )}
        </>
      )}
    </div>
  );
}
