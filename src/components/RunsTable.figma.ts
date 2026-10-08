// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=415-8668
// source=src/components/RunsTable.tsx
// component=RunsTable

import figma from "figma";

export default {
  id: "RunsTable",
  imports: [
    "import RunsTable from \"./RunsTable\""
  ],
  example: figma.code`
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
  `,
  metadata: { nestable: true },
};
