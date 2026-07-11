import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Run } from "../lib/commands";
import RunsTable from "./RunsTable";

afterEach(cleanup);

function makeRun(overrides: Partial<Run> = {}): Run {
  return {
    id: "run-1",
    workflow_id: "wf-1234abcd",
    started_at: "2026-01-02T03:04:00Z",
    finished_at: "2026-01-02T03:06:00Z",
    exit_code: 0,
    stdout: null,
    stderr: null,
    result_url: null,
    status: "success",
    workflow_name: "nightly-refresh",
    trigger_kind: "cron",
    ...overrides,
  };
}

describe("RunsTable", () => {
  it("renders the `.rh-table` with the canonical column headers", () => {
    const { container } = render(
      <RunsTable runs={[makeRun()]} onViewRun={() => {}} />,
    );
    const table = container.querySelector("table");
    expect(table).not.toBeNull();
    expect(table!.className).toBe("rh-table");

    const headers = Array.from(table!.querySelectorAll("thead th")).map(
      (th) => th.textContent,
    );
    expect(headers).toEqual([
      "Status",
      "Workflow",
      "Started",
      "Trigger",
      "Exit Code",
      "Actions",
    ]);
    expect(screen.getByText("Actions")).toHaveClass("sr-only");
  });

  it("renders one row per run, composing StatusBadge and a Details Button", () => {
    const runs = [
      makeRun({ id: "a", status: "running", workflow_name: "alpha" }),
      makeRun({ id: "b", status: "failed", workflow_name: "beta" }),
    ];
    render(<RunsTable runs={runs} onViewRun={() => {}} />);

    // 1 header row + 2 body rows.
    expect(screen.getAllByRole("row")).toHaveLength(3);

    // StatusBadge is composed: span.status-badge.<status> with the human label.
    expect(screen.getByText("running")).toHaveClass("status-badge", "running");
    expect(screen.getByText("failed")).toHaveClass("status-badge", "failed");

    // Workflow-name cell per row.
    expect(screen.getByText("alpha")).toBeInTheDocument();
    expect(screen.getByText("beta")).toBeInTheDocument();

    // A ghost Details Button action per row.
    expect(screen.getAllByRole("button", { name: "Details" })).toHaveLength(2);
  });

  it("falls back to workflow_id, `cron` trigger, and an em-dash exit code", () => {
    render(
      <RunsTable
        runs={[
          makeRun({
            workflow_name: null,
            workflow_id: "wf-fallback",
            trigger_kind: null,
            exit_code: null,
          }),
        ]}
        onViewRun={() => {}}
      />,
    );
    expect(screen.getByText("wf-fallback")).toBeInTheDocument();
    expect(screen.getByText("cron")).toBeInTheDocument();
    expect(screen.getByText("—")).toBeInTheDocument();
  });

  it("fires onViewRun with the row's run when Details is clicked", () => {
    const onViewRun = vi.fn();
    const run = makeRun({ id: "clicked" });
    render(<RunsTable runs={[run]} onViewRun={onViewRun} />);
    fireEvent.click(screen.getByRole("button", { name: "Details" }));
    expect(onViewRun).toHaveBeenCalledTimes(1);
    expect(onViewRun).toHaveBeenCalledWith(run);
  });

  it("renders the empty state (default + custom label) with no runs", () => {
    const { container, rerender } = render(
      <RunsTable runs={[]} onViewRun={() => {}} />,
    );
    expect(container.querySelector("table")).toBeNull();
    const empty = container.firstChild as HTMLElement;
    expect(empty.className).toBe("rh-empty");
    expect(empty.textContent).toBe("No runs.");

    rerender(
      <RunsTable
        runs={[]}
        onViewRun={() => {}}
        emptyLabel="No runs match these filters."
      />,
    );
    expect(screen.getByText("No runs match these filters.")).toHaveClass(
      "rh-empty",
    );
  });
});
