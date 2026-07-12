import { test, expect } from "@playwright/test";
import { expectNoAxeViolations } from "./support/axe";
import { openRunDetail } from "./support/nav";

test.describe("Run detail smoke", () => {
  test("keeps authoritative observability accessible in both themes", async ({
    page,
  }) => {
    await openRunDetail(page);

    await expect(
      page.getByRole("region", { name: "Nightly sync run detail" }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Raw logs" }).focus();
    await page.keyboard.press("Enter");
    await expect(
      page.getByRole("button", { name: "Raw logs" }),
    ).toHaveAttribute("aria-expanded", "false");
    await page.keyboard.press("Enter");

    await expect(page.getByText("Run run-demo-1")).toBeVisible();
    await expect(page.getByText("succeeded", { exact: true })).toBeVisible();
    await expect(page.getByText(/Started /)).toBeVisible();
    const stdout = page.getByRole("tab", { name: "stdout" });
    const stderr = page.getByRole("tab", { name: "stderr" });
    await expect(stdout).toHaveAttribute("aria-selected", "true");
    await stdout.focus();
    await page.keyboard.press("ArrowRight");
    await expect(stderr).toBeFocused();
    await expect(stderr).toHaveAttribute("aria-selected", "true");
    await page.keyboard.press("ArrowLeft");
    await expect(stdout).toBeFocused();
    await expectNoAxeViolations(page, "run detail dark");

    await page.getByRole("button", { name: "Light theme" }).click();
    await expect(page.locator('html[data-theme="light"]')).toHaveCount(1);
    await expectNoAxeViolations(page, "run detail light");
  });
});
