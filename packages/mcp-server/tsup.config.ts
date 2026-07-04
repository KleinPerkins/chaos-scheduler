import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["src/index.ts", "src/cli.ts"],
  format: ["esm"],
  dts: true,
  sourcemap: true,
  clean: true,
  target: "node18",
  // Bundle the workspace SDK so the published server is self-contained; keep the
  // MCP SDK and zod as regular externals (declared dependencies).
  noExternal: ["@chaos-scheduler/sdk"],
  banner: { js: "" },
});
