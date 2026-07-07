import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["src/index.ts", "src/cli.ts"],
  format: ["esm"],
  dts: true,
  sourcemap: true,
  clean: true,
  target: "node18",
  // Bundle every *third-party* runtime dependency so the published server
  // ships with zero third-party npm footprint: an unpinned transitive dep (a
  // compromised patch to @modelcontextprotocol/sdk or zod, say) can never be
  // silently pulled onto a user's machine by the app's own
  // re-provision/npm-install flow, since there is nothing left for npm to
  // resolve at install time.
  //
  // @chaos-scheduler/sdk is deliberately NOT in this list, even though it's
  // a `file:../sdk-ts` dependency locally: it's our own package, published
  // and versioned independently (see the sdk-ts -> mcp-server release-
  // ordering gate in release.yml / docs/RELEASING.md), and `publish-mcp`
  // rewrites this dependency to the real published semver before `npm
  // publish`. Bundling it here would freeze whatever SDK code happened to be
  // on disk at mcp-server's own build time into every future install,
  // silently defeating that release-ordering gate — an SDK-only hotfix
  // republish would never actually reach a user who already has mcp-server
  // installed, even though npm would dutifully (and pointlessly) fetch the
  // newer, unused SDK into node_modules alongside the stale bundled copy.
  noExternal: ["@modelcontextprotocol/sdk", "zod"],
  banner: { js: "" },
});
