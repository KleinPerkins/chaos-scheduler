import figma from "@figma/code-connect";
import InfoTip from "./InfoTip";

/**
 * Figma Code Connect mapping for the `InfoTip` component set
 * (node 115:531, file twQmWC8dWT4tqeqIigNsRy).
 *
 * The master's `Title` / `Def` text properties and `Glossary` boolean map 1:1
 * to the code props. The `State` variant (Rest / Hover) is a pure CSS reveal
 * state (`:hover` / `:focus-within`) with no code prop, so it is intentionally
 * left unmapped. The glossary `glossaryRows` are a code-only data seam (the
 * master shows fixed sample rows), so they are supplied directly in the example.
 * Consumed by the `figma` CLI, not Vite: excluded from tsconfig.app.json and
 * ESLint so it never enters the app build.
 */
figma.connect(
  InfoTip,
  "https://www.figma.com/design/twQmWC8dWT4tqeqIigNsRy/Chaos-Scheduler?node-id=115-531",
  {
    props: {
      title: figma.string("Title"),
      def: figma.string("Def"),
      glossary: figma.boolean("Glossary"),
    },
    example: ({ title, def, glossary }) => (
      <InfoTip
        title={title}
        def={def}
        glossary={glossary}
        glossaryRows={[
          { term: "SLA slack", meaning: "Time until deadline breach." },
          { term: "p50", meaning: "Historical median runtime." },
        ]}
      />
    ),
  },
);
