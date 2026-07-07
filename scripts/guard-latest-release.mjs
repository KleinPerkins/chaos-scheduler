#!/usr/bin/env node
// Guard the updater endpoint during release-please's multi-release window.
//
// A newly-created desktop/component release can briefly steal GitHub's repo-wide
// "Latest" flag before the desktop build has uploaded latest.json. When that
// happens, /releases/latest/download/latest.json 404s until build-macos finishes.
// This guard pins "Latest" back to the newest asset-bearing desktop release.
import { execFileSync } from "node:child_process";

const ROOT_DESKTOP_TAG_RE = /^chaos-scheduler-v.+/;

function releaseTag(release) {
  return release?.tag_name ?? release?.tagName ?? "";
}

function releaseCreatedAt(release) {
  return release?.created_at ?? release?.createdAt ?? "";
}

export function isRootDesktopRelease(release) {
  const tag = releaseTag(release);
  return (
    ROOT_DESKTOP_TAG_RE.test(tag) &&
    !release?.draft &&
    !release?.isDraft &&
    !release?.prerelease &&
    !release?.isPrerelease
  );
}

export function hasLatestJsonAsset(release) {
  return (
    Array.isArray(release?.assets) &&
    release.assets.some((asset) => asset?.name === "latest.json")
  );
}

export function findNewestAssetBearingDesktopRelease(
  releases,
  desktopTag = "",
) {
  return [...releases]
    .filter(isRootDesktopRelease)
    .sort((a, b) => releaseCreatedAt(b).localeCompare(releaseCreatedAt(a)))
    .find((release) => {
      const tag = releaseTag(release);
      if (tag === desktopTag && !hasLatestJsonAsset(release)) {
        return false;
      }
      return hasLatestJsonAsset(release);
    });
}

export function planLatestGuard({
  releases,
  currentLatestTag,
  desktopTag = "",
}) {
  const currentLatest = releases.find(
    (release) => releaseTag(release) === currentLatestTag,
  );

  if (currentLatest && hasLatestJsonAsset(currentLatest)) {
    return {
      action: "noop",
      reason: `GitHub "Latest" already serves latest.json from ${currentLatestTag}.`,
    };
  }

  const target = findNewestAssetBearingDesktopRelease(releases, desktopTag);
  if (!target) {
    return {
      action: "noop",
      reason: "No prior asset-bearing desktop release found; nothing to pin.",
    };
  }

  const targetTag = releaseTag(target);
  if (currentLatestTag === targetTag) {
    return {
      action: "noop",
      reason: `GitHub "Latest" already points at ${targetTag}.`,
    };
  }

  return {
    action: "pin",
    targetTag,
    reason: currentLatestTag
      ? `GitHub "Latest" (${currentLatestTag}) does not currently serve latest.json.`
      : 'GitHub does not currently report a "Latest" release.',
  };
}

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--repo") {
      args.repo = argv[++i];
    } else if (arg === "--desktop-tag") {
      args.desktopTag = argv[++i];
    } else if (arg === "--dry-run") {
      args.dryRun = true;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return args;
}

function ghJson(args) {
  const output = execFileSync("gh", args, { encoding: "utf8" });
  return JSON.parse(output);
}

function gh(args) {
  execFileSync("gh", args, { stdio: "inherit" });
}

function usage() {
  console.error(
    "usage: node scripts/guard-latest-release.mjs --repo <owner/repo> [--desktop-tag <tag>] [--dry-run]",
  );
}

function main() {
  const {
    repo,
    desktopTag = "",
    dryRun = false,
  } = parseArgs(process.argv.slice(2));
  const targetRepo = repo || process.env.REPO || process.env.GITHUB_REPOSITORY;
  if (!targetRepo) {
    usage();
    process.exit(1);
  }

  const releases = ghJson(["api", `repos/${targetRepo}/releases?per_page=100`]);
  const latestSummaries = ghJson([
    "release",
    "list",
    "--repo",
    targetRepo,
    "--limit",
    "100",
    "--json",
    "tagName,isLatest",
  ]);
  const currentLatestTag =
    latestSummaries.find((release) => release.isLatest)?.tagName ?? "";

  const plan = planLatestGuard({ releases, currentLatestTag, desktopTag });
  console.log(plan.reason);
  if (plan.action !== "pin") {
    return;
  }

  console.log(`Pinning GitHub "Latest" to ${plan.targetTag}.`);
  if (!dryRun) {
    gh(["release", "edit", plan.targetTag, "--repo", targetRepo, "--latest"]);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    main();
  } catch (err) {
    console.error(`::error::${err.message}`);
    process.exit(1);
  }
}
