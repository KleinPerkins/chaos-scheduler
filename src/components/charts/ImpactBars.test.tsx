import { afterEach, describe, it, expect } from "vitest";
import { cleanup, render } from "@testing-library/react";
import ImpactBars, { type ImpactBarItem } from "./ImpactBars";

afterEach(cleanup);

const ITEMS: ImpactBarItem[] = [
  { label: "Host pool", value: 5400, valueLabel: "1h 30m" },
  { label: "Resource lock", value: 15120, valueLabel: "4h 12m" },
  { label: "Upstream dep", value: 10080, valueLabel: "2h 48m" },
];

function rows(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(".cs-impact__row"));
}

describe("ImpactBars", () => {
  it("ranks rows descending by value by default", () => {
    const { container } = render(<ImpactBars items={ITEMS} />);
    const labels = rows(container).map(
      (r) => r.querySelector(".cs-impact__label")?.textContent,
    );
    expect(labels).toEqual(["Resource lock", "Upstream dep", "Host pool"]);
  });

  it("preserves input order when sort is disabled", () => {
    const { container } = render(<ImpactBars items={ITEMS} sort={false} />);
    const labels = rows(container).map(
      (r) => r.querySelector(".cs-impact__label")?.textContent,
    );
    expect(labels).toEqual(["Host pool", "Resource lock", "Upstream dep"]);
  });

  it("renders the preformatted value labels", () => {
    const { getByText } = render(<ImpactBars items={ITEMS} />);
    expect(getByText("4h 12m")).toBeInTheDocument();
    expect(getByText("1h 30m")).toBeInTheDocument();
  });

  it("scales the top bar to 100% and applies the palette color", () => {
    const { container } = render(<ImpactBars items={ITEMS} />);
    const topBar =
      rows(container)[0].querySelector<HTMLElement>(".cs-impact__bar")!;
    expect(topBar.style.width).toBe("100%");
    expect(topBar.style.getPropertyValue("--cs-bar-color")).toBe(
      "var(--chart-1)",
    );
  });

  it("honors an explicit per-item color", () => {
    const { container } = render(
      <ImpactBars
        items={[{ label: "A", value: 10, color: "var(--success)" }]}
      />,
    );
    const bar = container.querySelector<HTMLElement>(".cs-impact__bar")!;
    expect(bar.style.getPropertyValue("--cs-bar-color")).toBe("var(--success)");
  });

  it("exposes an accessible list name when provided", () => {
    const { getByRole } = render(
      <ImpactBars items={ITEMS} ariaLabel="Top wait contributors" />,
    );
    expect(getByRole("list").getAttribute("aria-label")).toBe(
      "Top wait contributors",
    );
  });
});
