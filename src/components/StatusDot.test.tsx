import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/react";
import StatusDot from "./StatusDot";

afterEach(cleanup);

function classOf(container: HTMLElement): string {
  return (container.firstChild as HTMLElement).className;
}

const indexCss = readFileSync(resolve(process.cwd(), "src/index.css"), "utf8");

/**
 * Return the declaration body of the top-level CSS rule whose selector list
 * includes `selector`. Splitting on `}` isolates each flat rule, so an unrelated
 * `@media` block elsewhere in the file never desyncs the lookup.
 */
function ruleBodyFor(css: string, selector: string): string {
  const clean = css.replace(/\/\*[\s\S]*?\*\//g, "");
  for (const chunk of clean.split("}")) {
    const brace = chunk.indexOf("{");
    if (brace === -1) continue;
    const selectors = chunk
      .slice(0, brace)
      .split(",")
      .map((s) => s.trim());
    if (selectors.includes(selector)) return chunk.slice(brace + 1);
  }
  throw new Error(`No CSS rule found for selector ${selector}`);
}

describe("StatusDot", () => {
  it("defaults to the `.status-dot` base plus the status modifier", () => {
    const { container } = render(<StatusDot status="succeeded" />);
    expect(classOf(container)).toBe("status-dot succeeded");
  });

  it("renders the `.mc-dot` variant base plus the status modifier", () => {
    const { container } = render(
      <StatusDot variant="mc-dot" status="success" />,
    );
    expect(classOf(container)).toBe("mc-dot success");
  });

  it("maps each status to its modifier class for both variants", () => {
    const { container, rerender } = render(<StatusDot status="running" />);
    expect(classOf(container)).toBe("status-dot running");
    rerender(<StatusDot status="failed" />);
    expect(classOf(container)).toBe("status-dot failed");
    rerender(<StatusDot status="dead_lettered" />);
    expect(classOf(container)).toBe("status-dot dead_lettered");
    rerender(<StatusDot variant="mc-dot" status="queued" />);
    expect(classOf(container)).toBe("mc-dot queued");
  });

  it("merges a passthrough className after the base + status", () => {
    const { container } = render(
      <StatusDot variant="mc-dot" status="success" className="extra" />,
    );
    expect(classOf(container)).toBe("mc-dot success extra");
  });

  it("is decorative by default and honors explicit accessible overrides", () => {
    const { container, rerender } = render(<StatusDot status="running" />);
    const dot = () => container.firstChild as HTMLElement;

    expect(dot()).toHaveAttribute("aria-hidden", "true");

    rerender(
      <StatusDot status="running" aria-label="Running" aria-hidden={true} />,
    );
    expect(dot()).toHaveAttribute("aria-label", "Running");
    expect(dot()).not.toHaveAttribute("aria-hidden");
    expect(dot()).toHaveAttribute("role", "img");

    rerender(<StatusDot status="running" aria-hidden={false} />);
    expect(dot()).toHaveAttribute("aria-hidden", "false");
    expect(dot()).not.toHaveAttribute("role");
  });

  it("forwards native span attributes", () => {
    const { container } = render(<StatusDot status="running" title="live" />);
    expect(container.firstChild as HTMLElement).toHaveAttribute(
      "title",
      "live",
    );
  });

  // D04 parity: the Mission Control warning dot must render on the warning
  // color, matching the Figma warning mapping and the sibling `.status-dot`
  // warning tier — not the muted queued/admitted color.
  it("maps the mc-dot warning tier to the warning token, not muted (D04)", () => {
    const warningBody = ruleBodyFor(indexCss, ".mc-dot.warning");
    expect(warningBody).toContain("var(--warning)");
    expect(warningBody).not.toContain("var(--text-muted)");

    const pollExhaustedBody = ruleBodyFor(indexCss, ".mc-dot.poll_exhausted");
    expect(pollExhaustedBody).toContain("var(--warning)");
    expect(pollExhaustedBody).not.toContain("var(--text-muted)");
  });

  it("keeps the mc-dot queued/admitted tier muted", () => {
    const queuedBody = ruleBodyFor(indexCss, ".mc-dot.queued");
    expect(queuedBody).toContain("var(--text-muted)");
    expect(queuedBody).not.toContain("var(--warning)");
  });
});
