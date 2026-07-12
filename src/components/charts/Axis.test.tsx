import type { ReactElement } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import Axis from "./Axis";

afterEach(cleanup);

function renderInSvg(ui: ReactElement) {
  return render(<svg>{ui}</svg>);
}

describe("Axis", () => {
  it("renders explicit ticks plus a domain line (bottom)", () => {
    const { container } = renderInSvg(
      <Axis
        orientation="bottom"
        length={300}
        ticks={[
          { offset: 0, label: "a" },
          { offset: 150, label: "b" },
          { offset: 300, label: "c" },
        ]}
      />,
    );
    // one domain line + three tick marks
    expect(container.querySelectorAll("line")).toHaveLength(4);
    expect(container.querySelectorAll(".cs-axis__tick")).toHaveLength(3);
    for (const label of ["a", "b", "c"]) {
      expect(container.textContent).toContain(label);
    }
  });

  it("auto-computes numeric ticks from a domain, spanning the endpoints", () => {
    const { container } = renderInSvg(
      <Axis orientation="left" length={200} domain={[0, 100]} tickCount={5} />,
    );
    expect(container.querySelectorAll(".cs-axis__tick").length).toBeGreaterThan(
      0,
    );
    expect(container.textContent).toContain("0");
    expect(container.textContent).toContain("100");
  });

  it("applies a custom tickFormat in domain mode", () => {
    const { container } = renderInSvg(
      <Axis
        orientation="left"
        length={100}
        domain={[0, 100]}
        tickFormat={(v) => `${v}%`}
      />,
    );
    expect(container.textContent).toContain("100%");
  });

  it("can hide the domain line", () => {
    const { container } = renderInSvg(
      <Axis
        orientation="bottom"
        length={100}
        showDomainLine={false}
        ticks={[{ offset: 0, label: "x" }]}
      />,
    );
    // only the single tick mark, no domain line
    expect(container.querySelectorAll("line")).toHaveLength(1);
  });

  it("applies an orientation modifier class", () => {
    const { container } = renderInSvg(
      <Axis orientation="right" length={50} ticks={[]} />,
    );
    expect(container.querySelector(".cs-axis")).toHaveClass("cs-axis--right");
  });
});
