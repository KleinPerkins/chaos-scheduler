import figma from "@figma/code-connect";
import Modal from "./Modal";

/**
 * Figma Code Connect mapping for the `Modal` component set
 * (node 493:4307, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master's `Size` (Sm / Md / Lg) and `Footer` (With / Without) variants are
 * presentation-only shell states with no code prop (the primitive is a
 * class-less structure + behavior shell driven by its `onClose` / `children`
 * and passthrough `*ClassName` shell props), so nothing maps to a code prop and
 * `props` is intentionally omitted. The example shows representative shell usage
 * (a labelled dialog body). Consumed by the `figma` CLI, not Vite: excluded
 * from tsconfig.app.json and ESLint so it never enters the app build.
 */
figma.connect(
  Modal,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=493-4307",
  {
    example: () => (
      <Modal
        onClose={() => {}}
        labelledBy="rerun-title"
        describedBy="rerun-desc"
      >
        <h2 id="rerun-title">Re-run workflow</h2>
        <p id="rerun-desc">This enqueues a new run with the same parameters.</p>
      </Modal>
    ),
  },
);
