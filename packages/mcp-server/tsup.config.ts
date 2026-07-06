import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["src/index.ts", "src/cli.ts"],
  format: ["esm"],
  dts: true,
  sourcemap: true,
  clean: true,
  target: "node18",
  // Bundle every runtime dependency so the published server ships with zero
  // runtime npm dependencies: an unpinned transitive dep (a compromised
  // patch to @modelcontextprotocol/sdk or zod, say) can never be silently
  // pulled onto a user's machine by the app's own re-provision/npm-install
  // flow, since there is nothing left for npm to resolve at install time.
  noExternal: ["@chaos-scheduler/sdk", "@modelcontextprotocol/sdk", "zod"],
  banner: { js: "" },
});
