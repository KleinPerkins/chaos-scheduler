// Centralized product branding for the UI. Mirrors the user-facing values in
// `src-tauri/src/branding.rs` so a rebrand is a single-file change on each side.

/** Human-facing product name. */
export const PRODUCT_NAME = "Chaos Scheduler";

/** Short name used in tight spaces (menu-bar popup header, sidebar brand). */
export const PRODUCT_SHORT_NAME = "Chaos Scheduler";

/**
 * Current desktop app version. Auto-bumped on release by release-please
 * (see the `generic` extra-file entry in `release-please-config.json`); the
 * `x-release-please-version` marker below is what release-please rewrites.
 */
export const APP_VERSION = "0.6.0"; // x-release-please-version

/** Default email `from` display name (mirrors `EMAIL_FROM_NAME`). */
export const EMAIL_FROM_NAME = "Chaos Scheduler";

/** Repository slug used for release/download links. */
export const REPO_SLUG = "KleinPerkins/chaos-scheduler";

/** GitHub releases page for manual installs / release notes. */
export const RELEASES_URL = `https://github.com/${REPO_SLUG}/releases`;
