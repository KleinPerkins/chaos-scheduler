/**
 * Style Dictionary build for Chaos Scheduler design tokens.
 *
 * SOURCE OF TRUTH: token *values* live in `tokens/*.json` (git-durable).
 * This config owns the CSS custom-property *contract* — the exact `--var`
 * names, their order/grouping, and which colors get an rgb-triplet companion.
 *
 * Emits (via `npm run tokens`):
 *   - src/styles/tokens.css : `:root, :root[data-theme="dark"]` + `:root[data-theme="light"]`
 *   - src/styles/tokens.ts  : typed token maps + a `ThemeMode` union.
 *
 * Anti-drift guarantee: every `*-rgb` triplet is DERIVED from its source hex
 * at build time, so a hex and its rgb form can never disagree again. (This is
 * what fixed the old `--accent` / `--accent-rgb` mismatch.)
 */
import StyleDictionary from "style-dictionary";

/* ------------------------------------------------------------------ helpers */

/** "#6355e8" -> "99, 85, 232" (matches the ", "-separated CSS triplet style). */
function hexToRgbTriplet(hex) {
  const raw = String(hex).trim().replace(/^#/, "");
  const full =
    raw.length === 3
      ? raw
          .split("")
          .map((c) => c + c)
          .join("")
      : raw;
  if (!/^[0-9a-fA-F]{6}$/.test(full)) {
    throw new Error(`Cannot derive rgb triplet from non-hex value: "${hex}"`);
  }
  const r = parseInt(full.slice(0, 2), 16);
  const g = parseInt(full.slice(2, 4), 16);
  const b = parseInt(full.slice(4, 6), 16);
  return `${r}, ${g}, ${b}`;
}

/** Read a resolved token value at a dotted path, throwing if it is missing. */
function valueAt(scope, path) {
  const node = path
    .split(".")
    .reduce((acc, key) => (acc == null ? acc : acc[key]), scope);
  if (!node || node.value === undefined) {
    throw new Error(`Missing token at path: ${path}`);
  }
  return node.value;
}

/* -------------------------------------------------------------- emit plans */
/*
 * THEME_GROUPS are relative to `theme.<mode>` and emitted in BOTH the dark
 * (:root) and light blocks. Item shapes:
 *   { css, from }            -> `--css: <value>;`
 *   { css, from, rgb }       -> also `--rgb: <triplet-of-value>;` (same color)
 *   { css, from, rgbOnly }   -> `--css: <triplet-of-value>;` (triplet only)
 */
const THEME_GROUPS = [
  {
    title: "Surfaces",
    items: [
      { css: "bg-primary", from: "surface.primary", rgb: "bg-primary-rgb" },
      { css: "bg-secondary", from: "surface.secondary" },
      { css: "bg-tertiary", from: "surface.tertiary" },
      { css: "bg-hover", from: "surface.hover" },
      { css: "bg-elevated", from: "surface.elevated" },
      { css: "surface-overlay-rgb", from: "surface.overlay", rgbOnly: true },
    ],
  },
  {
    title: "Text",
    items: [
      { css: "text-primary", from: "text.primary" },
      { css: "text-secondary", from: "text.secondary" },
      { css: "text-muted", from: "text.muted" },
      { css: "text-inverse", from: "text.inverse" },
    ],
  },
  {
    title: "Lines",
    items: [
      { css: "border", from: "line.border" },
      { css: "border-strong", from: "line.strong" },
    ],
  },
  {
    title: "Accent",
    items: [
      { css: "accent", from: "accent.default", rgb: "accent-rgb" },
      { css: "accent-hover", from: "accent.hover" },
    ],
  },
  {
    title: "Status — solids",
    items: [
      { css: "success", from: "status.success.solid" },
      { css: "error", from: "status.error.solid" },
      { css: "warning", from: "status.warning.solid" },
      { css: "running", from: "status.running.solid" },
    ],
  },
  {
    title:
      "Status — tint triplets (composited with alpha for soft backgrounds)",
    items: [
      { css: "success-rgb", from: "status.success.tint", rgbOnly: true },
      { css: "error-rgb", from: "status.error.tint", rgbOnly: true },
      { css: "warning-rgb", from: "status.warning.tint", rgbOnly: true },
      { css: "running-rgb", from: "status.running.tint", rgbOnly: true },
    ],
  },
  {
    title: "Status — accessible text over soft tints",
    items: [
      { css: "success-text", from: "status.success.text" },
      { css: "error-text", from: "status.error.text" },
      { css: "warning-text", from: "status.warning.text" },
      { css: "running-text", from: "status.running.text" },
    ],
  },
  {
    title: "Neutral overlays (flip polarity between themes)",
    items: [
      { css: "elevate-rgb", from: "overlay.elevate", rgbOnly: true },
      { css: "scrim-rgb", from: "overlay.scrim", rgbOnly: true },
    ],
  },
  {
    title: "Elevation",
    items: [{ css: "shadow", from: "shadow.default" }],
  },
];

/* CONST_GROUPS are mode-agnostic and emitted once, in the :root block only. */
const CONST_GROUPS = [
  {
    title: "Radius",
    items: [
      { css: "radius", from: "radius.default" },
      { css: "radius-lg", from: "radius.lg" },
    ],
  },
  {
    title: "Typography — type scale",
    items: [
      { css: "font-size-xs", from: "font-size.xs" },
      { css: "font-size-sm", from: "font-size.sm" },
      { css: "font-size-md", from: "font-size.md" },
      { css: "font-size-lg", from: "font-size.lg" },
      { css: "font-size-xl", from: "font-size.xl" },
      { css: "font-size-2xl", from: "font-size.2xl" },
      { css: "line-height-tight", from: "line-height.tight" },
      { css: "line-height-normal", from: "line-height.normal" },
    ],
  },
  {
    title: "Typography — font family",
    items: [
      { css: "font-mono", from: "font.mono" },
      { css: "font-sans", from: "font.sans" },
    ],
  },
  {
    title: "Spacing (4px base grid)",
    items: [
      { css: "space-1", from: "space.1" },
      { css: "space-2", from: "space.2" },
      { css: "space-3", from: "space.3" },
      { css: "space-4", from: "space.4" },
      { css: "space-5", from: "space.5" },
      { css: "space-6", from: "space.6" },
      { css: "space-8", from: "space.8" },
    ],
  },
  {
    title: "Motion — durations",
    items: [
      { css: "duration-fast", from: "duration.fast" },
      { css: "duration-base", from: "duration.base" },
      { css: "duration-slow", from: "duration.slow" },
    ],
  },
  {
    title: "Motion — eases",
    items: [
      { css: "ease-standard", from: "ease.standard" },
      { css: "ease-out", from: "ease.out" },
      { css: "ease-in", from: "ease.in" },
    ],
  },
];

/* --------------------------------------------------------------- rendering */

/** Ordered [cssVar, value] pairs for a set of groups against a token scope. */
function collectEntries(scope, groups) {
  const out = [];
  for (const group of groups) {
    for (const item of group.items) {
      const value = valueAt(scope, item.from);
      if (item.rgbOnly) {
        out.push([item.css, hexToRgbTriplet(value)]);
      } else {
        out.push([item.css, value]);
        if (item.rgb) out.push([item.rgb, hexToRgbTriplet(value)]);
      }
    }
  }
  return out;
}

function renderCssGroups(scope, groups) {
  return groups
    .map((group) => {
      const lines = [];
      for (const item of group.items) {
        const value = valueAt(scope, item.from);
        if (item.rgbOnly) {
          lines.push(`  --${item.css}: ${hexToRgbTriplet(value)};`);
        } else {
          lines.push(`  --${item.css}: ${value};`);
          if (item.rgb) {
            lines.push(`  --${item.rgb}: ${hexToRgbTriplet(value)};`);
          }
        }
      }
      return `  /* ${group.title} */\n${lines.join("\n")}`;
    })
    .join("\n\n");
}

const GENERATED_HEADER = [
  "/**",
  " * DO NOT EDIT DIRECTLY — generated by Style Dictionary.",
  " * Values:   tokens/*.json",
  " * Contract: style-dictionary.config.mjs",
  " * Rebuild:  npm run tokens",
  " */",
].join("\n");

/** CSS format: one file, both theme blocks, same var names as the legacy :root. */
function formatThemedCss({ dictionary }) {
  const t = dictionary.tokens;
  const dark = t.theme.dark;
  const light = t.theme.light;

  const rootBlock = [
    ":root,",
    ':root[data-theme="dark"] {',
    "  color-scheme: dark;",
    "",
    renderCssGroups(dark, THEME_GROUPS),
    "",
    renderCssGroups(t, CONST_GROUPS),
    "}",
  ].join("\n");

  const lightBlock = [
    ':root[data-theme="light"] {',
    "  color-scheme: light;",
    "",
    renderCssGroups(light, THEME_GROUPS),
    "}",
  ].join("\n");

  return `${GENERATED_HEADER}\n\n${rootBlock}\n\n${lightBlock}\n`;
}

/** TypeScript format: typed token maps + a ThemeMode union. */
function formatTokensTs({ dictionary }) {
  const t = dictionary.tokens;
  const dark = collectEntries(t.theme.dark, THEME_GROUPS);
  const light = collectEntries(t.theme.light, THEME_GROUPS);
  const base = collectEntries(t, CONST_GROUPS);

  const toRecord = (entries, indent) =>
    entries
      .map(([name, value]) => `${indent}"${name}": ${JSON.stringify(value)},`)
      .join("\n");

  return `${GENERATED_HEADER}

export type ThemeMode = "dark" | "light";

/** Semantic color tokens, resolved per theme. Keys map 1:1 to \`--\` CSS vars. */
export const themeTokens = {
  dark: {
${toRecord(dark, "    ")}
  },
  light: {
${toRecord(light, "    ")}
  },
} as const;

/** Mode-agnostic tokens (radius, type scale, spacing, motion, fonts). */
export const baseTokens = {
${toRecord(base, "  ")}
} as const;

export type ThemeTokenName = keyof (typeof themeTokens)["dark"];
export type BaseTokenName = keyof typeof baseTokens;
export type TokenName = ThemeTokenName | BaseTokenName;
`;
}

/* ------------------------------------------------------------------ config */

export default {
  // The formats read the nested `dictionary.tokens` tree by path, so the flat
  // name-collision heuristic (e.g. two tokens both ending in `.default`) does
  // not affect output. Silence that expected, cosmetic warning.
  log: { warnings: "disabled" },
  hooks: {
    formats: {
      "css/chaos-themes": formatThemedCss,
      "typescript/chaos-tokens": formatTokensTs,
    },
  },
  source: ["tokens/**/*.json"],
  platforms: {
    web: {
      // No transforms: values are authored ready-to-emit and rgb triplets are
      // derived in the formats. References ({palette.*}) still resolve.
      transforms: [],
      buildPath: "src/styles/",
      files: [
        { destination: "tokens.css", format: "css/chaos-themes" },
        { destination: "tokens.ts", format: "typescript/chaos-tokens" },
      ],
    },
  },
};
