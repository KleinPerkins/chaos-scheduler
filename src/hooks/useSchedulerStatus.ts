import { useState, useEffect, useCallback } from "react";
import { getSchedulerStatus } from "../lib/commands";
import type { SchedulerStatus } from "../lib/commands";

export function useSchedulerStatus(pollInterval = 10000) {
  const [status, setStatus] = useState<SchedulerStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await getSchedulerStatus();
      setStatus(data);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, pollInterval);
    return () => clearInterval(id);
  }, [refresh, pollInterval]);

  return { status, error, refresh };
}
