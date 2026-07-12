# Chaos Scheduler — Design System

The visual foundation for the desktop app: **design tokens**, the **theme model**,
**fonts**, **motion**, and the shared UI primitives. This document is the entry
point; the machine-readable source of truth is the `tokens/` directory.

## Source of truth

- **Token _values_ live in `tokens/*.json`** (git-durable). This is the SOT.
- **The CSS custom-property _contract_** (exact `--var` names, ordering, and which
  colors get an rgb-triplet companion) lives in `style-dictionary.config.mjs`.
- **Generated, do-not-edit outputs** (committed for reviewability + a working
  bare `vite build`):
  - `src/styles/tokens.css` — the `:root` (dark, default) + `:root[data-theme="light"]` blocks.
  - `src/styles/tokens.ts` — typed token maps (`themeTokens`, `baseTokens`) + a `ThemeMode` union.
  - `figma-tokens.json` (repo root) — the one-way repo→Figma mirror manifest that
    projects the same tokens as Figma `cs.*` variable collections.

**Repo is the source of truth; Figma mirrors it.** Token values are authored here
and mirrored one-way into a Figma `cs.*` variable collection (pinned to Dark) — never
the reverse. The code never reads from Figma at build time. A required CI freshness
check (the `tokens` job) regenerates all three outputs and fails the build if any
drift from `tokens/*.json`.

## Build step

```bash
npm run tokens   # style-dictionary build → prettier --write on the two outputs
```

`tokens` runs automatically at the front of both `npm run build` and `npm run dev`,
so the generated files are always fresh. Re-run it by hand after editing anything
under `tokens/` or the emit plan in `style-dictionary.config.mjs`. The build is
deterministic and prettier-formatted, so committing the outputs produces no churn.

## Token tiers

| Tier               | Source file(s)                         | CSS vars                                                       | Mode-specific? |
| ------------------ | -------------------------------------- | -------------------------------------------------------------- | -------------- |
| Base palette       | `color.palette.json`                   | _(primitives; referenced by the theme tiers)_                  | no             |
| Semantic color     | `theme.dark.json` / `theme.light.json` | `--bg-*`, `--text-*`, `--border*`, `--accent*`, `--success*` … | **yes**        |
| Elevation          | `theme.dark.json` / `theme.light.json` | `--shadow`                                                     | **yes**        |
| Radius             | `radius.json`                          | `--radius`, `--radius-lg`                                      | no             |
| Spacing _(new)_    | `spacing.json`                         | `--space-1` … `--space-8` (4px grid)                           | no             |
| Type scale _(new)_ | `typography.json`                      | `--font-size-xs` … `--font-size-2xl`, `--line-height-*`        | no             |
| Motion _(new)_     | `motion.json`                          | `--duration-fast\|base\|slow`, `--ease-standard\|out\|in`      | no             |
| Font family        | `font.json`                            | `--font-sans`, `--font-mono`                                   | no             |

The base palette → semantic split means shared hues (brand accent, status tints)
are declared once as primitives and referenced by the per-mode semantic tokens.
The `theme.*.json` files _are_ the semantic layer; because semantic values differ
per mode, each mode gets its own file rather than a separate abstract layer.

## Theme model

- `data-theme` is set on `<html>` by `src/lib/theme.ts` (`initTheme()` runs
  pre-render in `src/main.tsx`). Default is **dark**; `light` and `system` are supported.
- `tokens.css` emits **dark under `:root, :root[data-theme="dark"]`** (so dark is
  the default even with no attribute) and **light overrides under `:root[data-theme="light"]`**.
- Mode-agnostic tiers (radius, spacing, type, motion, fonts) are emitted **once**
  in the root block and inherited by both themes.
- CSS import order in `src/main.tsx`: `fonts.css` → `tokens.css` → `index.css`
  (tokens before the globals that consume them).

## Color model — hex + derived rgb (no drift)

Some colors need both a hex form (`--accent: #6355e8`) and an rgb-triplet form
(`--accent-rgb: 99, 85, 232`) so they can be composited with `rgba(var(--x-rgb), α)`
for soft, alpha-tinted backgrounds.

**Every `*-rgb` triplet is derived from its source hex at build time.** A hex and
its triplet can never disagree again. This fixed a real bug where the hand-maintained
`--accent` / `--accent-rgb` had drifted apart in both themes:

- dark `--accent-rgb` `124, 108, 255` → **`99, 85, 232`** (now derived from `#6355e8`)
- light `--accent-rgb` `99, 85, 232` → **`86, 70, 214`** (now derived from `#5646d6`)

