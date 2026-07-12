import { afterEach, describe, expect, it, vi } from "vitest";
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

  it("renders the bounded Global History region and truthful filters", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_global_run_history: () => loadedRuns,
    };

    render(<GlobalHistory onViewRun={() => {}} />);

    expect(
      await screen.findByRole("heading", { name: "Global History" }),
    ).toBeInTheDocument();
    await screen.findByText("Nightly sync");
    expect(
      screen.getByRole("region", { name: "Global History" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Latest 100 indexed runs across workflows. Search filters loaded rows only.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("group", { name: "Run history filters" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Search loaded rows")).toBeInTheDocument();
    expect(screen.getByLabelText("Status")).toBeInTheDocument();
    expect(screen.getByLabelText("Environment")).toBeInTheDocument();
    expect(screen.getByLabelText("Trigger")).toBeInTheDocument();
    expect(screen.getByText("Latest 100")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeInTheDocument();
  });

  it("exposes the results as a named region and reports the loaded count", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_global_run_history: () => loadedRuns,
    };

    render(<GlobalHistory onViewRun={() => {}} />);

    await screen.findByText("Nightly sync");
    // The runs live inside a named region (aria-labelledby the "Latest runs"
    // heading) nested within the outer "Global History" surface region.
    expect(
      screen.getByRole("region", { name: "Latest runs" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(`${loadedRuns.length} loaded · newest first`),
    ).toBeInTheDocument();
    expect(screen.getByText("Ledger reconcile")).toBeInTheDocument();
    expect(screen.getByText("Cursor triage")).toBeInTheDocument();
  });

  it("drills into a run when its Details action is pressed", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_global_run_history: () => loadedRuns,
    };
    const onViewRun = vi.fn();

    render(<GlobalHistory onViewRun={onViewRun} />);

    await screen.findByText("Nightly sync");
    const detailButtons = screen.getAllByRole("button", { name: "Details" });
    expect(detailButtons).toHaveLength(loadedRuns.length);
    // Second row is "Ledger reconcile" (run-bravo).
    fireEvent.click(detailButtons[1]);

    expect(onViewRun).toHaveBeenCalledTimes(1);
    expect(onViewRun.mock.calls[0][0]).toMatchObject({ id: "run-bravo" });
  });

  it("re-queries the bounded backend window when the status filter changes", async () => {
    installStrictIpcMocks();
    const statusCalls: string[] = [];
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_global_run_history: (args) => {
        const status = String(args.statusFilter);
        statusCalls.push(status);
        // The status filter is BACKEND-scoped: the query re-runs and returns a
        // narrower set, unlike the client-side search refinement.
        return status === "failed" ? [loadedRuns[1]] : loadedRuns;
      },
    };

    render(<GlobalHistory onViewRun={() => {}} />);

    await screen.findByText("Nightly sync");
    expect(statusCalls).toEqual(["all"]);

    fireEvent.change(screen.getByLabelText("Status"), {
      target: { value: "failed" },
    });

    await waitFor(() => expect(statusCalls).toContain("failed"));
    await waitFor(() =>
      expect(screen.queryByText("Nightly sync")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Ledger reconcile")).toBeInTheDocument();
    expect(screen.getByText("1 loaded · newest first")).toBeInTheDocument();
  });

  it("announces a load failure as an alert", async () => {
    installStrictIpcMocks();
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_global_run_history: () => {
        throw new Error("db locked");
      },
    };

    render(<GlobalHistory onViewRun={() => {}} />);

    // A failed async load must be announced to assistive tech, not shown as a
    // silent div.
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(/History failed to load/i);
  });

  it("search refines only the loaded rows without refetching", async () => {
    installStrictIpcMocks();
    let fetchCount = 0;
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_global_run_history: () => {
        fetchCount += 1;
        return loadedRuns;
      },
    };

    render(<GlobalHistory onViewRun={() => {}} />);

    await screen.findByText("Nightly sync");
    expect(fetchCount).toBe(1);
    expect(screen.getByText("Ledger reconcile")).toBeInTheDocument();
    expect(screen.getByText("Cursor triage")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Search loaded rows"), {
      target: { value: "ledger" },
    });

    await waitFor(() =>
      expect(screen.queryByText("Nightly sync")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Ledger reconcile")).toBeInTheDocument();
    expect(screen.queryByText("Cursor triage")).not.toBeInTheDocument();
    expect(fetchCount).toBe(1);
  });

  it("refreshes the same bounded query on demand", async () => {
    installStrictIpcMocks();
    let fetchCount = 0;
    window.__CHAOS_IPC_OVERRIDES__ = {
      get_global_run_history: () => {
        fetchCount += 1;
        return loadedRuns;
      },
    };

    render(<GlobalHistory onViewRun={() => {}} />);

    await screen.findByText("Nightly sync");
    expect(fetchCount).toBe(1);
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(fetchCount).toBe(2));
  });
});
