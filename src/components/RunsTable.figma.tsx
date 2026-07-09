import figma from "@figma/code-connect";
import RunsTable from "./RunsTable";

/**
 * Figma Code Connect mapping for the `RunsTable` component set
 * (node 415:8668, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The Figma master's only property is a presentation `Tab` variant
 * (Active / Recent / Upcoming) that switches which slice of runs is shown plus
 * the tab-bar chrome — a caller-owned filter concern with no code equivalent
 * (the code table renders whatever `runs` it is handed), so nothing maps to a
 * code prop and `props` is intentionally omitted. The primitive is driven by
 * its `runs` data seam and `onViewRun` row action; the example mirrors the
 * master's Active-tab rows, and the composed `StatusBadge` / `Button` resolve
 * through their own Code Connect mappings. Consumed by the `figma` CLI, not
 * Vite: excluded from tsconfig.app.json and ESLint so it never enters the app
 * build.
 */
figma.connect(
  RunsTable,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=415-8668",
  {
    example: () => (
      <RunsTable
        runs={[
          {
            id: "3f2a9c1b",
            workflow_id: "nightly-refresh",
            workflow_name: "nightly-refresh",
            started_at: "2026-01-01T02:00:00Z",
            finished_at: null,
            exit_code: null,
            stdout: null,
            stderr: null,
            result_url: null,
            status: "running",
            trigger_kind: "cron",
          },
          {
            id: "a91be220",
            workflow_id: "data-export",
            workflow_name: "data-export",
            started_at: "2026-01-01T01:48:00Z",
            finished_at: "2026-01-01T02:00:03Z",
            exit_code: 0,
            stdout: null,
            stderr: null,
            result_url: null,
            status: "success",
            trigger_kind: "manual",
          },
        ]}
        onViewRun={() => {}}
      />
    ),
  },
);
