import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import StatusBar, { type StatusBarSegment } from "./StatusBar";

afterEach(cleanup);

const SEGMENTS: StatusBarSegment[] = [
  { status: "succeeded", label: "Succeeded", count: 210 },
  { status: "running", label: "Running", count: 24 },
  { status: "failed", label: "Failed", count: 24 },
];

function barOf(container: HTMLElement): HTMLElement {
  return container.firstChild as HTMLElement;
}

describe("StatusBar", () => {
  it("renders a `.status-bar` wrapping a `.status-bar-track` and a legend", () => {
    const { container } = render(<StatusBar segments={SEGMENTS} />);
    const bar = barOf(container);
    expect(bar.className).toBe("status-bar");
    expect(bar.querySelector(".status-bar-track")).not.toBeNull();
    expect(bar.querySelector(".status-bar-legend")).not.toBeNull();
  });

  it("renders one bar segment per non-zero count, colored by status", () => {
    const { container } = render(<StatusBar segments={SEGMENTS} />);
    const segs = container.querySelectorAll(
      ".status-bar-track > .status-bar-seg",
    );
    expect(segs).toHaveLength(3);
    expect(segs[0].className).toBe("status-bar-seg succeeded");
    expect(segs[1].className).toBe("status-bar-seg running");
    expect(segs[2].className).toBe("status-bar-seg failed");
  });

  it("sizes each segment proportionally to its share of the total", () => {
    const { container } = render(
      <StatusBar
        segments={[
          { status: "succeeded", label: "Succeeded", count: 3 },
          { status: "failed", label: "Failed", count: 1 },
        ]}
      />,
    );
    const segs = container.querySelectorAll<HTMLElement>(".status-bar-seg");
    expect(segs[0].style.width).toBe("75%");
    expect(segs[1].style.width).toBe("25%");
  });

  it("omits zero-count segments from the bar but keeps them in the legend", () => {
    const { container } = render(
      <StatusBar
        segments={[
          { status: "succeeded", label: "Succeeded", count: 5 },
          { status: "running", label: "Running", count: 0 },
          { status: "failed", label: "Failed", count: 0 },
        ]}
      />,
    );
    // Only the succeeded segment draws in the bar (and fills it entirely).
    const segs = container.querySelectorAll<HTMLElement>(".status-bar-seg");
    expect(segs).toHaveLength(1);
    expect(segs[0].className).toBe("status-bar-seg succeeded");
    expect(segs[0].style.width).toBe("100%");
    // The legend still lists all three provided statuses.
    expect(container.querySelectorAll(".status-bar-legend-item")).toHaveLength(
      3,
    );
  });

  it("renders decorative legend dots colored by status with the label text", () => {
    const { container } = render(<StatusBar segments={SEGMENTS} />);
    const items = container.querySelectorAll(".status-bar-legend-item");
    expect(items[0].textContent).toBe("Succeeded");
    const dot = items[0].querySelector(".status-bar-dot");
    expect(dot).toHaveClass("status-bar-dot", "succeeded");
    expect(dot).toHaveAttribute("aria-hidden", "true");
  });

  it("summarizes the non-zero segments as the track's accessible label", () => {
    render(<StatusBar segments={SEGMENTS} />);
    expect(screen.getByRole("img")).toHaveAttribute(
      "aria-label",
      "210 Succeeded, 24 Running, 24 Failed",
    );
  });

  it("labels an all-zero (empty) bar as `No runs` and draws no segments", () => {
    const { container } = render(
      <StatusBar
        segments={[{ status: "succeeded", label: "Succeeded", count: 0 }]}
      />,
    );
    expect(screen.getByRole("img")).toHaveAttribute("aria-label", "No runs");
    expect(container.querySelectorAll(".status-bar-seg")).toHaveLength(0);
  });

  it("hides the legend when `showLegend` is false", () => {
    const { container } = render(
      <StatusBar segments={SEGMENTS} showLegend={false} />,
    );
    expect(container.querySelector(".status-bar-legend")).toBeNull();
    // The bar itself still renders.
    expect(container.querySelector(".status-bar-track")).not.toBeNull();
  });

  it("merges a passthrough className and forwards native div attributes", () => {
    const { container } = render(
      <StatusBar segments={SEGMENTS} className="mc-statusbar" title="runs" />,
    );
    const bar = barOf(container);
    expect(bar.className).toBe("status-bar mc-statusbar");
    expect(bar).toHaveAttribute("title", "runs");
  });
});
