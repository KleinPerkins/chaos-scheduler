import { useCallback, useEffect, useState } from "react";
import { listEnvironments } from "../lib/commands";
import type { Environment } from "../lib/commands";

/** Load the user-managed environments (Phase 3 backend). Degrades to an empty
 * list if the environments backend is unavailable so callers can fall back to
 * environment values observed on workflow rows. */
export function useEnvironments() {
  const [environments, setEnvironments] = useState<Environment[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const data = await listEnvironments();
      setEnvironments(data);
      setError(null);
    } catch (e) {
      setError(String(e));
      setEnvironments([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return { environments, loading, error, refresh };
}
