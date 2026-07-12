import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import WorkflowCard from "./WorkflowCard";

const callbacks = {
  onOpen: vi.fn(),
  onQueue: vi.fn(),
  onToggleEnabled: vi.fn(),
  onHistory: vi.fn(),
  onEdit: vi.fn(),
  onDelete: vi.fn(),
};

function renderCard(
  overrides: Partial<React.ComponentProps<typeof WorkflowCard>> = {},
) {
  return render(
    <WorkflowCard
      name="nightly-refresh"
      environment="production"
      schedule="Daily at 02:00 · America/Los_Angeles"
      description="Refreshes analytics tables."
      enabled
      activity="none"
      {...callbacks}
      {...overrides}
    />,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("WorkflowCard", () => {
  it("keeps configuration state separate from observed queue activity", () => {
    const { rerender } = renderCard();
    expect(screen.getByText("Enabled")).toBeInTheDocument();
    expect(screen.queryByText(/Idle|Running/)).not.toBeInTheDocument();

    rerender(
      <WorkflowCard
        name="nightly-refresh"
        environment="production"
        schedule="Daily at 02:00 · America/Los_Angeles"
        description="Refreshes analytics tables."
        enabled
        activity="waiting"
        {...callbacks}
      />,
    );
    expect(screen.getByText("Enabled · Waiting to start")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Waiting…" })).toBeDisabled();
  });

  it("shows local submission progress without claiming the run started", () => {
    renderCard({ activity: "submitting" });
    expect(
      screen.getByText("Enabled · Submitting request…"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Submitting…" })).toBeDisabled();
    expect(screen.queryByText(/Running/)).not.toBeInTheDocument();
  });

  it("prioritizes re-enabling a disabled schedule while preserving manual queue access", () => {
    renderCard({ enabled: false });

    fireEvent.click(screen.getByRole("button", { name: "Enable scheduling" }));
    expect(callbacks.onToggleEnabled).toHaveBeenCalled();
    expect(
      screen.queryByRole("button", { name: "Queue run" }),
    ).not.toBeInTheDocument();

    fireEvent.click(
      screen.getByText("More actions", {
        selector: "summary .sr-only",
      }),
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: "Queue run for nightly-refresh",
      }),
    );
    expect(callbacks.onQueue).toHaveBeenCalled();
  });

  it("preserves details, history, edit, scheduling, and guarded delete actions", () => {
    renderCard();

    fireEvent.click(screen.getByRole("button", { name: "View details" }));
    expect(callbacks.onOpen).toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole("button", {
        name: "Queue run for nightly-refresh",
      }),
    );
    expect(callbacks.onQueue).toHaveBeenCalled();

    fireEvent.click(
      screen.getByText("More actions", {
        selector: "summary .sr-only",
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "View history" }));
    fireEvent.click(screen.getByRole("button", { name: "Edit workflow" }));
    fireEvent.click(screen.getByRole("button", { name: "Disable scheduling" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete workflow" }));

    expect(callbacks.onHistory).toHaveBeenCalled();
    expect(callbacks.onEdit).toHaveBeenCalled();
    expect(callbacks.onToggleEnabled).toHaveBeenCalled();
    expect(callbacks.onDelete).toHaveBeenCalled();
  });
});
