#!/usr/bin/env python
# -*- coding: utf-8 -*-
r"""Extract pixel data (mascot sprite grid + wordmark block rectangles) from the
WTUMCL source bitmap and persist it as tools/pixeldata.json.

Run once (the generated JSON is committed next to this script so the SVG/PNG
builders never need the original bitmap):

    python design/wtumcl-logo/tools/extract_source.py "F:\Desktop\WTUMC-AI-VEDIO\WTUMCLOGO.png"
"""
import json, os, sys
from PIL import Image
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "pixeldata.json")

DEFAULT_SRC = r"F:\Desktop\WTUMC-AI-VEDIO\WTUMCLOGO.png"

# --- mascot sprite grid (source pixel units) -------------------------------
M_PITCH = 3.4
M_OX, M_OY = 265.0, 265.0
M_CELLS = 40          # samples a 136x136 px box; out-of-sprite cells are dropped

# --- wordmark band ---------------------------------------------------------
W_BOX = (283, 453, 385, 532)   # x0,y0,x1,y1 (starts below the X diagonals)
W_THR = 140


def load(path):
    im = Image.open(path).convert("RGB")
    a = np.asarray(im).astype(int)
    lum = 0.299 * a[:, :, 0] + 0.587 * a[:, :, 1] + 0.114 * a[:, :, 2]
    blueness = a[:, :, 2] - a[:, :, 0]
    return a, lum, blueness


def _open(mask, n=1):
    """binary opening: kills the 2 px hairlines, keeps the sprite blocks."""
    m = mask.copy()
    for _ in range(n):
        m = (m & np.roll(m, 1, 0) & np.roll(m, -1, 0)
             & np.roll(m, 1, 1) & np.roll(m, -1, 1))
    for _ in range(n):
        m = (m | np.roll(m, 1, 0) | np.roll(m, -1, 0)
             | np.roll(m, 1, 1) | np.roll(m, -1, 1))
    return m


def mascot_grid(lum, blueness):
    light = lum > 140
    solid = _open(light, 1)
    cells = []
    for j in range(M_CELLS):
        for i in range(M_CELLS):
            x0 = M_OX + i * M_PITCH
            y0 = M_OY + j * M_PITCH
            xs = slice(int(round(x0 + M_PITCH * 0.15)), int(round(x0 + M_PITCH * 0.85)))
            ys = slice(int(round(y0 + M_PITCH * 0.15)), int(round(y0 + M_PITCH * 0.85)))
            lb = lum[ys, xs]
            bb = blueness[ys, xs]
            if solid[ys, xs].mean() < 0.5:
                continue
            white = ((lb > 165) & (bb < 10)).sum()
            blue = ((bb >= 12) & (lb > 140)).sum()
            cls = "w" if white > blue else "a"
            cells.append([i, j, cls])
    return cells


def wordmark_rects(lum):
    x0, y0, x1, y1 = W_BOX
    rows = []
    for y in range(y0, y1):
        r, st = [], None
        for x in range(x0, x1):
            v = lum[y, x] > W_THR
            if v and st is None:
                st = x
            elif not v and st is not None:
                r.append((st, x - 1)); st = None
        if st is not None:
            r.append((st, x1 - 1))
        rows.append(r)
    # merge consecutive rows whose run sets match within 1px -> one band
    bands = []
    for idx, r in enumerate(rows):
        y = y0 + idx
        if not r:
            continue
        if bands and bands[-1]["last"] == y - 1 and len(bands[-1]["runs"]) == len(r) \
                and all(abs(p - q) <= 1 and abs(u - v) <= 1
                        for (p, u), (q, v) in zip(bands[-1]["runs"], r)):
            b = bands[-1]
            b["runs"] = [(int(round((p + q) / 2.)), int(round((u + v) / 2.)))
                         for (p, u), (q, v) in zip(b["runs"], r)]
            b["last"] = y
        else:
            bands.append({"runs": list(r), "ys": [y], "last": y})
    rects = []
    for b in bands:
        ya, yb = b["ys"][0], b["last"] + 1
        for (a, c) in b["runs"]:
            rects.append([a, ya, c + 1, yb])
    return rects


def main():
    src = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_SRC
    a, lum, blueness = load(src)
    data = {
        "source": os.path.basename(src),
        "source_size": [int(a.shape[1]), int(a.shape[0])],
        "mascot": {"pitch": M_PITCH, "ox": M_OX, "oy": M_OY,
                   "cells": mascot_grid(lum, blueness)},
        "wordmark": {"rects": wordmark_rects(lum)},
    }
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(data, f, separators=(",", ":"))
    print("mascot cells:", len(data["mascot"]["cells"]))
    print("wordmark rects:", len(data["wordmark"]["rects"]))
    print("wrote", OUT, os.path.getsize(OUT), "bytes")


if __name__ == "__main__":
    main()
