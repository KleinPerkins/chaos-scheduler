/**
 * Webhook signature helpers matching the backend HMAC-SHA256 schemes.
 *
 * **Outbound** (completion webhooks): `hex(HMAC_SHA256(secret, raw_body))` per
 * `src-tauri/src/actions.rs::sign_payload`. Header: `X-Chaos-Signature: sha256=<hex>`.
 *
 * **Inbound** (dispatch trigger): canonical string
 * `METHOD\nPATH\nTIMESTAMP\nSHA256_HEX(body)` then
 * `hex(HMAC_SHA256(secret, canonical))` per `api.rs::inbound_canonical_payload`.
 * Headers: `X-Chaos-Timestamp`, `X-Chaos-Event-Id`, `X-Chaos-Signature`.
 *
 * Verification MUST run over the exact raw bytes received — never a re-serialized
 * object. Inbound verifier is case-sensitive on hex; outbound helper normalizes case.
 */

import {
  createHash,
  createHmac,
  randomUUID,
  timingSafeEqual,
} from "node:crypto";

/** Body payloads may be provided as a UTF-8 string or raw bytes. */
export type SignaturePayload = string | Uint8Array;

const SIGNATURE_PREFIX = "sha256=";

function toBytes(payload: SignaturePayload): Uint8Array {
  return typeof payload === "string"
    ? new TextEncoder().encode(payload)
    : payload;
}

function sha256Hex(payload: SignaturePayload): string {
  return createHash("sha256").update(toBytes(payload)).digest("hex");
}

/**
 * Compute the hex HMAC-SHA256 signature of `payload` with `secret`.
 * Equivalent to the backend's `sign_payload` (outbound webhooks).
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

/**
 * Canonical inbound dispatch signing string (parity with `inbound_canonical_payload`).
 */
export function inboundCanonicalPayload(
  method: string,
  path: string,
  timestamp: string,
  body: SignaturePayload,
): string {
  return `${method.toUpperCase()}\n${path}\n${timestamp}\n${sha256Hex(body)}`;
}

/**
 * Compute inbound dispatch HMAC hex over the canonical payload.
 */
export function computeInboundDispatchSignature(
  method: string,
  path: string,
  timestamp: string,
  body: SignaturePayload,
  secret: string,
): string {
  const canonical = inboundCanonicalPayload(method, path, timestamp, body);
  return createHmac("sha256", secret).update(canonical).digest("hex");
}

export interface InboundDispatchHeaderOptions {
  method?: string;
  path: string;
  timestamp?: string;
  eventId?: string;
  body: SignaturePayload;
  secret: string;
}

/**
 * Headers for `POST .../dispatch` when the workflow inbound secret is configured.
 */
export function inboundDispatchHeaders(
  options: InboundDispatchHeaderOptions,
): Record<string, string> {
  const method = options.method ?? "POST";
  const timestamp =
    options.timestamp ?? Math.floor(Date.now() / 1000).toString();
  const eventId = options.eventId ?? randomUUID();
  const signature = computeInboundDispatchSignature(
    method,
    options.path,
    timestamp,
    options.body,
    options.secret,
  );
  return {
    "x-chaos-timestamp": timestamp,
    "x-chaos-event-id": eventId,
    "x-chaos-signature": `${SIGNATURE_PREFIX}${signature}`,
  };
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
 * Verify a webhook signature in constant time (outbound / raw-body scheme).
 */
export function verifyWebhookSignature(
  payload: SignaturePayload,
  header: string | null | undefined,
  secret: string,
): boolean {
  if (!header || !secret) return false;
  const expected = computeWebhookSignature(payload, secret);
  const provided = normalizeSignature(header);
  if (provided.length !== expected.length) return false;
  return timingSafeEqual(
    Buffer.from(provided, "utf8"),
    Buffer.from(expected, "utf8"),
  );
}

/**
 * Verify an inbound dispatch signature (canonical scheme).
 */
export function verifyInboundDispatchSignature(
  method: string,
  path: string,
  timestamp: string,
  body: SignaturePayload,
  header: string | null | undefined,
  secret: string,
): boolean {
  if (!header || !secret) return false;
  const expected = computeInboundDispatchSignature(
    method,
    path,
    timestamp,
    body,
    secret,
  );
  const provided = header.trim();
  const hex = provided.toLowerCase().startsWith(SIGNATURE_PREFIX)
    ? provided.slice(SIGNATURE_PREFIX.length)
    : provided;
  if (hex.length !== expected.length) return false;
  return timingSafeEqual(
    Buffer.from(hex, "utf8"),
    Buffer.from(expected, "utf8"),
  );
}
