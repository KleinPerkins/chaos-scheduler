const RUN_STATUS_LABELS: Record<string, string> = {
  poll_exhausted: "poll exhausted",
  timed_out: "timed out",
};

/** Human-readable label for a run/task status code (badge CSS uppercases). */
export function formatRunStatusLabel(status: string): string {
  return RUN_STATUS_LABELS[status] ?? status.replace(/_/g, " ");
}

/**
 * Canonical status key for color/shape selection: collapses the `succeeded`
 * alias onto `success` so both render the same status-tier styling. Every other
 * status token passes through unchanged. Shared by Mission Control's status
 * dots and badges so the alias mapping lives in one place. Idempotent.
 */
export function statusKey(status: string): string {
  return status === "success" || status === "succeeded" ? "success" : status;
}
