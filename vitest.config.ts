import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: [
      "src/**/*.test.{ts,tsx}",
      "scripts/check-auto-merge-guard.test.mjs",
      "scripts/check-release-gating.test.mjs",
    ],
    restoreMocks: true,
  },
});
