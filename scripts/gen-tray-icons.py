#!/usr/bin/env python3
"""Regenerate the monochrome macOS menu-bar tray glyph from the app brand mark.

The macOS tray is rendered as a *template* image (``icon_as_template(true)`` in
``src-tauri/src/lib.rs``): AppKit uses only the ALPHA channel and paints it with
the menu-bar's content colour (dark on a light bar, light on a dark bar). Passing
the full app icon there renders as a solid filled silhouette, so the tray needs a
dedicated shape-on-transparent glyph.

This script derives that glyph from the hand-authored vector mark in
``public/favicon.svg`` (the "orbital-8": two arcs meeting at a waist + two nucleus
dots). It parses the arc endpoints / radius and the two nucleus centres straight
out of the SVG, then re-draws them in a single opaque colour on a transparent
background with tray-tuned thickness (bolder stroke + smaller nuclei so the ring
and its nucleus stay legible at ~16-22px). It writes:

  * ``src-tauri/icons/tray.svg`` — monochrome vector source (for future edits)
  * ``src-tauri/icons/tray.png`` — 44x44 RGBA8 raster wired into the TrayIconBuilder
    (44px == a 22pt menu-bar item at 2x Retina; AppKit downscales cleanly at 1x)

Run from the repo root (requires Pillow):

    python3 scripts/gen-tray-icons.py
"""
from __future__ import annotations

import math
import re
from pathlib import Path

from PIL import Image, ImageDraw

REPO_ROOT = Path(__file__).resolve().parent.parent
FAVICON = REPO_ROOT / "public" / "favicon.svg"
OUT_SVG = REPO_ROOT / "src-tauri" / "icons" / "tray.svg"
OUT_PNG = REPO_ROOT / "src-tauri" / "icons" / "tray.png"

# --- Tray-specific overrides (the brand mark's own stroke=40 / dot r=34 are too
# thin/large-nucleus to read at menu-bar sizes; these keep a bold ring with a
# clear gap around a distinct nucleus down to ~16px). Colour is irrelevant for a
# template image — only the alpha shape matters — so we draw pure black. ---
STROKE = 60.0        # arc stroke width, in the favicon's 512 user-space units
DOT_R = 38.0         # nucleus radius, same units
PAD_FRAC = 0.09      # transparent padding on each side of the square canvas
PNG_SIZE = 44        # exported raster edge (22pt menu-bar item @2x Retina)
SS = 4               # supersample factor for the raster (render then downscale)


def parse_mark(svg_text: str):
    """Extract (p0, p1, radius, [dot_centres]) from the favicon's orbital-8."""
    arc = re.search(
        r'd="M\s*([\d.]+)[ ,]+([\d.]+)\s+A\s*([\d.]+)[ ,]+[\d.]+\s+[\d.]+\s+'
        r"[01]\s+[01]\s+([\d.]+)[ ,]+([\d.]+)",
        svg_text,
    )
    if not arc:
        raise SystemExit("could not parse an arc path from public/favicon.svg")
    x0, y0, r, x1, y1 = (float(arc.group(i)) for i in range(1, 6))
    dots = [
        (float(cx), float(cy))
        for cx, cy in re.findall(r'<circle[^>]*cx="([\d.]+)"[^>]*cy="([\d.]+)"', svg_text)
    ]
    if len(dots) < 2:
        raise SystemExit("expected two nucleus <circle>s in public/favicon.svg")
    return (x0, y0), (x1, y1), r, dots[:2]


