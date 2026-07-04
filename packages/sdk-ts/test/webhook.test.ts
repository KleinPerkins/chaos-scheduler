import { describe, expect, it } from "vitest";
import {
  computeWebhookSignature,
  verifyWebhookSignature,
  webhookSignatureHeader,
} from "../src/webhook.js";

describe("webhook signatures (parity with src-tauri/src/actions.rs::sign_payload)", () => {
  // Cross-implementation vector: identical inputs to the backend unit test
  // `hmac_signature_is_stable_and_hex` (`sign_payload("topsecret", b"{\"a\":1}")`).
  // The hex below was produced by both Node's crypto and the Rust HMAC-SHA256.
  const VECTOR = {
    secret: "topsecret",
    body: '{"a":1}',
    hex: "bf1e6501b7fa928ec2391fea9dd90af3c9ad1b7b1ef6ff319c25940cec746bf8",
  };

  it("computes the exact backend HMAC-SHA256 hex", () => {
    expect(computeWebhookSignature(VECTOR.body, VECTOR.secret)).toBe(
      VECTOR.hex,
    );
  });

  it("emits the `sha256=` header the backend sends", () => {
    expect(webhookSignatureHeader(VECTOR.body, VECTOR.secret)).toBe(
      `sha256=${VECTOR.hex}`,
    );
  });

  it("verifies a header with the sha256= prefix", () => {
    expect(
      verifyWebhookSignature(
        VECTOR.body,
        `sha256=${VECTOR.hex}`,
        VECTOR.secret,
      ),
    ).toBe(true);
  });

  it("verifies a bare hex header (no prefix)", () => {
    expect(verifyWebhookSignature(VECTOR.body, VECTOR.hex, VECTOR.secret)).toBe(
      true,
    );
  });

  it("verifies case-insensitively on the hex digest", () => {
    expect(
      verifyWebhookSignature(
        VECTOR.body,
        `sha256=${VECTOR.hex.toUpperCase()}`,
        VECTOR.secret,
      ),
    ).toBe(true);
  });

  it("rejects a tampered body", () => {
    expect(
      verifyWebhookSignature('{"a":2}', `sha256=${VECTOR.hex}`, VECTOR.secret),
    ).toBe(false);
  });

  it("rejects a wrong secret", () => {
    expect(
      verifyWebhookSignature(VECTOR.body, `sha256=${VECTOR.hex}`, "nope"),
    ).toBe(false);
  });

  it("rejects a malformed / short signature without throwing", () => {
    expect(
      verifyWebhookSignature(VECTOR.body, "sha256=deadbeef", VECTOR.secret),
    ).toBe(false);
  });

  it("rejects empty header/secret", () => {
    expect(verifyWebhookSignature(VECTOR.body, "", VECTOR.secret)).toBe(false);
    expect(
      verifyWebhookSignature(VECTOR.body, `sha256=${VECTOR.hex}`, ""),
    ).toBe(false);
    expect(verifyWebhookSignature(VECTOR.body, null, VECTOR.secret)).toBe(
      false,
    );
  });

  it("accepts raw byte payloads identically to string payloads", () => {
    const bytes = new TextEncoder().encode(VECTOR.body);
    expect(computeWebhookSignature(bytes, VECTOR.secret)).toBe(VECTOR.hex);
    expect(
      verifyWebhookSignature(bytes, `sha256=${VECTOR.hex}`, VECTOR.secret),
    ).toBe(true);
  });

  it("second cross-impl vector (chaos-secret / hello)", () => {
    expect(computeWebhookSignature("hello", "chaos-secret")).toBe(
      "65b8731ae4e6c79e1ee19671cfc4561fe6e8c81d45b3172d83c8312457b4890e",
    );
  });
});
