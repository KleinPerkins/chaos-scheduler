import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

import figmaTokens from "../../figma-tokens.json";
import { baseTokens, themeTokens } from "./tokens";

/**
 * Consistency guard for the Figma token mirror.
 *
 * figma-tokens.json (the Figma projection) and tokens.ts (the code projection)
 * are BOTH generated from tokens/*.json by `npm run tokens`. This test fails if
 * they ever drift — a hand-edited generated artifact, or a manifest mapping that
 * loses/gains/renames a token. The hex→triplet and box-shadow derivations below
 * are re-implemented independently of style-dictionary.config.mjs so this is a
 * genuine cross-check, not a tautology.
 */

type ColorValue = { hex: string; alpha: number };
type FigmaValue = ColorValue | number | string;

interface FigmaVariable {
  name: string;
  type: "COLOR" | "FLOAT" | "STRING";
  scopes: string[];
  codeSyntax: { WEB: string };
  description?: string;
  values: Record<string, FigmaValue>;
}

interface FigmaCollection {
  name: string;
  modes: string[];
  variables: FigmaVariable[];
}

interface EffectStyle {
  name: string;
  boundColorVariable: string;
  offset: { x: number; y: number };
  radius: number;
  spread: number;
  color: Record<string, ColorValue>;
}

interface FigmaManifest {
  collections: FigmaCollection[];
  effectStyles: EffectStyle[];
}

const manifest = figmaTokens as unknown as FigmaManifest;

const dark = themeTokens.dark as Record<string, string>;
const light = themeTokens.light as Record<string, string>;
const base = baseTokens as Record<string, string>;

const themeScopes = [
  ["Dark", dark],
  ["Light", light],
] as const;

/**
 * The `*-rgb` CSS companions that are intentionally NOT separate variables:
 * they mirror a base color's hex as a triplet for `rgba(var(--x-rgb), α)`. The
 * two surface/brand companions plus the eight categorical data-viz series.
 */
const RGB_COMPANIONS: ReadonlyArray<readonly [string, string]> = [
  ["bg-primary", "bg-primary-rgb"],
  ["accent", "accent-rgb"],
  ["chart-1", "chart-1-rgb"],
  ["chart-2", "chart-2-rgb"],
  ["chart-3", "chart-3-rgb"],
  ["chart-4", "chart-4-rgb"],
  ["chart-5", "chart-5-rgb"],
  ["chart-6", "chart-6-rgb"],
  ["chart-7", "chart-7-rgb"],
  ["chart-8", "chart-8-rgb"],
];

const EXPECTED_COUNTS: Record<string, number> = {
  "cs.color": 37,
  "cs.space": 7,
  "cs.radius": 2,
  "cs.type": 10,
  "cs.motion": 6,
};

const BASE_COLLECTIONS = ["cs.space", "cs.radius", "cs.type", "cs.motion"];

function collection(name: string): FigmaCollection {
  const found = manifest.collections.find((c) => c.name === name);
  if (!found) throw new Error(`Missing collection: ${name}`);
  return found;
}

function cssName(v: FigmaVariable): string {
  return v.codeSyntax.WEB.replace(/^var\(--/, "").replace(/\)$/, "");
}

function isColor(v: FigmaValue): v is ColorValue {
  return typeof v === "object" && v !== null && "hex" in v;
}

/** "#1a1d27" -> "26, 29, 39" (independent re-impl of the config's derivation). */
function hexToTriplet(hex: string): string {
  const raw = hex.replace(/^#/, "");
  const r = parseInt(raw.slice(0, 2), 16);
  const g = parseInt(raw.slice(2, 4), 16);
  const b = parseInt(raw.slice(4, 6), 16);
  return `${r}, ${g}, ${b}`;
}

/** Extract { hex, alpha } from a CSS box-shadow's rgba(...) color. */
function parseShadowColor(shadow: string): ColorValue {
  const m = shadow.match(/rgba?\(\s*([^)]+)\)/);
  if (!m) throw new Error(`No rgba() in shadow: ${shadow}`);
  const parts = m[1].split(",").map((s) => s.trim());
  const [r, g, b] = parts.map((n) => parseInt(n, 10));
  const alpha = parts[3] === undefined ? 1 : Number(parts[3]);
  const hex = `#${[r, g, b].map((n) => n.toString(16).padStart(2, "0")).join("")}`;
  return { hex, alpha };
}

type Rgb = readonly [number, number, number];