def arc_center(x0, y0, r, large, sweep, x1, y1):
    """SVG endpoint -> center parametrization (rx == ry == r, no rotation)."""
    x1p, y1p = (x0 - x1) / 2.0, (y0 - y1) / 2.0
    rx = ry = abs(r)
    lam = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry)
    if lam > 1:
        s = math.sqrt(lam)
        rx *= s
        ry *= s
    num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p
    den = rx * rx * y1p * y1p + ry * ry * x1p * x1p
    co = math.sqrt(max(0.0, num / den))
    if large == sweep:
        co = -co
    cxp, cyp = co * (rx * y1p / ry), co * (-ry * x1p / rx)
    cx, cy = cxp + (x0 + x1) / 2.0, cyp + (y0 + y1) / 2.0

    def ang(ux, uy, vx, vy):
        a = math.acos(max(-1.0, min(1.0, (ux * vx + uy * vy) / (math.hypot(ux, uy) * math.hypot(vx, vy)))))
        return -a if (ux * vy - uy * vx) < 0 else a

    t1 = ang(1, 0, (x1p - cxp) / rx, (y1p - cyp) / ry)
    dt = ang((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry)
    if not sweep and dt > 0:
        dt -= 2 * math.pi
    if sweep and dt < 0:
        dt += 2 * math.pi
    return cx, cy, rx, ry, t1, dt


def arc_pts(p0, p1, r, n=1800):
    cx, cy, rx, ry, t1, dt = arc_center(p0[0], p0[1], r, 1, 1, p1[0], p1[1])
    return [(cx + rx * math.cos(t1 + dt * i / n), cy + ry * math.sin(t1 + dt * i / n)) for i in range(n + 1)]


def render(p0, p1, r, dots):
    """Draw the glyph big (disk-stamped arcs -> round caps), return (img, viewbox)."""
    big = 512 * SS
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    hs = STROKE / 2.0 * SS
    black = (0, 0, 0, 255)
    for a, b in ((p0, p1), (p1, p0)):
        for x, y in arc_pts(a, b, r):
            X, Y = x * SS, y * SS
            d.ellipse([X - hs, Y - hs, X + hs, Y + hs], fill=black)
    for cx, cy in dots:
        X, Y, rr = cx * SS, cy * SS, DOT_R * SS
        d.ellipse([X - rr, Y - rr, X + rr, Y + rr], fill=black)

    minx, miny, maxx, maxy = img.getbbox()
    w, h = maxx - minx, maxy - miny
    content = max(w, h)
    side = int(round(content / (1 - 2 * PAD_FRAC)))
    square = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    square.paste(img.crop((minx, miny, maxx, maxy)), ((side - w) // 2, (side - h) // 2))

    vb_side = content / SS / (1 - 2 * PAD_FRAC)
    cx512, cy512 = (minx + maxx) / 2.0 / SS, (miny + maxy) / 2.0 / SS
    viewbox = (cx512 - vb_side / 2.0, cy512 - vb_side / 2.0, vb_side, vb_side)
    return square, viewbox


def write_svg(p0, p1, r, dots, viewbox):
    vx, vy, vs, _ = viewbox
    f = lambda v: f"{v:.2f}".rstrip("0").rstrip(".")
    OUT_SVG.write_text(
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{PNG_SIZE}" height="{PNG_SIZE}"'
        f' viewBox="{f(vx)} {f(vy)} {f(vs)} {f(vs)}" fill="none"'
        f' role="img" aria-label="Chaos Scheduler">\n'
        f"  <!-- GENERATED by scripts/gen-tray-icons.py from public/favicon.svg."
        f" Monochrome menu-bar template glyph; edit the source or tunables and regenerate. -->\n"
        f'  <path d="M{f(p0[0])} {f(p0[1])} A{f(r)} {f(r)} 0 1 1 {f(p1[0])} {f(p1[1])}"'
        f' stroke="#000" stroke-width="{f(STROKE)}" stroke-linecap="round" />\n'
        f'  <path d="M{f(p1[0])} {f(p1[1])} A{f(r)} {f(r)} 0 1 1 {f(p0[0])} {f(p0[1])}"'
        f' stroke="#000" stroke-width="{f(STROKE)}" stroke-linecap="round" />\n'
        f'  <circle cx="{f(dots[0][0])}" cy="{f(dots[0][1])}" r="{f(DOT_R)}" fill="#000" />\n'
        f'  <circle cx="{f(dots[1][0])}" cy="{f(dots[1][1])}" r="{f(DOT_R)}" fill="#000" />\n'
        f"</svg>\n"
    )


def main():
    p0, p1, r, dots = parse_mark(FAVICON.read_text())
    square, viewbox = render(p0, p1, r, dots)
    png = square.resize((PNG_SIZE, PNG_SIZE), Image.LANCZOS)
    png.save(OUT_PNG)
    write_svg(p0, p1, r, dots, viewbox)
    print(f"favicon geometry: p0={p0} p1={p1} r={r} dots={dots}")
    print(f"tray: stroke={STROKE} dot_r={DOT_R} viewBox={tuple(round(v, 2) for v in viewbox)}")
    print(f"wrote {OUT_PNG.relative_to(REPO_ROOT)} ({png.size[0]}x{png.size[1]} RGBA)")
    print(f"wrote {OUT_SVG.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
