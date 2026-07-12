import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import Legend from "./Legend";

afterEach(cleanup);

describe("Legend", () => {
  it("renders one list item per entry with its label", () => {
    render(
      <Legend
        items={[
          { label: "East", color: "var(--chart-1)" },
          { label: "West", color: "var(--chart-2)" },
        ]}
      />,
    );
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
    expect(screen.getByText("East")).toBeInTheDocument();
    expect(screen.getByText("West")).toBeInTheDocument();
  });

  it("binds each swatch color to the token via a CSS custom property", () => {
    const { container } = render(
      <Legend items={[{ label: "East", color: "var(--chart-1)" }]} />,
    );
    const swatch = container.querySelector(".cs-swatch") as HTMLElement;
    expect(swatch.style.getPropertyValue("--cs-swatch-color")).toBe(
      "var(--chart-1)",
    );
  });

  it("supports a vertical orientation and a passthrough className", () => {
    const { container } = render(
      <Legend items={[]} orientation="vertical" className="filters" />,
    );
    const list = container.querySelector("ul")!;
    expect(list).toHaveClass("cs-legend--vertical");
    expect(list).toHaveClass("filters");
  });

  it("applies the requested swatch shape", () => {
    const { container } = render(
      <Legend
        items={[{ label: "avg", color: "var(--chart-3)", shape: "line" }]}
      />,
    );
    expect(container.querySelector(".cs-swatch--line")).toBeInTheDocument();
  });
});
