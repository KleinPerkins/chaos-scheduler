import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import StatusBadge from "./StatusBadge";

afterEach(cleanup);

describe("StatusBadge", () => {
  it("renders the base class plus the status modifier", () => {
    render(<StatusBadge status="succeeded">Succeeded</StatusBadge>);
    expect(screen.getByText("Succeeded")).toHaveClass(
      "status-badge",
      "succeeded",
    );
  });

  it("maps each canonical status to its modifier class", () => {
    const { rerender } = render(<StatusBadge status="running">R</StatusBadge>);
    expect(screen.getByText("R")).toHaveClass("status-badge", "running");
    rerender(<StatusBadge status="failed">F</StatusBadge>);
    expect(screen.getByText("F")).toHaveClass("status-badge", "failed");
    rerender(<StatusBadge status="poll_exhausted">W</StatusBadge>);
    expect(screen.getByText("W")).toHaveClass("status-badge", "poll_exhausted");
  });

  it("renders children as the label", () => {
    render(<StatusBadge status="queued">Queued</StatusBadge>);
    expect(screen.getByText("Queued")).toBeInTheDocument();
  });

  it("merges a passthrough className after the status modifier", () => {
    render(
      <StatusBadge status="succeeded" className="mc-badge">
        S
      </StatusBadge>,
    );
    expect(screen.getByText("S").className).toBe(
      "status-badge succeeded mc-badge",
    );
  });

  it("forwards native span attributes", () => {
    render(
      <StatusBadge status="running" title="live">
        Running
      </StatusBadge>,
    );
    expect(screen.getByText("Running")).toHaveAttribute("title", "live");
  });
});
