import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import ChartTooltip from "./ChartTooltip";

afterEach(cleanup);

describe("ChartTooltip", () => {
  it("renders as a tooltip with a header and value rows", () => {
    render(
      <ChartTooltip
        header="Jul 4"
        rows={[
          { label: "OK", value: 10, color: "var(--success)" },
          { label: "Fail", value: 2, color: "var(--error)" },
        ]}
      />,
    );
    expect(screen.getByRole("tooltip")).toBeInTheDocument();
    expect(screen.getByText("Jul 4")).toBeInTheDocument();
    expect(screen.getByText("OK")).toBeInTheDocument();
    expect(screen.getByText("10")).toBeInTheDocument();
  });

  it("omits the header when not provided", () => {
    const { container } = render(
      <ChartTooltip rows={[{ label: "x", value: 1 }]} />,
    );
    expect(container.querySelector(".cs-chart-tooltip__header")).toBeNull();
  });

  it("draws a swatch only for rows that carry a color", () => {
    const { container } = render(
      <ChartTooltip
        rows={[
          { label: "a", value: 1, color: "var(--chart-1)" },
          { label: "b", value: 2 },
        ]}
      />,
    );
    expect(container.querySelectorAll(".cs-swatch")).toHaveLength(1);
  });

  it("forwards a positioning style and className", () => {
    const { container } = render(
      <ChartTooltip
        rows={[]}
        className="pos"
        style={{ position: "absolute", left: 10 }}
      />,
    );
    const tip = container.querySelector(".cs-chart-tooltip") as HTMLElement;
    expect(tip).toHaveClass("pos");
    expect(tip.style.position).toBe("absolute");
    expect(tip.style.left).toBe("10px");
  });
});
