#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Single geometric model for the WTUMCL logo.

All coordinates below are expressed in the source bitmap coordinate system
(664 x 664, measured from the WTUMCLOGO.png reference) and are mapped into the
1024 x 1024 design canvas by T().

Both the SVG master and every raster render are produced from the primitive
list returned by build(), so vector and bitmap can never drift apart.
"""
import json, math, os

HERE = os.path.dirname(os.path.abspath(__file__))
PIXELDATA = os.path.join(HERE, "pixeldata.json")

# ---------------------------------------------------------------- geometry --
SRC = 664.0
CX, CY = 333.5, 333.0      # hexagon centre in the source bitmap
R_OUT = 312.3              # outer hexagon circumradius (measured 312.3)
R_IN = 226.9               # inner black hexagon circumradius (measured 226.9)
HAIR = 2.0                 # hairline width

CANVAS = 1024.0
K = CANVAS / SRC

PALETTE = {
    "ink": "#000000",
    "white": "#FFFFFF",
    "accent": "#B8CFD0",       # measured mascot blue (184,207,208)
    "accent_dark": "#6E9AA0",  # inverse only, same colour family
}


def T(x, y):
    """source px -> 1024 design units, hexagon centred on the canvas."""
    return (CANVAS / 2.0 + (x - CX) * K, CANVAS / 2.0 + (y - CY) * K)


def _hexagon(cx, cy, r):
    """pointy-top regular hexagon, vertices clockwise from the top one."""
    hw = math.sqrt(3.0) / 2.0 * r
    return [(cx, cy - r), (cx + hw, cy - r / 2.0), (cx + hw, cy + r / 2.0),
            (cx, cy + r), (cx - hw, cy + r / 2.0), (cx - hw, cy - r / 2.0)]


def _poly(pts):
    return [(round(p[0], 3), round(p[1], 3)) for p in pts]


def _bar(x1, y1, x2, y2, w=HAIR):
    """bar with butt caps -> 4 point polygon."""
    dx, dy = x2 - x1, y2 - y1
    ln = math.hypot(dx, dy)
    if ln == 0:
        return None
    nx, ny = -dy / ln * w / 2.0, dx / ln * w / 2.0
    return [(x1 + nx, y1 + ny), (x2 + nx, y2 + ny),
            (x2 - nx, y2 - ny), (x1 - nx, y1 - ny)]


def _rect(x0, y0, x1, y1):
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


# ---------------------------------------------------------------- hairlines --
def hairlines():
    """Measured hairline end points, source space."""
    y_h = 343.5                      # horizontal rail
    x_l, x_r = 194.5, 472.5          # long verticals
    x_ul, x_ur = 264.5, 402.5        # short chandelier verticals
    y_top_l = 186.4                  # x=194.5 meets the inner hexagon
    y_top_u = 145.9                  # x=264.5 meets the inner hexagon
    y_cup = 262.0                    # chandelier apex
    y_cross = 423.8                  # X crossing
    x_low_l, x_low_r = 241.5, 425.5  # basket verticals
    y_basket = 465.0
    y_basket_end = 502.0
    segs = [
        (CX, CY - R_OUT, CX, CY + R_OUT),
        (CX - 0.5 * math.sqrt(3) * R_IN, y_h, CX + 0.5 * math.sqrt(3) * R_IN, y_h),
        (x_l, y_top_l, x_l, y_h),
        (x_r, y_top_l, x_r, y_h),
        (x_ul, y_top_u, x_ul, 228.0),
        (x_ur, y_top_u, x_ur, 228.0),
        (x_ul, 228.0, CX, y_cup),
        (x_ur, 228.0, CX, y_cup),
        (x_l, y_h, CX, y_cross),
        (x_r, y_h, CX, y_cross),
        (CX, y_cross, x_low_l, y_basket),
        (CX, y_cross, x_low_r, y_basket),
        (x_low_l, y_basket, x_low_l, y_basket_end),
        (x_low_r, y_basket, x_low_r, y_basket_end),
    ]
    return [_bar(*s) for s in segs]


# ------------------------------------------------------------------- pixels --
def _pixeldata():
    with open(PIXELDATA, "r", encoding="utf-8") as f:
        return json.load(f)


def mascot_rects():
    """Merge 3.4 px sprite cells into horizontal runs (fewer elements)."""
    d = _pixeldata()["mascot"]
    p, ox, oy = d["pitch"], d["ox"], d["oy"]
    rows = {}
    for i, j, cls in d["cells"]:
        rows.setdefault(j, []).append((i, cls))
    out = []
    for j in sorted(rows.keys()):
        cells = sorted(rows[j])
        start, cls, prev = cells[0][0], cells[0][1], cells[0][0]
        for i, c in cells[1:]:
            if i == prev + 1 and c == cls:
                prev = i
                continue
            out.append((start, j, prev, cls))
            start, cls, prev = i, c, i
        out.append((start, j, prev, cls))
    rects = []
    for i0, j, i1, cls in out:
        rects.append((ox + i0 * p, oy + j * p, ox + (i1 + 1) * p, oy + (j + 1) * p, cls))
    return rects


def wordmark_rects():
    return [tuple(r) for r in _pixeldata()["wordmark"]["rects"]]


# -------------------------------------------------------------------- build --
def build(inverse=False):
    """Return a z-ordered list of (colour key, [polygon, ...])."""
    outer = _poly([T(*p) for p in _hexagon(CX, CY, R_OUT)])
    inner = _poly([T(*p) for p in _hexagon(CX, CY, R_IN)])
    lines = [_poly([T(*p) for p in ln]) for ln in hairlines() if ln]
    mascot = mascot_rects()
    word = wordmark_rects()

    m_rects = [_poly(_rect(T(x0, y0)[0], T(x0, y0)[1], T(x1, y1)[0], T(x1, y1)[1]))
               for x0, y0, x1, y1, _c in mascot]
    m_cls = [c for _x0, _y0, _x1, _y1, c in mascot]
    w_rects = [_poly(_rect(T(x0, y0)[0], T(x0, y0)[1], T(x1, y1)[0], T(x1, y1)[1]))
               for x0, y0, x1, y1 in word]

    accent_m = [r for r, c in zip(m_rects, m_cls) if c == "a"]
    white_m = [r for r, c in zip(m_rects, m_cls) if c == "w"]

    if not inverse:
        return [
            ("white", [outer]),
            ("ink", [inner]),
            ("white", lines),
            ("white", w_rects),
            ("white", white_m),
            ("accent", accent_m),
        ]

    bg = _poly([(0.0, 0.0), (CANVAS, 0.0), (CANVAS, CANVAS), (0.0, CANVAS)])
    return [
        ("white", [bg]),
        ("ink", [outer]),
        ("white", [inner]),
        ("ink", lines),
        ("ink", w_rects),
        ("accent_dark", white_m),
        ("accent", accent_m),
    ]


MEASUREMENTS = {
    "src_size": 664,
    "outer_bbox": (63, 21, 604, 645),
    "R_out": R_OUT,
    "R_in": R_IN,
    "rim": R_OUT - R_IN,
    "centre": (CX, CY),
    "hairline_px": HAIR,
}


if __name__ == "__main__":
    for name, inv in (("normal", False), ("inverse", True)):
        layers = build(inv)
        print(name, "layers", len(layers), "prims", sum(len(p) for _c, p in layers))
