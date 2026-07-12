/**
 * Standard run/task duration formatting. One shared ladder so every surface
 * renders elapsed times identically (each screen previously carried its own
 * partial copy of this logic):
 *
 *   <60s → `#s`     (e.g. `42s`)
 *   <60m → `#m #s`  (e.g. `3m 7s`)
 *   <24h → `#h #m`  (e.g. `2h 5m`)
 *   ≥24h → `#d #h`  (e.g. `1d 3h`)
 *
 * Values are floored within a tier (never rounded up); sub-second and negative
 * inputs clamp to `0s`.
 *
 * @param ms elapsed time in milliseconds
 */
export function formatDuration(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  if (totalSeconds < 60) return `${totalSeconds}s`;
  const totalMinutes = Math.floor(totalSeconds / 60);
  if (totalMinutes < 60) return `${totalMinutes}m ${totalSeconds % 60}s`;
  const totalHours = Math.floor(totalMinutes / 60);
  if (totalHours < 24) return `${totalHours}h ${totalMinutes % 60}m`;
  const days = Math.floor(totalHours / 24);
  return `${days}d ${totalHours % 24}h`;
}

/**
 * Convenience wrapper: format the elapsed time between two ISO-8601 timestamps
 * with {@link formatDuration}. Callers own the still-running case (a null end),
 * so their existing placeholder text is preserved at the call site.
 */
export function formatDurationBetween(
  startIso: string,
  endIso: string,
): string {
  return formatDuration(
    new Date(endIso).getTime() - new Date(startIso).getTime(),
  );
}
