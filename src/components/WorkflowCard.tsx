import { useId } from "react";
import Button from "./Button";
import EnvironmentBadge from "./EnvironmentBadge";
import StatusDot from "./StatusDot";
import "./WorkflowCard.css";

export type WorkflowActivity = "none" | "submitting" | "waiting";
export type WorkflowDeleteState = "idle" | "armed" | "deleting";

export interface WorkflowCardProps {
  name: string;
  environment: string;
  schedule: string;
  description?: string | null;
  enabled: boolean;
  activity: WorkflowActivity;
  managedExternally?: boolean;
  actionBusy?: boolean;
  deleteState?: WorkflowDeleteState;
  onOpen: () => void;
  onQueue: () => void;
  onToggleEnabled: () => void;
  onHistory: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

function stateLabel(enabled: boolean, activity: WorkflowActivity): string {
  if (!enabled) return "Disabled";
  if (activity === "submitting") return "Enabled · Submitting request…";
  if (activity === "waiting") return "Enabled · Waiting to start";
  return "Enabled";
}

export default function WorkflowCard({
  name,
  environment,
  schedule,
  description,
  enabled,
  activity,
  managedExternally = false,
  actionBusy = false,
  deleteState = "idle",
  onOpen,
  onQueue,
  onToggleEnabled,
  onHistory,
  onEdit,
  onDelete,
}: WorkflowCardProps) {
  const titleId = useId();
  const queueBusy = activity !== "none";
  const status =
    activity === "submitting" || activity === "waiting"
      ? "running"
      : enabled
        ? "success"
        : "disabled";

  return (
    <article className="workflow-card" aria-labelledby={titleId}>
      <button
        type="button"
        className="workflow-card-title"
        id={titleId}
        onClick={onOpen}
      >
        {name}
      </button>

      <div className={`workflow-card-status workflow-card-status--${status}`}>
        <StatusDot status={status} />
        <span>{stateLabel(enabled, activity)}</span>
      </div>

      <EnvironmentBadge
        environment={environment}
        managed={managedExternally}
        size="sm"
      />
      <p className="workflow-card-schedule">{schedule}</p>
      <p className="workflow-card-description" title={description ?? undefined}>
        {description || "No description provided."}
      </p>

      <div className="workflow-card-footer">
        <Button type="button" variant="ghost" size="sm" onClick={onOpen}>
          View details
        </Button>
        <div className="workflow-card-actions">
          {enabled ? (
            <Button
              type="button"
              variant={queueBusy ? "ghost" : "primary"}
              size="sm"
              onClick={onQueue}
              disabled={actionBusy || queueBusy}
              aria-label={
                activity === "none" ? `Queue run for ${name}` : undefined
              }
            >
              {activity === "submitting"
                ? "Submitting…"
                : activity === "waiting"
                  ? "Waiting…"
                  : "Queue run"}
            </Button>
          ) : (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={onToggleEnabled}
              disabled={actionBusy}
            >
              Enable scheduling
            </Button>
          )}

          <details className="workflow-card-more">
            <summary title={`More actions for ${name}`}>
              <span aria-hidden="true">•••</span>
              <span className="sr-only">More actions</span>
            </summary>
            <div
              className="workflow-card-menu"
              role="group"
              aria-label={`More actions for ${name}`}
            >
              {!enabled && (
                <button
                  type="button"
                  onClick={onQueue}
                  disabled={actionBusy || queueBusy}
                  aria-label={`Queue run for ${name}`}
                >
                  Queue run
                </button>
              )}
              <button type="button" onClick={onHistory}>
                View history
              </button>
              <button
                type="button"
                onClick={onEdit}
                disabled={managedExternally}
                title={
                  managedExternally
                    ? "Externally managed — read-only"
                    : undefined
                }
              >
                Edit workflow
              </button>
              {enabled && (
                <button
                  type="button"
                  onClick={onToggleEnabled}
                  disabled={actionBusy}
                >
                  Disable scheduling
                </button>
              )}
              <button
                type="button"
                className="workflow-card-delete"
                onClick={onDelete}
                disabled={actionBusy}
              >
                {deleteState === "deleting"
                  ? "Deleting…"
                  : deleteState === "armed"
                    ? "Confirm delete"
                    : "Delete workflow"}
              </button>
            </div>
          </details>
        </div>
      </div>
    </article>
  );
}
