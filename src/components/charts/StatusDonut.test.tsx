import { afterEach, describe, it, expect } from "vitest";
import { cleanup, render } from "@testing-library/react";
import StatusDonut, { type StatusDonutSegment } from "./StatusDonut";

afterEach(cleanup);

const SEGMENTS: StatusDonutSegment[] = [
  { label: "Succeeded", value: 1024, color: "var(--success)" },
  { label: "Running", value: 96, color: "var(--running)" },
  { label: "Warning", value: 108, color: "var(--warning)" },
  { label: "Failed", value: 56, color: "var(--error)" },
];

function circles(container: HTMLElement): SVGCircleElement[] {
  return Array.from(container.querySelectorAll("circle"));
}

describe("StatusDonut", () => {
  it("shows the total and sub-label in the center", () => {
    const { getByText } = render(
      <StatusDonut segments={SEGMENTS} centerLabel="runs" />,
    );
    // 1024 + 96 + 108 + 56 = 1284 → locale-formatted
    expect(getByText("1,284")).toBeInTheDocument();
    expect(getByText("runs")).toBeInTheDocument();
  });

  it("renders one arc per segment with its color", () => {
    const { container } = render(<StatusDonut segments={SEGMENTS} />);
    const arcs = circles(container);
    expect(arcs).toHaveLength(SEGMENTS.length);
    expect(arcs[0].getAttribute("style")).toContain("var(--success)");
    expect(arcs[3].getAttribute("style")).toContain("var(--error)");
  });

  it("builds an accessible summary from the segments", () => {
    const { getByRole } = render(
      <StatusDonut segments={SEGMENTS} centerLabel="runs" />,
    );
    expect(getByRole("img").getAttribute("aria-label")).toBe(
      "1,284 runs: 1024 Succeeded, 96 Running, 108 Warning, 56 Failed",
    );
  });

  it("falls back to a single track ring and 0 when empty", () => {
    const empty: StatusDonutSegment[] = [
      { label: "Succeeded", value: 0, color: "var(--success)" },
      { label: "Failed", value: 0, color: "var(--error)" },
    ];
    const { container, getByText, getByRole } = render(
      <StatusDonut segments={empty} />,
    );
    expect(circles(container)).toHaveLength(1);
    expect(getByText("0")).toBeInTheDocument();
    expect(getByRole("img").getAttribute("aria-label")).toBe("No data");
  });

  it("honors a centerValue override", () => {
    const { getByText } = render(
      <StatusDonut segments={SEGMENTS} centerValue="1.3k" centerLabel="runs" />,
    );
    expect(getByText("1.3k")).toBeInTheDocument();
  });
});