Note that a status **solid** (e.g. `--success`, used for text/dots) and its
**tint** (`--success-rgb`, used for backgrounds) are intentionally _different_
colors — the tint is a more saturated hue. They are separate tokens, and each rgb
triplet is still derived from its own hex.

## UI primitive inventory

Shared, tokenized primitives live in `src/index.css` (globals only — no token
`:root` blocks):

- **Buttons**: `.btn`, `.btn-primary`, `.btn-ghost`, `.btn-danger`, `.btn-sm`
- **Status badges**: `.status-badge` + state modifiers (`.success`, `.failed`,
  `.running`, `.queued`, `.poll_exhausted`, `.stale`, …) with shape-coded `::before` glyphs
- **Status dots**: `.status-dot` + the same state modifiers
- **Base elements**: resets, `input`/`select`/`textarea`, focus-visible rings, scrollbars
- **Reduced motion**: a `prefers-reduced-motion: reduce` reset that neutralizes all
  transitions/animations globally (kept intentionally — motion tokens are opt-in and
  this reset still overrides them)

Per-component styles live next to each component as `src/components/<Name>.css`.

## Fonts — self-hosted Inter (OFL)

- Inter is self-hosted via the **`@fontsource/inter`** package (SIL Open Font
  License — safe to commit / ship in a public repo). Its woff2 files are bundled
  into `dist/` by Vite, so the offline Tauri build needs no network.
- `src/styles/fonts.css` wires the `@font-face` declarations for the weights the UI
  actually uses: **400 (Regular), 500 (Medium), 600 (Semibold)**, Latin subset.
- `--font-sans` leads with `"Inter"` and falls back to the original system stack
  (`-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif`) so nothing
  breaks if a face fails to load.

**Intentional near-match substitution:** the **code ships Inter**, while the
**Figma design uses Calibre** (commercial / Klim — cannot be committed to a public
repo). Inter is a deliberate, licensed open near-match for Calibre — they are paired
on purpose, not by accident. If Calibre is ever licensed for the app, swap the
`font.sans` token + `fonts.css` faces; nothing else needs to change.

## Motion policy

- Durations: `--duration-fast` (0.15s, the legacy transition speed), `--duration-base`
  (0.2s), `--duration-slow` (0.3s).
- Eases: `--ease-standard`, `--ease-out`, `--ease-in`.
- Adopt incrementally. Obvious literals (e.g. the `0.15s` transitions in
  `index.css`) have been swapped to `--duration-fast`; the remaining per-component
  transitions can migrate opportunistically. The global reduced-motion reset always wins.

## Design ↔ code integration (shipped)

- **Figma `cs.*` mirror — LIVE.** The generated `figma-tokens.json` is the one-way
  repo→Figma manifest; its `cs.*` variable collections (pinned to Dark) mirror the
  token values so design and code share names. Repo stays the SOT — code never reads
  Figma. `.github/workflows/figma-variables-sync.yml` runs the one-way sync on token
  changes.
- **Code Connect — LIVE (not deferred).** Figma components are mapped to the React
  primitives above via source-tracked `src/**/*.figma.tsx` files wired through the
  root `figma.config.json` (react parser). `.github/workflows/figma-code-connect.yml`
  publishes the mappings to the Figma team library on every push to `main` (PRs get a
  `--dry-run` validate-only pass); a credential-free `code-connect` job in the
  `ci-required` fan-in type-checks and parses every mapping on the PR. See
  **`design/divergence-ledger.md`** for the per-component mapping status (18+ live
  mappings) plus the full screen/component/design-revision divergence ledger.

### Verification mechanisms (live vs pending)

Design↔code parity is verified by three gates; be precise about which are live:

- **Token projection + CI diff-fail (the G02 mechanism) — LIVE.** The `tokens` job
  (in the `ci-required` fan-in) regenerates `tokens.css`, `tokens.ts`, and
  `figma-tokens.json` and fails on any drift, so a token change can never merge with
  stale generated output.
- **Protected live-Figma token readback (the G03 mechanism) — pending.** Reading the
  live Figma file back to confirm the mirror applied is not yet automated.
- **Descendant `cs.*` binding / remote-dependency audit (the G04 mechanism) —
  pending.** The one-time plugin/API audit that every master binds only to `cs.*`
  and carries no remote-component dependency has not run (the ledger's rows carry
  `pending G04 audit`).

## Follow-ups (open)

- **Pending asset**: none for fonts — Inter ships via npm. (The earlier Calibre
  self-host plan is dropped now that the repo is public.)
- **Wider token adoption**: migrate remaining hard-coded spacing/type/motion literals
  in per-component CSS to the new scales, case by case (low priority, low risk).
