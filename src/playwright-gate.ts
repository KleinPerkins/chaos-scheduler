import { mockIPC } from "@tauri-apps/api/mocks";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "./test/fixtures/ipc-registry";

/**
 * Installs strict Tauri IPC mocks when `VITE_PLAYWRIGHT=true`. Playwright tests
 * may set `window.__CHAOS_IPC_OVERRIDES__` via `addInitScript` before navigation.
 */
if (import.meta.env.VITE_PLAYWRIGHT === "true") {
  const registry = createDefaultIpcRegistry();
  mockIPC(
    (cmd, args) =>
      resolveIpcInvoke(cmd, (args ?? {}) as Record<string, unknown>, registry),
    { shouldMockEvents: true },
  );
}
