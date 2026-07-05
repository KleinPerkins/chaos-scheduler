const RUN_STATUS_LABELS: Record<string, string> = {
  poll_exhausted: "poll exhausted",
  timed_out: "timed out",
};

/** Human-readable label for a run/task status code (badge CSS uppercases). */
export function formatRunStatusLabel(status: string): string {
  return RUN_STATUS_LABELS[status] ?? status.replace(/_/g, " ");
}
