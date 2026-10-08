// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=493-4307
// source=src/components/Modal.tsx
// component=Modal

import figma from "figma";

export default {
  id: "Modal",
  imports: [
    "import Modal from \"./Modal\""
  ],
  example: figma.code`
    <Modal
      onClose={() => {}}
      labelledBy="rerun-title"
      describedBy="rerun-desc"
    >
      <h2 id="rerun-title">Re-run workflow</h2>
      <p id="rerun-desc">This enqueues a new run with the same parameters.</p>
    </Modal>
  `,
  metadata: { nestable: true },
};
