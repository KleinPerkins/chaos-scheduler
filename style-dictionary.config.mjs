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
 *   - figma-tokens.json     : a one-way (repo → Figma) mirror manifest of the
 *                             same tokens as Figma variable collections
 *                             (`cs.*`), consumed by the Figma sync (MCP/REST).
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

/** "#0F1117" | "#f17" -> "#0f1117" (lowercase, expanded). For Figma color values. */
function normalizeHex(hex) {
  const raw = String(hex).trim().replace(/^#/, "");
  const full =
    raw.length === 3
      ? raw
          .split("")
          .map((c) => c + c)
          .join("")
      : raw;
  if (!/^[0-9a-fA-F]{6}$/.test(full)) {
    throw new Error(`Not a hex color: "${hex}"`);
  }
  return `#${full.toLowerCase()}`;
}

/**
 * Parse a CSS box-shadow ("0 2px 8px rgba(15, 23, 42, 0.12)") into the pieces
 * Figma needs for an effect style + a mode-aware bound color. Figma has no
 * composite-shadow variable, so `shadow` becomes an effect style whose color is
 * the only alpha-bearing color value in the mirror.
 */
function parseBoxShadow(value) {
  const m = String(value)
    .trim()
    .match(
      /^(-?\d+(?:\.\d+)?)(?:px)?\s+(-?\d+(?:\.\d+)?)(?:px)?\s+(-?\d+(?:\.\d+)?)(?:px)?(?:\s+(-?\d+(?:\.\d+)?)(?:px)?)?\s+rgba?\(\s*([^)]+)\)$/,
    );
  if (!m) {
    throw new Error(`Cannot parse box-shadow: "${value}"`);
  }
  const parts = m[5].split(",").map((s) => s.trim());
  const [r, g, b] = parts.map((n) => parseInt(n, 10));
  const alpha = parts[3] === undefined ? 1 : Number(parts[3]);
  const hex = `#${[r, g, b].map((n) => n.toString(16).padStart(2, "0")).join("")}`;
  return {
    offset: { x: Number(m[1]), y: Number(m[2]) },
    radius: Number(m[3]),
    spread: m[4] === undefined ? 0 : Number(m[4]),
    color: { hex, alpha },
  };
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

/* --------------------------------------------------------- figma manifest */
/*
 * figma-tokens.json is a one-way (repo → Figma) mirror: the same tokens,
 * projected as Figma variable collections (`cs.*`). The repo stays the SOURCE
 * OF TRUTH; nothing here ever reads Figma.
 *
 * THEME_GROUPS / CONST_GROUPS remain the single iteration source (shared with
 * the CSS/TS formats). The Figma-only metadata — collection, grouped variable
 * name, resolved variable type, and picker scopes — is layered on per token via
 * FIGMA_COLOR_META / FIGMA_BASE_META, keyed by the same `css` name used
 * everywhere else. WEB code syntax is pinned to the exact `--var` (the keystone
 * that makes Figma Dev Mode show the code's variable name).
 *
 * Value shapes: COLOR -> { hex, alpha }; FLOAT -> number; STRING -> string.
 * The `*-rgb` CSS companions (bg-primary-rgb, accent-rgb) are NOT separate Figma
 * variables — they are the same color, so the mirror carries the color once.
 */

const FIGMA_COLOR_COLLECTION = "cs.color";
const FIGMA_SHADOW_EFFECT_STYLE = "cs/shadow/default";
const FIGMA_FILE_KEY_HINT = "twQmWC8dWT4tqeqIigNsRy";

/** cs.color: keyed by `css` -> { name (Figma slash path), scopes, description? }. */
const FIGMA_COLOR_META = {
  "bg-primary": { name: "bg/primary", scopes: ["FRAME_FILL", "SHAPE_FILL"] },
  "bg-secondary": {
    name: "bg/secondary",
    scopes: ["FRAME_FILL", "SHAPE_FILL"],
  },
  "bg-tertiary": { name: "bg/tertiary", scopes: ["FRAME_FILL", "SHAPE_FILL"] },
  "bg-hover": { name: "bg/hover", scopes: ["FRAME_FILL", "SHAPE_FILL"] },
  "bg-elevated": { name: "bg/elevated", scopes: ["FRAME_FILL", "SHAPE_FILL"] },
  "surface-overlay-rgb": {
    name: "surface/overlay",
    scopes: ["FRAME_FILL", "SHAPE_FILL"],
    description:
      "Overlay surface color. Consumed in code as the --surface-overlay-rgb triplet, composited with alpha.",
  },
  "text-primary": { name: "text/primary", scopes: ["TEXT_FILL"] },
  "text-secondary": { name: "text/secondary", scopes: ["TEXT_FILL"] },
  "text-muted": { name: "text/muted", scopes: ["TEXT_FILL"] },
  "text-inverse": { name: "text/inverse", scopes: ["TEXT_FILL"] },
  border: { name: "border", scopes: ["STROKE_COLOR"] },
  "border-strong": { name: "border-strong", scopes: ["STROKE_COLOR"] },
  accent: {
    name: "accent/default",
    scopes: ["FRAME_FILL", "SHAPE_FILL", "STROKE_COLOR"],
  },
  "accent-hover": {
    name: "accent/hover",
    scopes: ["FRAME_FILL", "SHAPE_FILL", "STROKE_COLOR"],
  },
  success: {
    name: "status/success/solid",
    scopes: ["SHAPE_FILL", "STROKE_COLOR", "TEXT_FILL"],
  },
  error: {
    name: "status/error/solid",
    scopes: ["SHAPE_FILL", "STROKE_COLOR", "TEXT_FILL"],
  },
  warning: {
    name: "status/warning/solid",
    scopes: ["SHAPE_FILL", "STROKE_COLOR", "TEXT_FILL"],
  },
  running: {
    name: "status/running/solid",
    scopes: ["SHAPE_FILL", "STROKE_COLOR", "TEXT_FILL"],
  },
  "success-rgb": {
    name: "status/success/tint",
    scopes: ["FRAME_FILL", "SHAPE_FILL"],
    description:
      "Soft status background. Consumed in code as the --success-rgb triplet, composited with alpha.",
  },
  "error-rgb": {
    name: "status/error/tint",
    scopes: ["FRAME_FILL", "SHAPE_FILL"],
    description:
      "Soft status background. Consumed in code as the --error-rgb triplet, composited with alpha.",
  },
  "warning-rgb": {
    name: "status/warning/tint",
    scopes: ["FRAME_FILL", "SHAPE_FILL"],
    description:
      "Soft status background. Consumed in code as the --warning-rgb triplet, composited with alpha.",
  },
  "running-rgb": {
    name: "status/running/tint",
    scopes: ["FRAME_FILL", "SHAPE_FILL"],
    description:
      "Soft status background. Consumed in code as the --running-rgb triplet, composited with alpha.",
  },
  "success-text": { name: "status/success/text", scopes: ["TEXT_FILL"] },
  "error-text": { name: "status/error/text", scopes: ["TEXT_FILL"] },
  "warning-text": { name: "status/warning/text", scopes: ["TEXT_FILL"] },
  "running-text": { name: "status/running/text", scopes: ["TEXT_FILL"] },
  "elevate-rgb": {
    name: "overlay/elevate",
    scopes: ["FRAME_FILL", "SHAPE_FILL"],
    description:
      "Neutral elevate overlay (flips per theme). Consumed in code as the --elevate-rgb triplet, composited with alpha.",
  },
  "scrim-rgb": {
    name: "overlay/scrim",
    scopes: ["FRAME_FILL", "SHAPE_FILL"],
    description:
      "Scrim overlay. Consumed in code as the --scrim-rgb triplet, composited with alpha.",
  },
  shadow: {
    name: "shadow",
    scopes: ["EFFECT_COLOR"],
    description:
      "Shadow color bound into the cs/shadow/default effect style. NOTE: --shadow is the full box-shadow string in code; this variable carries only its color.",
  },
};

/** Mode-agnostic collections: keyed by `css` -> { collection, name, type, scopes }. */
const FIGMA_BASE_META = {
  radius: {
    collection: "cs.radius",
    name: "default",
    type: "FLOAT",
    scopes: ["CORNER_RADIUS"],
  },
  "radius-lg": {
    collection: "cs.radius",
    name: "lg",
    type: "FLOAT",
    scopes: ["CORNER_RADIUS"],
  },
  "font-size-xs": {
    collection: "cs.type",
    name: "size/xs",
    type: "FLOAT",
    scopes: ["FONT_SIZE"],
  },
  "font-size-sm": {
    collection: "cs.type",
    name: "size/sm",
    type: "FLOAT",
    scopes: ["FONT_SIZE"],
  },
  "font-size-md": {
    collection: "cs.type",
    name: "size/md",
    type: "FLOAT",
    scopes: ["FONT_SIZE"],
  },
  "font-size-lg": {
    collection: "cs.type",
    name: "size/lg",
    type: "FLOAT",
    scopes: ["FONT_SIZE"],
  },
  "font-size-xl": {
    collection: "cs.type",
    name: "size/xl",
    type: "FLOAT",
    scopes: ["FONT_SIZE"],
  },
  "font-size-2xl": {
    collection: "cs.type",
    name: "size/2xl",
    type: "FLOAT",
    scopes: ["FONT_SIZE"],
  },
  "line-height-tight": {
    collection: "cs.type",
    name: "line-height/tight",
    type: "FLOAT",
    scopes: ["LINE_HEIGHT"],
  },
  "line-height-normal": {
    collection: "cs.type",
    name: "line-height/normal",
    type: "FLOAT",
    scopes: ["LINE_HEIGHT"],
  },
  "font-mono": {
    collection: "cs.type",
    name: "family/mono",
    type: "STRING",
    scopes: ["FONT_FAMILY"],
  },
  "font-sans": {
    collection: "cs.type",
    name: "family/sans",
    type: "STRING",
    scopes: ["FONT_FAMILY"],
  },
  "space-1": {
    collection: "cs.space",
    name: "1",
    type: "FLOAT",
    scopes: ["GAP"],
  },
  "space-2": {
    collection: "cs.space",
    name: "2",
    type: "FLOAT",
    scopes: ["GAP"],
  },
  "space-3": {
    collection: "cs.space",
    name: "3",
    type: "FLOAT",
    scopes: ["GAP"],
  },
  "space-4": {
    collection: "cs.space",
    name: "4",
    type: "FLOAT",
    scopes: ["GAP"],
  },
  "space-5": {
    collection: "cs.space",
    name: "5",
    type: "FLOAT",
    scopes: ["GAP"],
  },
  "space-6": {
    collection: "cs.space",
    name: "6",
    type: "FLOAT",
    scopes: ["GAP"],
  },
  "space-8": {
    collection: "cs.space",
    name: "8",
    type: "FLOAT",
    scopes: ["GAP"],
  },
  "duration-fast": {
    collection: "cs.motion",
    name: "duration/fast",
    type: "STRING",
    scopes: [],
  },
  "duration-base": {
    collection: "cs.motion",
    name: "duration/base",
    type: "STRING",
    scopes: [],
  },
  "duration-slow": {
    collection: "cs.motion",
    name: "duration/slow",
    type: "STRING",
    scopes: [],
  },
  "ease-standard": {
    collection: "cs.motion",
    name: "ease/standard",
    type: "STRING",
    scopes: [],
  },
  "ease-out": {
    collection: "cs.motion",
    name: "ease/out",
    type: "STRING",
    scopes: [],
  },
  "ease-in": {
    collection: "cs.motion",
    name: "ease/in",
    type: "STRING",
    scopes: [],
  },
};

/** Emit order of the mode-agnostic collections. */
const FIGMA_BASE_COLLECTION_ORDER = [
  "cs.space",
  "cs.radius",
  "cs.type",
  "cs.motion",
];

/** A cs.color value ({hex, alpha}) for one theme scope. `shadow` carries alpha. */
function figmaColorValue(scope, item) {
  if (item.css === "shadow") {
    return parseBoxShadow(valueAt(scope, item.from)).color;
  }
  return { hex: normalizeHex(valueAt(scope, item.from)), alpha: 1 };
}

/** Build the Figma mirror manifest from the resolved token tree. */
function buildFigmaManifest(tokens) {
  const dark = tokens.theme.dark;
  const light = tokens.theme.light;

  // cs.color — semantic colors, Light/Dark modes.
  const colorVariables = [];
  for (const group of THEME_GROUPS) {
    for (const item of group.items) {
      const meta = FIGMA_COLOR_META[item.css];
      if (!meta) {
        throw new Error(`No Figma color metadata for token "${item.css}"`);
      }
      const variable = {
        name: meta.name,
        type: "COLOR",
        scopes: meta.scopes,
        codeSyntax: { WEB: `var(--${item.css})` },
        values: {
          Light: figmaColorValue(light, item),
          Dark: figmaColorValue(dark, item),
        },
      };
      if (meta.description) variable.description = meta.description;
      colorVariables.push(variable);
    }
  }

  // Mode-agnostic collections — single "Value" mode.
  const baseBuckets = new Map(
    FIGMA_BASE_COLLECTION_ORDER.map((name) => [name, []]),
  );
  for (const group of CONST_GROUPS) {
    for (const item of group.items) {
      const meta = FIGMA_BASE_META[item.css];
      if (!meta) {
        throw new Error(`No Figma base metadata for token "${item.css}"`);
      }
      const raw = valueAt(tokens, item.from);
      const value = meta.type === "FLOAT" ? parseFloat(raw) : raw;
      if (meta.type === "FLOAT" && Number.isNaN(value)) {
        throw new Error(`FLOAT token "${item.css}" is not numeric: "${raw}"`);
      }
      const bucket = baseBuckets.get(meta.collection);
      if (!bucket) {
        throw new Error(
          `Unknown collection "${meta.collection}" for "${item.css}"`,
        );
      }
      bucket.push({
        name: meta.name,
        type: meta.type,
        scopes: meta.scopes,
        codeSyntax: { WEB: `var(--${item.css})` },
        values: { Value: value },
      });
    }
  }

  const collections = [
    {
      name: FIGMA_COLOR_COLLECTION,
      modes: ["Light", "Dark"],
      variables: colorVariables,
    },
    ...FIGMA_BASE_COLLECTION_ORDER.map((name) => ({
      name,
      modes: ["Value"],
      variables: baseBuckets.get(name),
    })),
  ];

  // Shadow is composite → an effect style (not a variable). Geometry is
  // mode-agnostic (identical across themes); color is mode-aware via the bound
  // cs.color/shadow variable created above.
  const darkShadow = parseBoxShadow(valueAt(dark, "shadow.default"));
  const lightShadow = parseBoxShadow(valueAt(light, "shadow.default"));
  const effectStyles = [
    {
      name: FIGMA_SHADOW_EFFECT_STYLE,
      type: "DROP_SHADOW",
      boundColorVariable: `${FIGMA_COLOR_COLLECTION}/shadow`,
      offset: darkShadow.offset,
      radius: darkShadow.radius,
      spread: darkShadow.spread,
      color: { Light: lightShadow.color, Dark: darkShadow.color },
    },
  ];

  return {
    $comment:
      "DO NOT EDIT DIRECTLY — generated by Style Dictionary (npm run tokens). Values: tokens/*.json · Contract: style-dictionary.config.mjs. One-way mirror: repo → Figma (cs.* variables); the repo is the source of truth.",
    fileKeyHint: FIGMA_FILE_KEY_HINT,
    collections,
    effectStyles,
  };
}

/** JSON format: the Figma mirror manifest (prettier re-formats it afterward). */
function formatFigmaManifest({ dictionary }) {
  return `${JSON.stringify(buildFigmaManifest(dictionary.tokens), null, 2)}\n`;
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
      "json/figma-manifest": formatFigmaManifest,
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
    // Figma mirror manifest lives at the repo root — it is a cross-tool contract
    // for the Figma sync, not an app style asset.
    figma: {
      transforms: [],
      buildPath: "./",
      files: [
        { destination: "figma-tokens.json", format: "json/figma-manifest" },
      ],
    },
  },
};
