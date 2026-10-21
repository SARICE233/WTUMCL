# -*- coding: utf-8 -*-
"""Single geometric model for the WTUMCL v2 logo.

Coordinates live in the 664 px source bitmap space and are mapped into the
1024 px design canvas by T(); every exported number keeps 3 decimals.  The
model is built exclusively from measured data:

    data/geometry.json   hexagon centre/R + hairline centre lines + width
    data/lattice.json    centre sprite, 16x16 integer lattice + palette
    data/wordmark.json   WTU / MC rectangle decomposition

Both the SVG masters and every raster export are produced from build(), so the
vector and the bitmaps cannot drift apart.
"""
import json
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))
V2 = os.path.dirname(HERE)
DATA = os.path.join(V2, "data")

SRC = 664.0
CANVAS = 1024.0
K = CANVAS / SRC

# Ink colour measured inside the source bitmap (the AI reference is not pure
# black: the field averages RGB 3/9/8, black level L=7.3).  Using the measured
# ink keeps the exported bitmap on the source's own noise floor.
INK = "#000805"
WHITE = "#F8FDFE"

PALETTE_NORMAL = {
    "ink": INK,
    "white": WHITE,
}
PALETTE_INVERSE = {
    "paper": "#000000",
    "field": "#FFFFFF",
    "ink": "#000000",
}

# sprite palette remap for the inverse colourway (blue-grey family, as v1)
INVERSE_SPRITE = ["#6E9AA0", "#8FB6B9", "#B8CFD0"]

APOTHEM_K = math.sqrt(3.0) / 2.0


def load():
    with open(os.path.join(DATA, "geometry.json"), "r", encoding="utf-8") as f:
        geom = json.load(f)
    with open(os.path.join(DATA, "lattice.json"), "r", encoding="utf-8") as f:
        lat = json.load(f)
    with open(os.path.join(DATA, "wordmark.json"), "r", encoding="utf-8") as f:
        word = json.load(f)
    return geom, lat, word


GEOM, LAT, WORD = load()
HEX = GEOM["hex"]
CX = HEX["cx"]
CY = HEX["cy"]
HAIR_W = GEOM["hairline_width"]
SPRITE = LAT["blocks"]["sprite"]


def T(x, y):
    """source px -> 1024 design units, hexagon centre on the canvas centre."""
    return (CANVAS / 2.0 + (x - CX) * K, CANVAS / 2.0 + (y - CY) * K)


def r3(v):
    v = round(float(v) + 0.0, 3)
    return 0.0 if v == 0 else v


def P(x, y):
    a, b = T(x, y)
    return (r3(a), r3(b))


def hexagon(cx, cy, r):
    hw = APOTHEM_K * r
    return [(cx, cy - r), (cx + hw, cy - r / 2.0), (cx + hw, cy + r / 2.0),
            (cx, cy + r), (cx - hw, cy + r / 2.0), (cx - hw, cy - r / 2.0)]


def rect_poly(x0, y0, x1, y1):
    return [P(x0, y0), P(x1, y0), P(x1, y1), P(x0, y1)]


def bar_poly(x1, y1, x2, y2, w=None):
    w = HAIR_W if w is None else w
    dx, dy = x2 - x1, y2 - y1
    ln = math.hypot(dx, dy)
    if ln == 0:
        return None
    nx, ny = -dy / ln * w / 2.0, dx / ln * w / 2.0
    return [P(x1 + nx, y1 + ny), P(x2 + nx, y2 + ny),
            P(x2 - nx, y2 - ny), P(x1 - nx, y1 - ny)]


def sprite_rects():
    """16x16 lattice cells -> (polygon, palette index).

    The cells are drawn from the sub-pixel refined (warped) lattice stored in
    cell_boxes, so adjacent cells share their common grid line exactly.
    """
    boxes = {(b[0], b[1]): (b[2], b[3], b[4], b[5])
             for b in SPRITE["cell_boxes"]}
    out = []
    for c in SPRITE["cells"]:
        ci = c["c"]
        if ci < 0:
            continue
        x0, y0, x1, y1 = boxes[(c["i"], c["j"])]
        out.append((rect_poly(x0, y0, x1, y1), ci))
    return out


def wordmark_rects():
    out = []
    for name, blk in WORD["blocks"].items():
        for x0, y0, x1, y1 in blk["rects"]:
            if x1 - x0 < 0.2 or y1 - y0 < 0.2:
                continue
            out.append(rect_poly(x0, y0, x1, y1))
    return out


def build(inverse=False):
    """Z-ordered list of (fill, [polygon, ...]) in 1024 design units."""
    outer = [P(*p) for p in hexagon(HEX["cx"], HEX["cy"], HEX["R_out"])]
    field = [P(*p) for p in hexagon(HEX["field_cx"], HEX["field_cy"], HEX["R_field"])]
    bars = [bar_poly(ln["x1"], ln["y1"], ln["x2"], ln["y2"]) for ln in GEOM["lines"]]
    bars = [b for b in bars if b]
    sprite = sprite_rects()
    word = wordmark_rects()

    if not inverse:
        layers = [(WHITE, [outer]), (INK, [field])]
        for ci in range(len(SPRITE["palette"])):
            col = "#%02X%02X%02X" % tuple(SPRITE["palette"][ci])
            polys = [p for p, c in sprite if c == ci]
            if polys:
                layers.append((col, polys))
        layers.append((WHITE, bars))
        layers.append((WHITE, word))
        return layers

    layers = [("#000000", [outer]), ("#FFFFFF", [field])]
    for ci in range(len(SPRITE["palette"])):
        col = INVERSE_SPRITE[ci % len(INVERSE_SPRITE)]
        polys = [p for p, c in sprite if c == ci]
        if polys:
            layers.append((col, polys))
    layers.append(("#000000", bars))
    layers.append(("#000000", word))
    return layers


def stats():
    return {
        "layers": len(build(False)),
        "primitives": sum(len(p) for _c, p in build(False)),
        "sprite_cells": sum(1 for c in SPRITE["cells"] if c["c"] >= 0),
        "wordmark_rects": len(wordmark_rects()),
        "hairlines": len(GEOM["lines"]),
    }


if __name__ == "__main__":
    print(json.dumps(stats(), indent=1))
