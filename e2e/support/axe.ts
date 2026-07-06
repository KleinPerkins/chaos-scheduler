import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";

/** Run axe against the current page; fails the test on serious violations. */
export async function expectNoAxeViolations(
  page: Page,
  context?: string,
): Promise<void> {
  const results = await new AxeBuilder({ page }).analyze();

  const serious = results.violations.filter((v) =>
    ["critical", "serious"].includes(v.impact ?? ""),
  );

  if (serious.length > 0) {
    const summary = serious
      .map((v) => {
        const nodes = v.nodes
          .map(
            (n) =>
              `    - ${n.target.join(" ")}\n      ${n.failureSummary ?? ""}`,
          )
          .join("\n");
        return `${v.id}: ${v.description} (${v.nodes.length} nodes)\n${nodes}`;
      })
      .join("\n");
    throw new Error(
      `axe violations${context ? ` (${context})` : ""}:\n${summary}`,
    );
  }
}
