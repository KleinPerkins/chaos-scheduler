# ADR 0008: Charts use bespoke in-repo SVG primitives (d3-scale/d3-shape), not a charting library

Status: accepted — 2026-08-22

## Decision

Data visualization in the desktop UI is built from bespoke in-repo SVG chart primitives
(`src/components/charts/*` — Axis, ChartTooltip, DualAxisLine, Gauge, ImpactBars, Legend, QueueLine,
RaceTrack, StatusDonut, ThresholdBand, Vehicle, plus `scales.ts`) using `d3-scale`/`d3-shape` (and
`d3-array`/`d3-time`) for math only — not a charting component library (D07).

## Why

- **Alternatives considered.** (a) A batteries-included chart library (Recharts/Chart.js/nivo) —
  rejected: heavy bundle weight for a desktop app, limited control over the exact Mission Control
  visual language (race-track, dual-axis, threshold bands), and its own theming layer that fights the
  repo's design-token + Figma Code Connect pipeline. (b) Raw SVG with no helpers — rejected: reinventing
  scales/among axes is error-prone. (c) Bespoke SVG primitives over d3 scale/shape math, bound to the
  design tokens and each mapped via Code Connect — chosen.
- **Evidence.** Every chart primitive ships with a unit test (`.test.tsx`) and a Figma Code Connect
  mapping (`.figma.tsx`); the primitives depend only on the d3 scale/shape/array/time math packages.

## Consequences

- **Enables.** Full control of the Mission Control visual language; tight coupling to design tokens and
  Code Connect; minimal dependency surface; per-primitive tests.
- **Forecloses.** No adoption of a general charting component library for these surfaces; new charts
  are built as in-repo primitives.
- **Invariant to keep true.** Charts consume `d3-*` for math only and bind to design tokens; each
  primitive keeps its unit test and Code Connect mapping.
