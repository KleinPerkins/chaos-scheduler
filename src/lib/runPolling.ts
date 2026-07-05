/** Run statuses that may still change while the scheduler executes work. */
const ACTIVE_RUN_STATUSES = new Set(["running", "queued", "admitted"]);

export function isActiveRunStatus(status: string): boolean {
  return ACTIVE_RUN_STATUSES.has(status);
}

/** Exponential backoff for live run polling (2s base, 30s cap). */
export function nextPollDelayMs(
  attempt: number,
  baseMs = 2000,
  maxMs = 30000,
): number {
  return Math.min(baseMs * 1.5 ** attempt, maxMs);
}
