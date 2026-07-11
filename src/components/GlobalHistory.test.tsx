import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import GlobalHistory from "./GlobalHistory";
import { sampleRun } from "../test/fixtures/data";
import {
  createDefaultIpcRegistry,
  resolveIpcInvoke,
} from "../test/fixtures/ipc-registry";
import type { Run } from "../lib/commands";

function installStrictIpcMocks(): void {
  const registry = createDefaultIpcRegistry();
  mockIPC(
    (cmd, args) =>
      resolveIpcInvoke(cmd, (args ?? {}) as Record<string, unknown>, registry),
    { shouldMockEvents: true },
  );
}

const loadedRuns: Run[] = [
  { ...sampleRun, id: "run-alpha", workflow_name: "Nightly sync" },
  { ...sampleRun, id: "run-bravo", workflow_name: "Ledger reconcile" },
  { ...sampleRun, id: "run-charlie", workflow_name: "Cursor triage" },
];

describe("GlobalHistory", () => {
  afterEach(() => {
    cleanup();
    clearMocks();
    delete window.__CHAOS_IPC_OVERRIDES__;
  });

  it("renders the History surface with a bounded latest-100 label", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_global_run_history: () => loadedRuns,
    };

    render(<GlobalHistory onViewRun={() => {}} />);

    expect(
      await screen.findByRole("heading", { name: "History" }),
    ).toBeInTheDocument();
    await screen.findByText("Nightly sync");
    expect(screen.getByText("Latest 100")).toBeInTheDocument();
  });

  it("search refines only the loaded rows without refetching", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_global_run_history: () => loadedRuns,
    };

    render(<GlobalHistory onViewRun={() => {}} />);

    await screen.findByText("Nightly sync");
    expect(screen.getByText("Ledger reconcile")).toBeInTheDocument();
    expect(screen.getByText("Cursor triage")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Search loaded runs"), {
      target: { value: "ledger" },
    });

    await waitFor(() =>
      expect(screen.queryByText("Nightly sync")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Ledger reconcile")).toBeInTheDocument();
    expect(screen.queryByText("Cursor triage")).not.toBeInTheDocument();
  });
});
