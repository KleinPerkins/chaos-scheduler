import { describe, expect, it, vi } from "vitest";
import { openExternalSafe } from "./openExternalSafe";

vi.mock("./commands", () => ({
  openUrl: vi.fn().mockResolvedValue(undefined),
}));

describe("openExternalSafe", () => {
  it("allows https URLs", async () => {
    await expect(
      openExternalSafe("https://example.com/path"),
    ).resolves.toBeUndefined();
  });

  it("blocks file URLs", async () => {
    await expect(openExternalSafe("file:///etc/passwd")).rejects.toThrow(
      /Blocked URL scheme/i,
    );
  });
});