function parseHex(hex: string): Rgb {
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as [
    number,
    number,
    number,
  ];
}

function parseTriplet(value: string): Rgb {
  return value.split(",").map(Number) as [number, number, number];
}

function composite(foreground: Rgb, background: Rgb, alpha: number): Rgb {
  return foreground.map((channel, i) =>
    Math.round(channel * alpha + background[i] * (1 - alpha)),
  ) as [number, number, number];
}

function relativeLuminance(color: Rgb): number {
  const [r, g, b] = color.map((channel) => {
    const value = channel / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrastRatio(foreground: Rgb, background: Rgb): number {
  const lighter = Math.max(
    relativeLuminance(foreground),
    relativeLuminance(background),
  );
  const darker = Math.min(
    relativeLuminance(foreground),
    relativeLuminance(background),
  );
  return (lighter + 0.05) / (darker + 0.05);
}

function colorVarByCss(name: string): FigmaVariable {
  const found = collection("cs.color").variables.find(
    (v) => cssName(v) === name,
  );
  if (!found) throw new Error(`Missing cs.color variable for --${name}`);
  return found;
}

const infoTipCss = readFileSync(
  resolve(process.cwd(), "src/components/InfoTip.css"),
  "utf8",
);

/** Body of a single CSS rule matched by EXACT selector (comments stripped). */
function cssRuleBody(css: string, selector: string): string {
  const clean = css.replace(/\/\*[\s\S]*?\*\//g, "");
  for (const chunk of clean.split("}")) {
    const brace = chunk.indexOf("{");
    if (brace === -1) continue;
    const selectors = chunk
      .slice(0, brace)
      .split(",")
      .map((s) => s.trim());
    if (selectors.includes(selector)) return chunk.slice(brace + 1);
  }
  throw new Error(`No CSS rule found for selector ${selector}`);
}

/** Resolve `<prop>: var(--x)` inside a rule body to the token name `x`. */
function boundToken(body: string, prop: string): string {
  const m = body.match(
    new RegExp(`(?:^|[;{\\s])${prop}:\\s*var\\(--([\\w-]+)\\)`),
  );
  if (!m) throw new Error(`No ${prop}: var(--…) in rule body`);
  return m[1];
}

describe("figma-tokens manifest", () => {
  it("has the expected collections, modes, and variable counts", () => {
    for (const [name, count] of Object.entries(EXPECTED_COUNTS)) {
      expect(collection(name).variables.length, `${name} count`).toBe(count);
    }
    expect(collection("cs.color").modes).toEqual(["Light", "Dark"]);
    for (const name of BASE_COLLECTIONS) {
      expect(collection(name).modes, `${name} modes`).toEqual(["Value"]);
    }
    const total = manifest.collections.reduce(
      (n, c) => n + c.variables.length,
      0,
    );
    expect(total).toBe(62);
  });

  it("every variable is well-formed (type, scopes, WEB code syntax, per-mode values)", () => {
    for (const c of manifest.collections) {
      for (const v of c.variables) {
        expect(v.name, "name").toBeTruthy();
        expect(["COLOR", "FLOAT", "STRING"], `${v.name} type`).toContain(
          v.type,
        );
        expect(Array.isArray(v.scopes), `${v.name} scopes`).toBe(true);
        expect(v.codeSyntax.WEB, `${v.name} WEB`).toMatch(/^var\(--.+\)$/);
        for (const mode of c.modes) {
          expect(v.values[mode], `${c.name}/${v.name} @ ${mode}`).toBeDefined();
        }
      }
    }
  });

  it("cs.color values match tokens.ts per theme (rgb-only tokens derive the triplet)", () => {
    for (const v of collection("cs.color").variables) {
      const name = cssName(v);
      if (name === "shadow") continue; // alpha-bearing; checked separately
      for (const [mode, scope] of themeScopes) {
        const mv = v.values[mode];
        if (!isColor(mv)) throw new Error(`${v.name} @ ${mode} is not a color`);
        expect(mv.alpha, `${v.name} @ ${mode} alpha`).toBe(1);
        if (name.endsWith("-rgb")) {
          expect(hexToTriplet(mv.hex), `${v.name} @ ${mode}`).toBe(scope[name]);
        } else {
          expect(mv.hex, `${v.name} @ ${mode}`).toBe(scope[name].toLowerCase());
        }
      }
    }
  });

  it("status text keeps WCAG AA contrast over tinted badge surfaces", () => {
    const statuses = ["success", "error", "warning", "running"] as const;
    const surfaces = [
      "bg-primary",
      "bg-secondary",
      "bg-tertiary",
      "bg-hover",
      "bg-elevated",
    ] as const;

    for (const [mode, scope] of themeScopes) {
      for (const status of statuses) {
        const text = parseHex(scope[`${status}-text`]);
        const tint = parseTriplet(scope[`${status}-rgb`]);
        for (const surface of surfaces) {
          const badge = composite(tint, parseHex(scope[surface]), 0.22);
          expect(
            contrastRatio(text, badge),
            `${status}-text on 22% ${status} tint over ${surface} @ ${mode}`,
          ).toBeGreaterThanOrEqual(4.5);
        }
      }
    }
  });

  it("keeps the InfoTip glyph at WCAG AA over its trigger surface", () => {
    // Discover the tokens the trigger ACTUALLY binds so this fails if the glyph
    // color regresses to a low-contrast token — e.g. --accent, which is only
    // ~2.82:1 on the dark --bg-tertiary surface (fails 1.4.3) — instead of
    // pinning a hard-coded pair the CSS could silently drift away from.
    const trigger = cssRuleBody(infoTipCss, ".info-tip-trigger");
    const fg = boundToken(trigger, "color");
    const bg = boundToken(trigger, "background");

    // The glyph is small text (18px circle, --font-size-xs), so AA is 4.5:1 in
    // BOTH themes, not the 3:1 large-text bar.
    for (const [mode, scope] of themeScopes) {
      expect(
        contrastRatio(parseHex(scope[fg]), parseHex(scope[bg])),
        `--${fg} on --${bg} @ ${mode}`,
      ).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("rgb companion CSS vars derive from the mirrored base color", () => {
    for (const [baseCss, companion] of RGB_COMPANIONS) {
      const v = colorVarByCss(baseCss);
      for (const [mode, scope] of themeScopes) {
        const mv = v.values[mode];
        if (!isColor(mv)) throw new Error(`${v.name} @ ${mode} is not a color`);
        expect(hexToTriplet(mv.hex), `${companion} @ ${mode}`).toBe(
          scope[companion],
        );
      }
    }
  });

  it("shadow variable and cs/shadow/default effect style match tokens.ts", () => {
    const shadowVar = colorVarByCss("shadow");
    for (const [mode, scope] of themeScopes) {
      const mv = shadowVar.values[mode];
      if (!isColor(mv)) throw new Error("shadow value is not a color");
      expect(mv, `shadow @ ${mode}`).toEqual(parseShadowColor(scope.shadow));
    }

    const effect = manifest.effectStyles.find(
      (e) => e.name === "cs/shadow/default",
    );
    if (!effect) throw new Error("Missing cs/shadow/default effect style");
    expect(effect.boundColorVariable).toBe("cs.color/shadow");
    expect(effect.offset).toEqual({ x: 0, y: 2 });
    expect(effect.radius).toBe(8);
    expect(effect.spread).toBe(0);
    expect(effect.color.Dark).toEqual(parseShadowColor(dark.shadow));
    expect(effect.color.Light).toEqual(parseShadowColor(light.shadow));
  });

  it("base collections match tokens.ts (FLOAT parsed from px/unitless, STRING verbatim)", () => {
    for (const name of BASE_COLLECTIONS) {
      for (const v of collection(name).variables) {
        const css = cssName(v);
        const expected = base[css];
        expect(expected, `${css} present in baseTokens`).toBeDefined();
        const value = v.values.Value;
        if (v.type === "FLOAT") {
          expect(value, `${v.name}`).toBe(parseFloat(expected));
        } else {
          expect(value, `${v.name}`).toBe(expected);
        }
      }
    }
  });

  it("covers every tokens.ts key exactly — no orphans, no extras", () => {
    const colorCovered = new Set<string>();
    for (const v of collection("cs.color").variables) {
      colorCovered.add(cssName(v));
    }
    for (const [, companion] of RGB_COMPANIONS) colorCovered.add(companion);
    expect(new Set(Object.keys(dark))).toEqual(colorCovered);
    expect(new Set(Object.keys(light))).toEqual(colorCovered);

    const baseCovered = new Set<string>();
    for (const name of BASE_COLLECTIONS) {
      for (const v of collection(name).variables) baseCovered.add(cssName(v));
    }
    expect(new Set(Object.keys(base))).toEqual(baseCovered);
  });
});
