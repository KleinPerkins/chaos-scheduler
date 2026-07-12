import { afterEach, describe, it, expect } from "vitest";
import { cleanup, render } from "@testing-library/react";
import QueueLine from "./QueueLine";
import type { LineSeries } from "./DualAxisLine";

afterEach(cleanup);

const SERIES: LineSeries[] = [
  { label: "default", data: [20, 45, 60, 92], color: "var(--chart-3)" },
  { label: "batch", data: [60, 65, 70, 82], color: "var(--chart-2)" },
];
const CATS = ["Mon", "Tue", "Wed", "Thu"];

describe("QueueLine", () => {
  it("draws one occupancy polyline per series", () => {
    const { container } = render(
      <QueueLine categories={CATS} series={SERIES} />,
    );
    const lines = container.querySelectorAll("polyline");
    expect(lines).toHaveLength(2);
    expect(lines[0].getAttribute("style")).toContain("var(--chart-3)");
  });

  it("renders the near-capacity band via ThresholdBand by default", () => {
    const { container } = render(
      <QueueLine categories={CATS} series={SERIES} />,
    );
    expect(container.querySelector(".cs-threshold-band")).not.toBeNull();
  });

  it("omits the band when disabled", () => {
    const { container } = render(
      <QueueLine categories={CATS} series={SERIES} showCapacityBand={false} />,
    );
    expect(container.querySelector(".cs-threshold-band")).toBeNull();
  });

  it("summarizes series and the capacity threshold accessibly", () => {
    const { getByRole } = render(
      <QueueLine categories={CATS} series={SERIES} capacity={90} />,
    );
    expect(getByRole("img").getAttribute("aria-label")).toBe(
      "Queue occupancy for default, batch against a 90% capacity threshold",
    );
  });

  it("draws the plot frame and tick labels only with showAxes", () => {
    const { container: compact } = render(
      <QueueLine categories={CATS} series={SERIES} showCapacityBand={false} />,
    );
    expect(compact.querySelectorAll("text")).toHaveLength(0);

    const { container: full } = render(
      <QueueLine
        categories={CATS}
        series={SERIES}
        showCapacityBand={false}
        showAxes
      />,
    );
    // 5 y ticks + 4 x labels
    expect(full.querySelectorAll("text")).toHaveLength(9);
  });
});
