import type { ReactElement } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import ThresholdBand from "./ThresholdBand";

afterEach(cleanup);

/** SVG children must live under an <svg> root. */
function renderInSvg(ui: ReactElement) {
  return render(<svg>{ui}</svg>);
}

describe("ThresholdBand", () => {
  it("draws a rect spanning the band with a tinted, token-bound fill", () => {
    const { container } = renderInSvg(
      <ThresholdBand
        x={10}
        width={100}
        y1={40}
        y2={10}
        color="var(--warning)"
      />,
    );
    const rect = container.querySelector("rect")!;
    expect(rect).toHaveAttribute("x", "10");
    expect(rect).toHaveAttribute("width", "100");
    // order-independent: y = min(y1,y2), height = |y2 - y1|
    expect(rect).toHaveAttribute("y", "10");
    expect(rect).toHaveAttribute("height", "30");
    expect(rect.style.fill).toBe("var(--warning)");
    expect(rect).toHaveAttribute("fill-opacity", "0.12");
  });

  it("omits boundary lines by default and draws them when requested", () => {
    const { container: none } = renderInSvg(
      <ThresholdBand x={0} width={50} y1={0} y2={20} />,
    );
    expect(none.querySelectorAll("line")).toHaveLength(0);

    const { container: both } = renderInSvg(
      <ThresholdBand x={0} width={50} y1={0} y2={20} boundary="both" />,
    );
    expect(both.querySelectorAll("line")).toHaveLength(2);
  });

  it("renders a corner label only when provided", () => {
    const { container: bare } = renderInSvg(
      <ThresholdBand x={0} width={50} y1={0} y2={20} />,
    );
    expect(bare.querySelector("text")).toBeNull();

    const { getByText } = renderInSvg(
      <ThresholdBand x={0} width={50} y1={0} y2={20} label="Degraded" />,
    );
    expect(getByText("Degraded")).toBeInTheDocument();
  });

  it("merges a passthrough className onto the group", () => {
    const { container } = renderInSvg(
      <ThresholdBand x={0} width={1} y1={0} y2={1} className="zone" />,
    );
    expect(container.querySelector("g.cs-threshold-band")).toHaveClass("zone");
  });
});
