import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import StatusDot from "./StatusDot";

afterEach(cleanup);

function classOf(container: HTMLElement): string {
  return (container.firstChild as HTMLElement).className;
}

describe("StatusDot", () => {
  it("defaults to the `.status-dot` base plus the status modifier", () => {
    const { container } = render(<StatusDot status="succeeded" />);
    expect(classOf(container)).toBe("status-dot succeeded");
  });

  it("renders the `.mc-dot` variant base plus the status modifier", () => {
    const { container } = render(
      <StatusDot variant="mc-dot" status="success" />,
    );
    expect(classOf(container)).toBe("mc-dot success");
  });

  it("maps each status to its modifier class for both variants", () => {
    const { container, rerender } = render(<StatusDot status="running" />);
    expect(classOf(container)).toBe("status-dot running");
    rerender(<StatusDot status="failed" />);
    expect(classOf(container)).toBe("status-dot failed");
    rerender(<StatusDot status="dead_lettered" />);
    expect(classOf(container)).toBe("status-dot dead_lettered");
    rerender(<StatusDot variant="mc-dot" status="queued" />);
    expect(classOf(container)).toBe("mc-dot queued");
  });

  it("merges a passthrough className after the base + status", () => {
    const { container } = render(
      <StatusDot variant="mc-dot" status="success" className="extra" />,
    );
    expect(classOf(container)).toBe("mc-dot success extra");
  });

  it("is decorative by default and honors explicit accessible overrides", () => {
    const { container, rerender } = render(<StatusDot status="running" />);
    const dot = () => container.firstChild as HTMLElement;

    expect(dot()).toHaveAttribute("aria-hidden", "true");

    rerender(
      <StatusDot status="running" aria-label="Running" aria-hidden={true} />,
    );
    expect(dot()).toHaveAttribute("aria-label", "Running");
    expect(dot()).not.toHaveAttribute("aria-hidden");

    rerender(<StatusDot status="running" aria-hidden={false} />);
    expect(dot()).toHaveAttribute("aria-hidden", "false");
  });

  it("forwards native span attributes", () => {
    const { container } = render(<StatusDot status="running" title="live" />);
    expect(container.firstChild as HTMLElement).toHaveAttribute(
      "title",
      "live",
    );
  });
});
