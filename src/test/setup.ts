import "@testing-library/jest-dom/vitest";
import { randomFillSync } from "node:crypto";

Object.defineProperty(globalThis, "crypto", {
  value: {
    getRandomValues: <T extends ArrayBufferView>(buffer: T): T => {
      randomFillSync(buffer as NodeJS.ArrayBufferView);
      return buffer;
    },
  },
});
