import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import StatCard from "./StatCard";

afterEach(cleanup);

function cardOf(container: HTMLElement): HTMLElement {
  return container.firstChild as HTMLElement;
}

describe("StatCard", () => {
  it("defaults to the Mission Control (`mc`) variant: div > span value + span label", () => {
    const { container } = render(<StatCard value={5} label="Running now" />);
    const card = cardOf(container);
    expect(card.tagName).toBe("DIV");
    expect(card.className).toBe("mc-stat-card");

    const [value, label] = Array.from(card.children) as HTMLElement[];
    expect(value.tagName).toBe("SPAN");
    expect(value.className).toBe("mc-stat-value");
    expect(value.textContent).toBe("5");
    expect(label.tagName).toBe("SPAN");
    expect(label.className).toBe("mc-stat-label");
    expect(label.textContent).toBe("Running now");
  });

  it("renders the Run Detail (`rd`) variant: div > div value + div label", () => {
    const { container } = render(
      <StatCard variant="rd" value="128" label="requests" />,
    );
    const card = cardOf(container);
    expect(card.className).toBe("rd-stat-card");

    const [value, label] = Array.from(card.children) as HTMLElement[];
    expect(value.tagName).toBe("DIV");
    expect(value.className).toBe("rd-stat-value");
    expect(value.textContent).toBe("128");
    expect(label.tagName).toBe("DIV");
    expect(label.className).toBe("rd-stat-label");
    expect(label.textContent).toBe("requests");
  });

  it("merges a passthrough className after the base card class", () => {
    const { container } = render(
      <StatCard value="1" label="x" className="extra" />,
    );
    expect(cardOf(container).className).toBe("mc-stat-card extra");
  });

  it("forwards native div attributes", () => {
    const { container } = render(<StatCard value="1" label="x" title="tip" />);
    expect(cardOf(container)).toHaveAttribute("title", "tip");
  });
});
