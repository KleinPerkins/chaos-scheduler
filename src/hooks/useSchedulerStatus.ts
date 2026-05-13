import { useState, useEffect, useCallback, useRef } from "react";
import { getSchedulerStatus } from "../lib/commands";
import type { SchedulerStatus } from "../lib/commands";

export function useSchedulerStatus(pollInterval = 10000) {
  const [status, setStatus] = useState<SchedulerStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const latestRequestId = useRef(0);

  const refresh = useCallback(async () => {
    const requestId = latestRequestId.current + 1;
    latestRequestId.current = requestId;
    try {
      const data = await getSchedulerStatus();
      if (requestId !== latestRequestId.current) return;
      setStatus(data);
      setError(null);
    } catch (e) {
      if (requestId !== latestRequestId.current) return;
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    const initial = setTimeout(refresh, 0);
    const id = setInterval(refresh, pollInterval);
    return () => {
      clearTimeout(initial);
      clearInterval(id);
    };
  }, [refresh, pollInterval]);

  return { status, error, refresh };
}
