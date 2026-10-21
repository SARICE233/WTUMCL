# -*- coding: utf-8 -*-
"""Sub-pixel vectorisation of the "WTU" / "MC" wordmark.

The wordmark is white on black, axis aligned, so it is recovered as a set of
rectangles: the binary mask (50 % of the local white level) is decomposed into
maximal horizontal bands of identical runs and every rectangle edge is then
refined to the sub-pixel 50 % intensity crossing measured on the source
bitmap.  The lattice analysis of the same blocks is reported separately by
fit_lattice.py.

    python design/wtumcl-logo/v2/tools/fit_wordmark.py

Writes v2/data/wordmark.json
"""
import os
import sys

import numpy as np

import geomlib as G

OUT = os.path.join(G.DATA, "wordmark.json")
BOXES = {
    "wtu": (283, 451, 385, 486),
    "mc": (297, 492, 372, 527),
}
RUN_TOL = 1.5      # px tolerance when merging the runs of consecutive rows
THR = 127.0


def runs_of(row):
    out, start = [], None
    for i, v in enumerate(row):
        if v and start is None:
            start = i
        elif not v and start is not None:
            out.append((start, i))
            start = None
    if start is not None:
        out.append((start, len(row)))
    return tuple(out)


def edge_cross(prof, coords, level, rising):
    """Sub-pixel crossing of level along coords (1-D), nearest to the middle."""
    s = prof - level
    idx = np.nonzero(np.diff(np.sign(s)) != 0)[0]
    if len(idx) == 0:
        return None
    if rising:
        idx = [i for i in idx if s[i] < 0 <= s[i + 1]]
    else:
        idx = [i for i in idx if s[i] >= 0 > s[i + 1]]
    if not idx:
        return None
    i = int(idx[0])
    d = s[i + 1] - s[i]
    f = 0.0 if d == 0 else -s[i] / d
    return float(coords[i] + f * (coords[i + 1] - coords[i]))


def band_bounds(lum, band, xruns, level):
    """Sub-pixel top/bottom of a band, averaged over all its runs."""
    y0, y1 = band
    tops, bots = [], []
    ts = np.linspace(y0 - 4.0, y1 + 4.0, 121)
    for (a, b) in xruns:
        xm = 0.5 * (a + b)
        prof = G.sample(lum, np.full_like(ts, xm), ts)
        hi = y0 + 0.55 * (y1 - y0)
        lo = y0 + 0.45 * (y1 - y0)
        top = edge_cross(prof[ts <= hi], ts[ts <= hi], level, rising=True)
        bot = edge_cross(prof[ts >= lo], ts[ts >= lo], level, rising=False)
        if top is not None:
            tops.append(top)
        if bot is not None:
            bots.append(bot)
    return (float(np.median(tops)) if tops else float(y0),
            float(np.median(bots)) if bots else float(y1))


def run_edges(lum, band, xrun, level):
    """Sub-pixel left/right of one run, averaged over the band rows."""
    y0, y1 = band
    ys = np.linspace(y0 + 0.3 * (y1 - y0), y1 - 0.3 * (y1 - y0), 7)
    a, b = xrun
    xs = np.linspace(a - 4.0, b + 4.0, 161)
    lefts, rights = [], []
    hi = a + 0.55 * (b - a)
    lo = a + 0.45 * (b - a)
    for y in ys:
        prof = G.sample(lum, xs, np.full_like(xs, y))
        l = edge_cross(prof[xs <= hi], xs[xs <= hi], level, rising=True)
        r = edge_cross(prof[xs >= lo], xs[xs >= lo], level, rising=False)
        if l is not None:
            lefts.append(l)
        if r is not None:
            rights.append(r)
    return (float(np.median(lefts)) if lefts else float(a),
            float(np.median(rights)) if rights else float(b))


def extract(lum, box):
    """Band/run decomposition with a px tolerance, then sub-pixel edges."""
    x0, y0, x1, y1 = box
    sub = lum[y0:y1, x0:x1]
    mask = sub > THR
    # one background / foreground level for the whole block: a band that lies
    # inside a thick stroke is mostly ink, so a per-band percentile would be
    # meaningless and would push the measured edges outwards.
    bg = float(np.percentile(sub, 5.0))
    fg = float(np.percentile(sub, 95.0))
    level = 0.5 * (bg + fg)
    rows = [runs_of(mask[j]) for j in range(sub.shape[0])]
    bands = []
    for j, r in enumerate(rows):
        if not r:
            continue
        if bands:
            b = bands[-1]
            same = (b["rows"][-1] == j - 1 and len(b["runs"]) == len(r)
                    and all(abs(p[0] - q[0]) <= RUN_TOL and abs(p[1] - q[1]) <= RUN_TOL
                            for p, q in zip(b["runs"], r)))
            if same:
                b["rows"].append(j)
                b["runs"] = [((p[0] + q[0]) * 0.5, (p[1] + q[1]) * 0.5)
                             for p, q in zip(b["runs"], r)]
                continue
        bands.append({"rows": [j], "runs": [tuple(map(float, rr)) for rr in r]})
    rects = []
    for b in bands:
        ya = y0 + b["rows"][0]
        yb = y0 + b["rows"][-1] + 1
        # level is fixed per block (see above)
        top, bot = band_bounds(lum, (ya, yb),
                               [(x0 + int(round(a)), x0 + int(round(c))) for a, c in b["runs"]],
                               level)
        for (a, c) in b["runs"]:
            l, r = run_edges(lum, (ya, yb), (x0 + int(round(a)), x0 + int(round(c))), level)
            rects.append([round(l, 3), round(top, 3), round(r, 3), round(bot, 3)])
    return rects


def main():
    lum = G.load_lum()
    out = {"source": os.path.basename(G.source_path()), "thr": THR, "blocks": {}}
    for name, box in BOXES.items():
        rects = extract(lum, box)
        out["blocks"][name] = {"box": list(box), "rects": rects}
        print("%s: %d rectangles in %s" % (name, len(rects), box))
        for r in rects:
            print("   [%.2f, %.2f, %.2f, %.2f]" % tuple(r))
    G.save_json(OUT, out)
    print("wrote", OUT)


if __name__ == "__main__":
    main()
