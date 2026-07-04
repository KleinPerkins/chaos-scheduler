import { useState, useEffect, useCallback } from "react";
import { listWorkflows } from "../lib/commands";
import type { Workflow } from "../lib/commands";

export function useWorkflows() {
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      const data = await listWorkflows();
      setWorkflows(data);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  // Defer the initial load to a macrotask so the fetch's synchronous
  // setLoading(true) does not run inside the effect body (avoids the
  // cascading-render pattern flagged by react-hooks/set-state-in-effect).
  // Mirrors the established pattern in useSchedulerStatus.
  useEffect(() => {
    const id = setTimeout(() => void refresh(), 0);
    return () => clearTimeout(id);
  }, [refresh]);

  return { workflows, loading, error, refresh };
}
