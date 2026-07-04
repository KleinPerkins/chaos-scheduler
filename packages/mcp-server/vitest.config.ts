import { defineConfig } from "vitest/config";

// Self-contained config so vitest resolves it here and does not walk up the
// directory tree to the repo-root `vite.config.ts` (which imports `vite` and
// `@vitejs/plugin-react`). In CI each package runs a standalone `npm ci`, so
// those root-only deps are absent and loading the root config would fail.
export default defineConfig({
  test: {
    include: ["test/**/*.test.ts"],
  },
});
