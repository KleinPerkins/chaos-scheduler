// url=https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=115-531
// source=src/components/InfoTip.tsx
// component=InfoTip

import figma from "figma";

const title = figma.selectedInstance.getString("Title");

const def = figma.selectedInstance.getString("Def");

const glossary = figma.selectedInstance.getBoolean("Glossary");

export default {
  id: "InfoTip",
  imports: [
    "import InfoTip from \"./InfoTip\""
  ],
  example: figma.code`
    <InfoTip
      ${figma.helpers.react.renderProp("title", title)}
      ${figma.helpers.react.renderProp("def", def)}
      ${figma.helpers.react.renderProp("glossary", glossary)}
      glossaryRows={[
        { term: "SLA slack", meaning: "Time until deadline breach." },
        { term: "p50", meaning: "Historical median runtime." },
      ]}
    />
  `,
  metadata: { nestable: true },
};
