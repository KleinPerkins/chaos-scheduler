import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  applyUpdate,
  checkForUpdate,
  getAppUpdateStatus,
  isCommandUnavailable,
  setUpdaterPreferences,
} from "../lib/commands";
import type { UpdateSnapshot, UpdaterPreferencesPatch } from "../lib/commands";

const UPDATE_STATUS_EVENT = "update-status";

/**
 * Single frontend entry point for the updater snapshot (updater UX plan,
 * Section 1). Hydrates from `get_app_update_status` on mount — a no-network
 * read of whatever the background task last observed — then subscribes to
 * the `update-status` event Rust emits on every check/download/error
 * transition, so this hook (and every component that uses it) always
 * reflects the one Rust-owned snapshot without polling.
 */
export function useAppUpdate() {
  const [snapshot, setSnapshot] = useState<UpdateSnapshot | null>(null);
  const [unavailable, setUnavailable] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const status = await getAppUpdateStatus();
      setSnapshot(status);
      setUnavailable(false);
      return status;
    } catch (e) {
      if (isCommandUnavailable(e)) {
        setUnavailable(true);
      }
      return null;
    }
  }, []);

  // Deferred to a macrotask so the initial fetch's `setSnapshot` does not
  // run synchronously inside the effect body (mirrors useSchedulerStatus /
  // useWorkflows).
  useEffect(() => {
    const id = setTimeout(() => void refresh(), 0);
    return () => clearTimeout(id);
  }, [refresh]);

  useEffect(() => {
    const unlisten = listen<UpdateSnapshot>(UPDATE_STATUS_EVENT, (event) => {
      setSnapshot(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const install = useCallback((expectedVersion?: string) => {
    return applyUpdate(expectedVersion);
  }, []);

  /**
   * Triggers a real, manual on-demand check (unlike `refresh`, which only
   * re-reads whatever the backend last observed with no network round-trip).
   * `check_for_update` broadcasts its result via the `update-status` event
   * before it resolves, so `refresh()` afterwards is a defensive re-read
   * rather than the primary update path.
   */
  const checkNow = useCallback(async () => {
    await checkForUpdate();
    return refresh();
  }, [refresh]);

  const setBackgroundCheckEnabled = useCallback(async (enabled: boolean) => {
    const next = await setUpdaterPreferences({
      backgroundCheckEnabled: enabled,
    });
    setSnapshot(next);
    return next;
  }, []);

  const skipVersion = useCallback(async (version: string) => {
    const next = await setUpdaterPreferences({ skippedVersion: version });
    setSnapshot(next);
    return next;
  }, []);

  const clearSkippedVersion = useCallback(async () => {
    const next = await setUpdaterPreferences({ clearSkip: true });
    setSnapshot(next);
    return next;
  }, []);

  const setPreferences = useCallback(async (patch: UpdaterPreferencesPatch) => {
    const next = await setUpdaterPreferences(patch);
    setSnapshot(next);
    return next;
  }, []);

  return {
    snapshot,
    unavailable,
    refresh,
    checkNow,
    install,
    setBackgroundCheckEnabled,
    skipVersion,
    clearSkippedVersion,
    setPreferences,
  };
}
