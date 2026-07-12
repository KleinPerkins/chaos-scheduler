import { afterEach, describe, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import InfoTip, { type GlossaryRow } from "./InfoTip";

afterEach(cleanup);

const ROWS: GlossaryRow[] = [
  { term: "SLA slack", meaning: "Time until deadline breach." },
  { term: "p50", meaning: "Historical median runtime." },
];

describe("InfoTip", () => {
  it("renders a focusable trigger button whose accessible name is the title", () => {
    render(<InfoTip title="SLA slack" def="Time until deadline breach." />);
    const trigger = screen.getByRole("button", { name: "SLA slack" });
    expect(trigger).toHaveAttribute("type", "button");
  });

  it("renders a tooltip card with the title and definition", () => {
    render(<InfoTip title="Metric definition" def="One-line definition." />);
    const tip = screen.getByRole("tooltip");
    expect(within(tip).getByText("Metric definition")).toHaveClass(
      "info-tip-title",
    );
    expect(within(tip).getByText("One-line definition.")).toHaveClass(
      "info-tip-def",
    );
  });

  it("describes the trigger with the definition via aria-describedby", () => {
    render(<InfoTip title="p50" def="Historical median runtime." />);
    const trigger = screen.getByRole("button", { name: "p50" });
    const describedBy = trigger.getAttribute("aria-describedby");
    expect(describedBy).toBeTruthy();
    const def = document.getElementById(describedBy as string);
    expect(def).toHaveTextContent("Historical median runtime.");
  });

  it("does not render the glossary by default", () => {
    const { container } = render(<InfoTip title="t" def="d" />);
    expect(container.querySelector(".info-tip-glossary")).toBeNull();
  });

  it("renders a glossary table (header + rows) when enabled with rows", () => {
    const { container } = render(
      <InfoTip title="t" def="d" glossary glossaryRows={ROWS} />,
    );
    const glossary = container.querySelector(".info-tip-glossary");
    expect(glossary).not.toBeNull();
    // Header row + one row per entry.
    const rows = glossary!.querySelectorAll(".info-tip-g-row");
    expect(rows).toHaveLength(ROWS.length + 1);
    expect(rows[0]).toHaveClass("info-tip-g-head");
    expect(rows[0].textContent).toBe("TermMeaning");
    expect(rows[1].textContent).toBe("SLA slackTime until deadline breach.");
    expect(rows[2].textContent).toBe("p50Historical median runtime.");
  });

  it("omits the glossary when enabled but no rows are supplied", () => {
    const { container } = render(
      <InfoTip title="t" def="d" glossary glossaryRows={[]} />,
    );
    expect(container.querySelector(".info-tip-glossary")).toBeNull();
  });

  it("merges a passthrough className onto the container", () => {
    const { container } = render(
      <InfoTip title="t" def="d" className="mc-infotip" />,
    );
    expect((container.firstChild as HTMLElement).className).toBe(
      "info-tip mc-infotip",
    );
  });

  it("dismisses on Escape while keeping focus on the trigger", () => {
    const { container } = render(
      <InfoTip title="p50" def="Historical median runtime." />,
    );
    const trigger = screen.getByRole("button", { name: "p50" });
    trigger.focus();
    expect(trigger).toHaveFocus();

    fireEvent.keyDown(trigger, { key: "Escape" });

    // Card is hidden (via the dismissed-state class) but focus is retained,
    // per the APG tooltip pattern: Escape dismisses without moving focus.
    expect(container.firstChild).toHaveClass("is-dismissed");
    expect(trigger).toHaveFocus();
  });

  it("re-arms the tooltip once focus leaves the trigger", () => {
    const { container } = render(<InfoTip title="p50" def="d" />);
    const trigger = screen.getByRole("button", { name: "p50" });
    trigger.focus();
    fireEvent.keyDown(trigger, { key: "Escape" });
    expect(container.firstChild).toHaveClass("is-dismissed");

    fireEvent.blur(trigger);
    expect(container.firstChild).not.toHaveClass("is-dismissed");
  });
});
