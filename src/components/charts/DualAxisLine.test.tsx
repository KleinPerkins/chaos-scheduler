import { afterEach, describe, it, expect } from "vitest";
import { cleanup, render } from "@testing-library/react";
import DualAxisLine, { type LineSeries } from "./DualAxisLine";

afterEach(cleanup);

const LEFT: LineSeries[] = [
  { label: "Runtime", data: [8, 9, 7, 11], color: "var(--chart-2)" },
];
const RIGHT: LineSeries[] = [
  { label: "Wait", data: [40, 55, 45, 70], color: "var(--chart-1)" },
];
const CATS = ["Mon", "Tue", "Wed", "Thu"];

describe("DualAxisLine", () => {
  it("draws one polyline per series across both axes", () => {
    const { container } = render(
      <DualAxisLine categories={CATS} leftSeries={LEFT} rightSeries={RIGHT} />,
    );
    const lines = container.querySelectorAll("polyline");
    expect(lines).toHaveLength(2);
    expect(lines[0].getAttribute("style")).toContain("var(--chart-2)");
    expect(lines[1].getAttribute("style")).toContain("var(--chart-1)");
  });

  it("summarizes the series in an accessible label", () => {
    const { getByRole } = render(
      <DualAxisLine categories={CATS} leftSeries={LEFT} rightSeries={RIGHT} />,
    );
    expect(getByRole("img").getAttribute("aria-label")).toBe(
      "Line chart of Runtime, Wait across 4 intervals",
    );
  });

  it("renders a dashed baseline reference line", () => {
    const { container } = render(
      <DualAxisLine
        categories={CATS}
        leftSeries={LEFT}
        baselines={[{ value: 4, label: "Target" }]}
      />,
    );
    const dashed = Array.from(container.querySelectorAll("line")).filter((l) =>
      l.getAttribute("stroke-dasharray"),
    );
    expect(dashed).toHaveLength(1);
  });

  it("omits axis chrome by default and draws it with showAxes", () => {
    const { container: compact } = render(
      <DualAxisLine categories={CATS} leftSeries={LEFT} />,
    );
    // no gridlines / tick labels in the compact master default
    expect(compact.querySelectorAll("line")).toHaveLength(0);
    expect(compact.querySelectorAll("text")).toHaveLength(0);

    const { container: full } = render(
      <DualAxisLine categories={CATS} leftSeries={LEFT} showAxes />,
    );
    expect(full.querySelectorAll("line").length).toBeGreaterThan(0);
    expect(full.querySelectorAll("text").length).toBeGreaterThan(0);
  });

  it("reports no data for empty input", () => {
    const { getByRole } = render(
      <DualAxisLine categories={[]} leftSeries={[]} />,
    );
    expect(getByRole("img").getAttribute("aria-label")).toBe("No data");
  });
});
