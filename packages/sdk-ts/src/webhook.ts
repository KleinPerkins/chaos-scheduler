/**
 * Webhook signature helpers matching the backend HMAC-SHA256 scheme.
 *
 * SOURCE OF TRUTH: `src-tauri/src/actions.rs::sign_payload` computes
 * `hex(HMAC_SHA256(secret, raw_body))` and the webhook delivery sends it in the
 * `X-Chaos-Signature: sha256=<hex>` header (with `X-Chaos-Event:
 * run.succeeded|run.failed`). The inbound trigger receiver
 * (`api.rs::inbound_dispatch`) verifies the same scheme over the raw request
 * body. Verification MUST run over the exact raw bytes received — never a
 * re-serialized object, whose key order/spacing may differ.
 *
 * Note: this helper normalizes hex case for **outbound** result webhooks.
 * The scheduler's inbound trigger verifier is case-sensitive — send lowercase hex.
 */

import { createHmac, timingSafeEqual } from "node:crypto";

/** Body payloads may be provided as a UTF-8 string or raw bytes. */
export type SignaturePayload = string | Uint8Array;

const SIGNATURE_PREFIX = "sha256=";

function toBytes(payload: SignaturePayload): Uint8Array {
  return typeof payload === "string"
    ? new TextEncoder().encode(payload)
    : payload;
}

/**
 * Compute the hex HMAC-SHA256 signature of `payload` with `secret`.
 * Equivalent to the backend's `sign_payload`.
 */
export function computeWebhookSignature(
  payload: SignaturePayload,
  secret: string,
): string {
  return createHmac("sha256", secret).update(toBytes(payload)).digest("hex");
}

/** The full header value the backend emits, e.g. `sha256=<hex>`. */
export function webhookSignatureHeader(
  payload: SignaturePayload,
  secret: string,
): string {
  return `${SIGNATURE_PREFIX}${computeWebhookSignature(payload, secret)}`;
}

/** Strip an optional `sha256=` prefix and lowercase the hex digest. */
function normalizeSignature(header: string): string {
  const trimmed = header.trim();
  const withoutPrefix = trimmed.toLowerCase().startsWith(SIGNATURE_PREFIX)
    ? trimmed.slice(SIGNATURE_PREFIX.length)
    : trimmed;
  return withoutPrefix.trim().toLowerCase();
}

/**
 * Verify a webhook signature in constant time.
 *
 * @param payload The RAW request/response body (string or bytes). Do not
 *   re-serialize a parsed object — sign/verify the exact bytes.
 * @param header  The `X-Chaos-Signature` header value (with or without the
 *   `sha256=` prefix).
 * @param secret  The shared webhook secret configured on the action / inbound
 *   trigger.
 * @returns `true` iff the signature matches.
 */
export function verifyWebhookSignature(
  payload: SignaturePayload,
  header: string | null | undefined,
  secret: string,
): boolean {
  if (!header || !secret) return false;
  const expected = computeWebhookSignature(payload, secret);
  const provided = normalizeSignature(header);
  // Both are lowercase hex of equal length for a valid digest; timingSafeEqual
  // requires equal-length buffers, so guard length first (not secret-dependent).
  if (provided.length !== expected.length) return false;
  return timingSafeEqual(
    Buffer.from(provided, "utf8"),
    Buffer.from(expected, "utf8"),
  );
}
