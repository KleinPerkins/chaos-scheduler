import { REDACTED_SECRET } from "./resource-projection.js";

const MAX_PATCH_BYTES = 256 * 1024;
const MAX_PATCH_DEPTH = 32;
const MAX_PATCH_NODES = 10_000;

const SENSITIVE_KEYS = new Set([
  "secret",
  "signature_secret",
  "cursor_api_key",
  "smtp_password",
]);

const OMIT = Symbol("omit-redacted-secret");

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function cloneJson(
  value: unknown,
  depth: number,
  state: { nodes: number },
): unknown {
  state.nodes += 1;
  if (depth > MAX_PATCH_DEPTH || state.nodes > MAX_PATCH_NODES) {
    throw new Error("workflow spec patch exceeds safe traversal limits");
  }
  if (Array.isArray(value)) {
    return value.map((item) => cloneJson(item, depth + 1, state));
  }
  if (isJsonObject(value)) {
    const result = Object.create(null) as Record<string, unknown>;
    for (const [key, item] of Object.entries(value)) {
      result[key] = cloneJson(item, depth + 1, state);
    }
    return result;
  }
  return value;
}

function containsSensitiveSentinel(
  value: unknown,
  key: string | undefined,
  depth = 0,
): boolean {
  if (depth > MAX_PATCH_DEPTH) {
    throw new Error("workflow spec patch exceeds safe traversal limits");
  }
  if (
    key !== undefined &&
    SENSITIVE_KEYS.has(key.toLowerCase()) &&
    value === REDACTED_SECRET
  ) {
    return true;
  }
  if (Array.isArray(value)) {
    return value.some((item) =>
      containsSensitiveSentinel(item, undefined, depth + 1),
    );
  }
  if (isJsonObject(value)) {
    return Object.entries(value).some(([childKey, item]) =>
      containsSensitiveSentinel(item, childKey, depth + 1),
    );
  }
  return false;
}

function arrayItemIdentity(value: unknown): string | undefined {
  if (!isJsonObject(value)) return undefined;
  if (typeof value.id === "string" && value.id.trim()) {
    return `id:${value.id}`;
  }
  if (
    value.type === "webhook" &&
    typeof value.url === "string" &&
    value.url.trim()
  ) {
    return `webhook:${value.url}`;
  }
  return undefined;
}

function currentArrayItemForSentinel(
  patchItem: unknown,
  currentArray: unknown[],
  usedIdentities: Set<string>,
): unknown {
  const identity = arrayItemIdentity(patchItem);
  if (!identity) {
    throw new Error(
      "cannot safely preserve a redacted secret in a replaced array item without a stable identity",
    );
  }
  if (usedIdentities.has(identity)) {
    throw new Error(
      "cannot safely preserve a redacted secret for a duplicate array identity",
    );
  }
  usedIdentities.add(identity);
  const matches = currentArray.filter(
    (currentItem) => arrayItemIdentity(currentItem) === identity,
  );
  if (matches.length !== 1) {
    throw new Error(
      "cannot safely preserve a redacted secret in a replaced array item without one matching stored item",
    );
  }
  return matches[0];
}

function restoreRedactedSecrets(
  patch: unknown,
  current: unknown,
  key: string | undefined,
  depth: number,
  state: { nodes: number },
): unknown | typeof OMIT {
  state.nodes += 1;
  if (depth > MAX_PATCH_DEPTH || state.nodes > MAX_PATCH_NODES) {
    throw new Error("workflow spec patch exceeds safe traversal limits");
  }
  if (
    key !== undefined &&
    SENSITIVE_KEYS.has(key.toLowerCase()) &&
    patch === REDACTED_SECRET
  ) {
    return current === undefined ? OMIT : cloneJson(current, depth + 1, state);
  }
  if (Array.isArray(patch)) {
    const currentArray = Array.isArray(current) ? current : [];
    const usedIdentities = new Set<string>();
    return patch.map((item) => {
      const currentItem = containsSensitiveSentinel(item, undefined)
        ? currentArrayItemForSentinel(item, currentArray, usedIdentities)
        : undefined;
      const restored = restoreRedactedSecrets(
        item,
        currentItem,
        undefined,
        depth + 1,
        state,
      );
      return restored === OMIT ? null : restored;
    });
  }
  if (isJsonObject(patch)) {
    const currentObject = isJsonObject(current) ? current : {};
    const result = Object.create(null) as Record<string, unknown>;
    for (const [childKey, item] of Object.entries(patch)) {
      const restored = restoreRedactedSecrets(
        item,
        currentObject[childKey],
        childKey,
        depth + 1,
        state,
      );
      if (restored !== OMIT) result[childKey] = restored;
    }
    return result;
  }
  return patch;
}

function mergePatch(target: unknown, patch: unknown): unknown {
  if (!isJsonObject(patch)) return patch;

  const result = Object.create(null) as Record<string, unknown>;
  if (isJsonObject(target)) {
    for (const [key, value] of Object.entries(target)) {
      result[key] = value;
    }
  }
  for (const [key, value] of Object.entries(patch)) {
    if (value === null) {
      delete result[key];
    } else {
      result[key] = mergePatch(result[key], value);
    }
  }
  return result;
}

/**
 * RFC 7396 JSON Merge Patch with one safety extension: redaction sentinels in
 * known secret fields preserve the corresponding full stored value.
 */
export function applyWorkflowSpecMergePatch(
  current: unknown,
  patch: unknown,
): unknown {
  let encoded: string;
  try {
    encoded = JSON.stringify(patch);
  } catch {
    throw new Error("workflow spec patch must be JSON-serializable");
  }
  if (
    encoded === undefined ||
    Buffer.byteLength(encoded, "utf8") > MAX_PATCH_BYTES
  ) {
    throw new Error("workflow spec patch exceeds the 256 KiB size limit");
  }

  const restored = restoreRedactedSecrets(patch, current, undefined, 0, {
    nodes: 0,
  });
  if (restored === OMIT) return current;
  return mergePatch(current, restored);
}
