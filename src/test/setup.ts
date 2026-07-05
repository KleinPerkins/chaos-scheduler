import "@testing-library/jest-dom/vitest";
import { randomFillSync, randomUUID } from "node:crypto";

Object.defineProperty(globalThis, "crypto", {
  value: {
    randomUUID,
    getRandomValues: <T extends ArrayBufferView>(buffer: T): T => {
      randomFillSync(buffer as NodeJS.ArrayBufferView);
      return buffer;
    },
  },
});
