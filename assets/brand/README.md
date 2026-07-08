# Brand assets

## `icon-master-1024.png`

The 1024×1024 master for the Chaos Scheduler app icon: a dark squircle tile with
a glowing two-tone **orbital "8"** — a violet/magenta arc (top-right) and an
electric-blue arc (bottom-left), each with a nucleus dot (violet, blue) — on
near-black. Brand palette: indigo `#6355e8`, violet/magenta, electric blue.

**Provenance.** Derived from the operator-approved 1024×1024 render (delivered as
JPEG data with an opaque black background). Processing: the squircle tile was
isolated from the black background and the corners made transparent by fitting a
superellipse (`n≈4.54`, center ≈ (513,500), a≈439/b≈441) to the tile's detected
outline, so the tile "floats" as a proper macOS icon. No colors inside the tile
were altered.

## Regenerating the app icon set

The desktop icon set in `src-tauri/icons/` (`icon.icns`, `icon.ico`, `32x32.png`,
`128x128.png`, `128x128@2x.png`, plus the Windows `Square*Logo.png` / `StoreLogo.png`)
is generated from this master with the Tauri CLI:

```bash
npx @tauri-apps/cli icon assets/brand/icon-master-1024.png
```

This desktop app has no mobile targets, so the iOS/Android output the generator
also emits is not committed.

## Favicon / web + MCP mark

`public/favicon.svg` is a **hand-authored vector** redraw of the same orbital-8
mark (two arcs + two nucleus dots, brand gradients, subtle glow) — not a raster
embed — so it stays crisp from 16px up. The MCP server icon
(`packages/mcp-server/src/icon.ts`) is generated from that SVG via
`node scripts/gen-mcp-icon.mjs`.

## Menu-bar tray glyph

`src-tauri/icons/tray.svg` + `src-tauri/icons/tray.png` are the macOS menu-bar
**tray glyph**: a single-colour, shape-on-transparent redraw of the orbital-8
with **no squircle background**. The tray is rendered as a _template_ image
(`icon_as_template(true)` in `src-tauri/src/lib.rs`), so macOS uses only the
alpha shape and tints it to match the light/dark menu bar — the app/Dock icon is
unaffected. The glyph uses a bolder stroke and smaller nuclei than the full mark
so the ring and its nucleus stay legible at ~16–22px. `tray.png` is 44×44 (a
22pt menu-bar item at 2× Retina) and is embedded into the `TrayIconBuilder`.

The glyph geometry is derived directly from `public/favicon.svg`. Regenerate the
SVG + PNG from the mark with (requires Pillow, `pip install pillow`):

```bash
python3 scripts/gen-tray-icons.py
```
