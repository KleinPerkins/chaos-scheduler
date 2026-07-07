import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  findNewestAssetBearingDesktopRelease,
  hasLatestJsonAsset,
  isRootDesktopRelease,
  planLatestGuard,
} from "./guard-latest-release.mjs";

const release = (tag_name, created_at, assets = [], extra = {}) => ({
  tag_name,
  created_at,
  assets: assets.map((name) => ({ name })),
  ...extra,
});

describe("guard-latest-release", () => {
  it("recognizes only root desktop releases with latest.json assets", () => {
    assert.equal(
      isRootDesktopRelease(release("chaos-scheduler-v1.0.2", "2026-01-01")),
      true,
    );
    assert.equal(
      isRootDesktopRelease(
        release("chaos-scheduler-tauri-v1.0.2", "2026-01-01"),
      ),
      false,
    );
    assert.equal(
      hasLatestJsonAsset(
        release("chaos-scheduler-v1.0.2", "2026-01-01", ["latest.json"]),
      ),
      true,
    );
  });

  it("skips the in-flight desktop tag until it has uploaded latest.json", () => {
    const releases = [
      release("chaos-scheduler-v1.0.3", "2026-01-03", []),
      release("chaos-scheduler-v1.0.2", "2026-01-02", ["latest.json"]),
      release("chaos-scheduler-v1.0.1", "2026-01-01", ["latest.json"]),
    ];

    assert.equal(
      findNewestAssetBearingDesktopRelease(releases, "chaos-scheduler-v1.0.3")
        .tag_name,
      "chaos-scheduler-v1.0.2",
    );
  });

  it("does not pin when current Latest already serves latest.json", () => {
    const plan = planLatestGuard({
      currentLatestTag: "chaos-scheduler-v1.0.2",
      desktopTag: "chaos-scheduler-v1.0.3",
      releases: [
        release("chaos-scheduler-v1.0.3", "2026-01-03", []),
        release("chaos-scheduler-v1.0.2", "2026-01-02", ["latest.json"]),
      ],
    });

    assert.equal(plan.action, "noop");
  });

  it("pins back to the newest asset-bearing desktop release when Latest is assetless", () => {
    const plan = planLatestGuard({
      currentLatestTag: "mcp-server-v1.0.3",
      desktopTag: "chaos-scheduler-v1.0.3",
      releases: [
        release("mcp-server-v1.0.3", "2026-01-04", []),
        release("chaos-scheduler-v1.0.3", "2026-01-03", []),
        release("chaos-scheduler-v1.0.2", "2026-01-02", ["latest.json"]),
      ],
    });

    assert.deepEqual(
      { action: plan.action, targetTag: plan.targetTag },
      { action: "pin", targetTag: "chaos-scheduler-v1.0.2" },
    );
  });

  it("no-ops when no prior asset-bearing desktop release exists", () => {
    const plan = planLatestGuard({
      currentLatestTag: "chaos-scheduler-v1.0.1",
      desktopTag: "chaos-scheduler-v1.0.1",
      releases: [release("chaos-scheduler-v1.0.1", "2026-01-01", [])],
    });

    assert.equal(plan.action, "noop");
  });
});
