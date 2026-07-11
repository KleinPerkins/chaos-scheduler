import { afterEach, describe, it, expect } from "vitest";
import { cleanup, render } from "@testing-library/react";
import Gauge from "./Gauge";

afterEach(cleanup);

function paths(container: HTMLElement): SVGPathElement[] {
  return Array.from(container.querySelectorAll("path"));
}

describe("Gauge", () => {
  it("renders an accessible summary and the center labels", () => {
    const { getByRole, getByText } = render(<Gauge value={5} max={8} />);
    const svg = getByRole("img");
    expect(svg.getAttribute("aria-label")).toBe("63% utilized — 5 of 8 slots");
    // 5 / 8 = 62.5% → rounds to 63%
    expect(getByText("63%")).toBeInTheDocument();
    expect(getByText("5 of 8 slots")).toBeInTheDocument();
  });

  it("draws a track path plus a value arc when value > 0", () => {
    const { container } = render(<Gauge value={5} max={8} />);
    expect(paths(container)).toHaveLength(2);
  });

  it("omits the value arc at zero and reports 0%", () => {
    const { container, getByRole, getByText } = render(
      <Gauge value={0} max={8} />,
    );
    expect(paths(container)).toHaveLength(1);
    expect(getByText("0%")).toBeInTheDocument();
    expect(getByRole("img").getAttribute("aria-label")).toBe(
      "0% utilized — 0 of 8 slots",
    );
  });

  it("colors the value arc green below the warning threshold", () => {
    const { container } = render(<Gauge value={60} max={100} />);
    const valueArc = paths(container)[1];
    expect(valueArc.getAttribute("style")).toContain("var(--success)");
  });

  it("colors the value arc amber in the warning band", () => {
    const { container } = render(<Gauge value={80} max={100} />);
    const valueArc = paths(container)[1];
    expect(valueArc.getAttribute("style")).toContain("var(--warning)");
  });

  it("colors the value arc red at/above the danger threshold", () => {
    const { container } = render(<Gauge value={95} max={100} />);
    const valueArc = paths(container)[1];
    expect(valueArc.getAttribute("style")).toContain("var(--error)");
  });

  it("honors a valueColor override over the threshold ramp", () => {
    const { container } = render(
      <Gauge value={95} max={100} valueColor="var(--chart-1)" />,
    );
    const valueArc = paths(container)[1];
    expect(valueArc.getAttribute("style")).toContain("var(--chart-1)");
  });

  it("supports a custom unit and label override", () => {
    const { getByText } = render(
      <Gauge value={3} max={4} label="3 of 4 workers online" />,
    );
    expect(getByText("3 of 4 workers online")).toBeInTheDocument();
  });
});
