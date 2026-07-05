import vectors from "../../test-fixtures/webhook-vectors.v1.json" with { type: "json" };
import { describe, expect, it } from "vitest";
import {
  computeInboundDispatchSignature,
  computeWebhookSignature,
  inboundCanonicalPayload,
  inboundDispatchHeaders,
  verifyInboundDispatchSignature,
  verifyWebhookSignature,
  webhookSignatureHeader,
} from "../src/webhook.js";

describe("webhook signatures (parity with src-tauri/src/actions.rs::sign_payload)", () => {
  for (const vector of vectors.outbound) {
    it(`outbound vector secret=${vector.secret} body=${JSON.stringify(vector.body)}`, () => {
      expect(computeWebhookSignature(vector.body, vector.secret)).toBe(
        vector.signature_hex,
      );
      expect(webhookSignatureHeader(vector.body, vector.secret)).toBe(
        `sha256=${vector.signature_hex}`,
      );
      expect(
        verifyWebhookSignature(
          vector.body,
          `sha256=${vector.signature_hex}`,
          vector.secret,
        ),
      ).toBe(true);
    });
  }

  it("rejects a tampered outbound body", () => {
    const vector = vectors.outbound[0];
    expect(
      verifyWebhookSignature(
        '{"a":2}',
        `sha256=${vector.signature_hex}`,
        vector.secret,
      ),
    ).toBe(false);
  });

  it("rejects empty header/secret", () => {
    const vector = vectors.outbound[0];
    expect(verifyWebhookSignature(vector.body, "", vector.secret)).toBe(false);
    expect(
      verifyWebhookSignature(vector.body, `sha256=${vector.signature_hex}`, ""),
    ).toBe(false);
  });

  it("accepts raw byte payloads identically to string payloads", () => {
    const vector = vectors.outbound[0];
    const bytes = new TextEncoder().encode(vector.body);
    expect(computeWebhookSignature(bytes, vector.secret)).toBe(
      vector.signature_hex,
    );
  });
});

describe("inbound dispatch signatures (parity with api.rs::inbound_canonical_payload)", () => {
  for (const vector of vectors.inbound) {
    it(`pinned inbound vector path=${vector.path}`, () => {
      expect(
        inboundCanonicalPayload(
          vector.method,
          vector.path,
          vector.timestamp,
          vector.body,
        ),
      ).toContain(vector.timestamp);
      expect(
        computeInboundDispatchSignature(
          vector.method,
          vector.path,
          vector.timestamp,
          vector.body,
          vector.secret,
        ),
      ).toBe(vector.signature_hex);
      const headers = inboundDispatchHeaders({
        method: vector.method,
        path: vector.path,
        timestamp: vector.timestamp,
        eventId: vector.event_id,
        body: vector.body,
        secret: vector.secret,
      });
      expect(headers["x-chaos-timestamp"]).toBe(vector.timestamp);
      expect(headers["x-chaos-event-id"]).toBe(vector.event_id);
      expect(headers["x-chaos-signature"]).toBe(
        `sha256=${vector.signature_hex}`,
      );
      expect(
        verifyInboundDispatchSignature(
          vector.method,
          vector.path,
          vector.timestamp,
          vector.body,
          headers["x-chaos-signature"],
          vector.secret,
        ),
      ).toBe(true);
    });
  }

  it("rejects legacy raw-body inbound signatures", () => {
    const vector = vectors.inbound[0];
    const legacy = webhookSignatureHeader(vector.body, vector.secret);
    expect(
      verifyInboundDispatchSignature(
        vector.method,
        vector.path,
        vector.timestamp,
        vector.body,
        legacy,
        vector.secret,
      ),
    ).toBe(false);
  });
});
