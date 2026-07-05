import { openUrl } from "./commands";

const ALLOWED_PROTOCOLS = new Set(["http:", "https:", "cursor:"]);

/** Open a user-facing URL only when the scheme is on the allowlist. */
export async function openExternalSafe(url: string): Promise<void> {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error("Invalid URL");
  }
  if (!ALLOWED_PROTOCOLS.has(parsed.protocol)) {
    throw new Error(`Blocked URL scheme: ${parsed.protocol}`);
  }
  await openUrl(url);
}
