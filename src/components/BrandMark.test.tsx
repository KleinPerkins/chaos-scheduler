import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import BrandMark from "./BrandMark";

afterEach(cleanup);

describe("BrandMark", () => {
  it("renders an accessible `img` svg named for the product by default", () => {
    render(<BrandMark />);
    const svg = screen.getByRole("img", { name: "Chaos Scheduler" });
    expect(svg.tagName.toLowerCase()).toBe("svg");
    expect(svg).toHaveAttribute("viewBox", "0 0 512 512");
  });

  it("defaults to a 30px square and honors a custom size", () => {
    const { container, rerender } = render(<BrandMark />);
    let svg = container.querySelector("svg")!;
    expect(svg).toHaveAttribute("width", "30");
    expect(svg).toHaveAttribute("height", "30");

    rerender(<BrandMark size={18} />);
    svg = container.querySelector("svg")!;
    expect(svg).toHaveAttribute("width", "18");
    expect(svg).toHaveAttribute("height", "18");
  });

  it("renders the orbital-8 geometry: two arcs, two dots, drawn twice (glow + crisp)", () => {
    const { container } = render(<BrandMark />);
    const svg = container.querySelector("svg")!;
    expect(svg.querySelectorAll("path")).toHaveLength(2);
    expect(svg.querySelectorAll("circle")).toHaveLength(2);
    // The mark is stamped twice: a blurred glow underlay + the crisp mark.
    const uses = svg.querySelectorAll("use");
    expect(uses).toHaveLength(2);
    expect(uses[0].getAttribute("href")).toMatch(/-mark$/);
    expect(uses[1].getAttribute("href")).toBe(uses[0].getAttribute("href"));
  });

  it("renders as decorative (aria-hidden, no accessible name) when title is empty", () => {
    const { container } = render(<BrandMark title="" />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    const svg = container.querySelector("svg")!;
    expect(svg).toHaveAttribute("aria-hidden", "true");
    expect(svg).not.toHaveAttribute("aria-label");
  });

  it("uses a custom title as the accessible name", () => {
    render(<BrandMark title="Chaos Scheduler home" />);
    expect(
      screen.getByRole("img", { name: "Chaos Scheduler home" }),
    ).toBeInTheDocument();
  });

  it("merges a passthrough className", () => {
    const { container } = render(<BrandMark className="sidebar-brand-mark" />);
    expect(container.querySelector("svg")).toHaveClass("sidebar-brand-mark");
  });

  it("scopes gradient ids per instance so multiple marks do not collide", () => {
    const { container } = render(
      <>
        <BrandMark />
        <BrandMark />
      </>,
    );
    const svgs = container.querySelectorAll("svg");
    const firstGradientId = (svg: Element) =>
      svg.querySelector("linearGradient")!.getAttribute("id");
    expect(firstGradientId(svgs[0])).not.toBe(firstGradientId(svgs[1]));
  });
});
