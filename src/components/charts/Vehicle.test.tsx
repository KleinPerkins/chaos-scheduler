import { afterEach, describe, it, expect } from "vitest";
import { cleanup, render } from "@testing-library/react";
import Vehicle from "./Vehicle";
import type { VehicleColor, VehicleStyle } from "./Vehicle";

afterEach(cleanup);

const STYLES: VehicleStyle[] = ["sedan", "coupe", "racer", "truck"];
const COLOR_VAR: Record<VehicleColor, string> = {
  blue: "var(--running)",
  teal: "var(--chart-3)",
  amber: "var(--warning)",
};

/** The first painted shape is always the body (path for cars, rect for truck). */
function body(container: HTMLElement): SVGElement {
  return container.querySelector("path, rect") as SVGElement;
}

describe("Vehicle", () => {
  it("renders an accessible image with variant data attributes", () => {
    const { getByRole } = render(<Vehicle style="sedan" color="teal" />);
    const svg = getByRole("img");
    expect(svg.getAttribute("aria-label")).toBe("teal sedan");
    expect(svg.getAttribute("data-vehicle-style")).toBe("sedan");
    expect(svg.getAttribute("data-vehicle-color")).toBe("teal");
    expect(svg.tagName.toLowerCase()).toBe("svg");
  });

  it.each(STYLES)("renders the %s silhouette", (style) => {
    const { getByRole } = render(<Vehicle style={style} />);
    expect(getByRole("img").getAttribute("data-vehicle-style")).toBe(style);
  });

  it.each(Object.entries(COLOR_VAR))(
    "binds the body fill to the %s token",
    (color, token) => {
      const { container } = render(
        <Vehicle style="sedan" color={color as VehicleColor} />,
      );
      expect(body(container).getAttribute("style")).toContain(token);
    },
  );

  it("applies the status-red override in the over-state", () => {
    const { getByRole, container } = render(<Vehicle style="racer" over />);
    expect(getByRole("img").hasAttribute("data-vehicle-over")).toBe(true);
    expect(body(container).getAttribute("style")).toContain("var(--error)");
  });

  it("does not paint the body red when not over", () => {
    const { getByRole, container } = render(
      <Vehicle style="racer" color="blue" />,
    );
    expect(getByRole("img").hasAttribute("data-vehicle-over")).toBe(false);
    expect(body(container).getAttribute("style")).toContain("var(--running)");
  });

  it("draws the truck with six wheels (mid axle omitted)", () => {
    const { container } = render(<Vehicle style="truck" />);
    // 6 tires + 6 hubs, no headlight circle.
    expect(container.querySelectorAll("circle")).toHaveLength(12);
  });

  it("renders decoratively (aria-hidden, no role) when asked", () => {
    const { queryByRole, container } = render(
      <Vehicle style="coupe" decorative />,
    );
    expect(queryByRole("img")).toBeNull();
    expect(container.querySelector("svg")?.getAttribute("aria-hidden")).toBe(
      "true",
    );
  });

  it("binds glass, tires, and hubs to shared tokens (never raw hex)", () => {
    const { container } = render(<Vehicle style="sedan" color="blue" />);
    const html = container.innerHTML;
    expect(html).toContain("var(--bg-secondary)"); // glass + tires
    expect(html).toContain("var(--text-secondary)"); // hubs
    expect(html).not.toMatch(/#[0-9a-fA-F]{3,6}/);
  });
});
