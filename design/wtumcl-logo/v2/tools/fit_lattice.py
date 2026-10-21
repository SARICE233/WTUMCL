def kmeans_colors(vals, k, iters=80, seed=12345, restarts=12):
    """k-means with random restarts (keeps the lowest inertia solution)."""
    v = np.asarray(vals, dtype=np.float64)
    if len(v) == 0:
        return np.zeros((0, 3))
    k = min(k, len(v))
    rng = np.random.RandomState(seed)
    best = None
    for _ in range(restarts):
        c = v[rng.choice(len(v), k, replace=False)].copy()
        for _ in range(iters):
            d = ((v[:, None, :] - c[None, :, :]) ** 2).sum(axis=2)
            lab = d.argmin(axis=1)
            new = np.array([v[lab == j].mean(axis=0) if (lab == j).any() else c[j]
                            for j in range(k)])
            if np.abs(new - c).max() < 1e-9:
                c = new
                break
            c = new
        d = ((v[:, None, :] - c[None, :, :]) ** 2).sum(axis=2)
        lab = d.argmin(axis=1)
        inertia = float((((v - c[lab]) ** 2).sum(axis=1)).mean())
        if best is None or inertia < best[0]:
            best = (inertia, c, lab)
    return best[1], best[2]


# -*- coding: utf-8 -*-
"""Integer-lattice recovery of the pixel-art blocks of the WTUMCL logo.

For every block (centre sprite, "WTU", "MC") the grid is found by minimising
the sum of the within-cell luminance variance:

    E(s, x0, y0) = sum_cells var(I over cell pixels) / sum_cells #pixels

searched over s in [6, 9] (step 0.01) and one phase inside the period
(step 0.05).  Hairline pixels are excluded from the objective (they are the
only non-pixel-art content in the blocks).  Cell colours are the median RGB of
the central 50 % of the cell, then k-means clustered to at most 6 palette
entries; the black entry is the transparent "hole".

    python design/wtumcl-logo/v2/tools/fit_lattice.py            # full search
    python design/wtumcl-logo/v2/tools/fit_lattice.py --fast     # coarse pass

Writes v2/data/lattice.json
"""
import math
import os
import sys

import numpy as np

import geomlib as G

OUT = os.path.join(G.DATA, "lattice.json")
GEOM = os.path.join(G.DATA, "geometry.json")

BLOCKS = [
    # name, coarse search box, nominal canvas cells (Minecraft item icon = 16x16)
    ("sprite", (250, 250, 417, 417), 16),
    ("wtu", (270, 445, 430, 512), None),
    ("mc", (295, 495, 420, 560), None),
]

S_MIN, S_MAX, S_STEP = 6.0, 9.0, 0.01
# cells darker than this are background or dominated by a hairline stroke
INK_THR = 45.0
P_STEP, P_STEP_FAST = 0.05, 0.25


def line_mask(shape, geom, pad=3.0):
    """Mask hairline pixels (dilated) so they cannot bias the lattice fit."""
    h, w = shape
    yy, xx = np.mgrid[0:h, 0:w]
    m = np.zeros(shape, dtype=bool)
    for ln in geom["lines"]:
        x1, y1, x2, y2 = ln["x1"], ln["y1"], ln["x2"], ln["y2"]
        dx, dy = x2 - x1, y2 - y1
        L2 = dx * dx + dy * dy
        t = ((xx - x1) * dx + (yy - y1) * dy) / L2
        t = np.clip(t, 0.0, 1.0)
        d2 = (xx - (x1 + t * dx)) ** 2 + (yy - (y1 + t * dy)) ** 2
        m |= d2 <= (0.5 * ln["w"] + pad) ** 2
    return m


def integral(arr):
    z = np.zeros((arr.shape[0] + 1, arr.shape[1] + 1), dtype=np.float64)
    z[1:, 1:] = arr.cumsum(0).cumsum(1)
    return z


class Window(object):
    def __init__(self, lum, valid, box):
        x0, y0, x1, y1 = [int(v) for v in box]
        self.box = (x0, y0, x1, y1)
        sub = lum[y0:y1, x0:x1]
        v = valid[y0:y1, x0:x1].astype(np.float64)
        self.v = v
        self.I1 = integral(sub * v)
        self.I2 = integral((sub ** 2) * v)
        self.M = integral(v)
        self.h, self.w = sub.shape
        self.sub = sub

    def score(self, s, x0, y0):
        """Within-cell variance (per valid pixel) for one lattice."""
        ex = np.rint(x0 + s * np.arange(0, int((self.w - x0) / s) + 2)).astype(int)
        ey = np.rint(y0 + s * np.arange(0, int((self.h - y0) / s) + 2)).astype(int)
        ex = np.unique(np.clip(ex, 0, self.w))
        ey = np.unique(np.clip(ey, 0, self.h))
        if len(ex) < 3 or len(ey) < 3:
            return 1e18, 0, 0
        n = self.M[np.ix_(ey, ex)]
        s1 = self.I1[np.ix_(ey, ex)]
        s2 = self.I2[np.ix_(ey, ex)]
        dn = n[1:, 1:] - n[:-1, 1:] - n[1:, :-1] + n[:-1, :-1]
        d1 = s1[1:, 1:] - s1[:-1, 1:] - s1[1:, :-1] + s1[:-1, :-1]
        d2 = s2[1:, 1:] - s2[:-1, 1:] - s2[1:, :-1] + s2[:-1, :-1]
        ok = dn > 0.5
        if not ok.any():
            return 1e18, 0, 0
        nn = np.maximum(dn, 1e-9)
        var = np.where(ok, d2 / nn - (d1 / nn) ** 2, 0.0)
        return float(var.sum() / dn[ok].sum()), len(ex) - 1, len(ey) - 1


def search(win, step_phase):
    best = (1e18, None)
    phases = {}
    for s in np.arange(S_MIN, S_MAX + 1e-9, S_STEP):
        bx = (1e18, None)
        for x0 in np.arange(0.0, s, step_phase):
            # vectorise over y0
            ys = np.arange(0.0, s, step_phase)
            ey = np.rint(ys[:, None] + s * np.arange(0, int(win.h / s) + 2)[None, :]).astype(int)
            ey = np.clip(ey, 0, win.h)
            ex = np.rint(x0 + s * np.arange(0, int((win.w - x0) / s) + 2)).astype(int)
            ex = np.clip(ex, 0, win.w)
            Mx = win.M[:, ex]
            I1x = win.I1[:, ex]
            I2x = win.I2[:, ex]
            n = Mx[ey[:, :, None], np.arange(len(ex))[None, None, :]]
            # direct fancy indexing on both axes
            n = win.M[ey[:, :, None], ex[None, None, :]]
            s1 = win.I1[ey[:, :, None], ex[None, None, :]]
            s2 = win.I2[ey[:, :, None], ex[None, None, :]]
            dn = n[:, 1:, 1:] - n[:, :-1, 1:] - n[:, 1:, :-1] + n[:, :-1, :-1]
            d1 = s1[:, 1:, 1:] - s1[:, :-1, 1:] - s1[:, 1:, :-1] + s1[:, :-1, :-1]
            d2 = s2[:, 1:, 1:] - s2[:, :-1, 1:] - s2[:, 1:, :-1] + s2[:, :-1, :-1]
            ok = dn > 0.5
            nn = np.maximum(dn, 1e-9)
            var = np.where(ok, d2 / nn - (d1 / nn) ** 2, 0.0)
            tot = var.sum(axis=(1, 2))
            cnt = dn.sum(axis=(1, 2))
            sc = np.where(cnt > 0, tot / np.maximum(cnt, 1e-9), 1e18)
            j = int(np.argmin(sc))
            if sc[j] < bx[0]:
                bx = (float(sc[j]), (x0, float(ys[j])))
        phases[round(float(s), 3)] = bx
    sb = min(phases.items(), key=lambda kv: kv[1][0])
    return sb[0], sb[1][1][0], sb[1][1][1], sb[1][0], phases


def refine(win, s0, x00, y00, span=0.06):
    """Local refinement at 0.05 phase / 0.01 s resolution around a solution."""
    best = (1e18, None)
    for s in np.arange(s0 - span, s0 + span + 1e-9, S_STEP):
        for dx in np.arange(-0.3, 0.3001, P_STEP):
            for dy in np.arange(-0.3, 0.3001, P_STEP):
                x0 = (x00 + dx) % s
                y0 = (y00 + dy) % s
                sc, _, _ = win.score(s, x0, y0)
                if sc < best[0]:
                    best = (sc, (float(s), float(x0), float(y0)))
    return best


def bbox_of_content(lum, valid, box, thr=60.0):
    x0, y0, x1, y1 = [int(v) for v in box]
    sub = lum[y0:y1, x0:x1]
    m = (sub > thr) & valid[y0:y1, x0:x1]
    ys, xs = np.nonzero(m)
    if len(xs) == 0:
        raise RuntimeError("empty content box %s" % (box,))
    return (x0 + int(xs.min()), y0 + int(ys.min()),
            x0 + int(xs.max()) + 1, y0 + int(ys.max()) + 1)


def cell_median(rgb, valid, s, ox, oy, ci, cj, frac=0.25):
    """Median RGB over the central 50% of cell (ci, cj)."""
    x0 = ox + ci * s
    y0 = oy + cj * s
    xa = int(math.floor(x0 + frac * s))
    xb = int(math.ceil(x0 + (1 - frac) * s))
    ya = int(math.floor(y0 + frac * s))
    yb = int(math.ceil(y0 + (1 - frac) * s))
    xa, ya = max(xa, 0), max(ya, 0)
    xb, yb = min(xb, rgb.shape[1]), min(yb, rgb.shape[0])
    if xb <= xa or yb <= ya:
        return None, 0
    patch = rgb[ya:yb, xa:xb]
    m = valid[ya:yb, xa:xb]
    if m.sum() < 1:
        return None, 0
    vals = patch[m]
    med = np.median(vals, axis=0)
    return med, int(m.sum())


MERGE_DIST = 28.0     # RGB euclidean distance under which palette entries merge


def merge_palette(cen, lab, dist=MERGE_DIST):
    """Agglomerate palette centres closer than dist, then re-assign labels."""
    cen = [np.asarray(c, dtype=np.float64) for c in cen]
    groups = [[i] for i in range(len(cen))]
    changed = True
    while changed:
        changed = False
        for a in range(len(groups)):
            for b in range(a + 1, len(groups)):
                ca = np.mean([cen[i] for i in groups[a]], axis=0)
                cb = np.mean([cen[i] for i in groups[b]], axis=0)
                if float(np.linalg.norm(ca - cb)) < dist:
                    groups[a] = groups[a] + groups[b]
                    del groups[b]
                    changed = True
                    break
            if changed:
                break
    new_cen = np.array([np.mean([cen[i] for i in g], axis=0) for g in groups])
    lab = np.asarray(lab, dtype=int)
    for a in range(len(groups)):
        for i in groups[a]:
            lab[lab == i] = a
    # renumber for stability
    order = np.argsort(-new_cen.mean(axis=1))
    remap = np.zeros(len(new_cen), dtype=int)
    remap[order] = np.arange(len(new_cen))
    return new_cen[order], remap[lab]


def kmeans_colors(vals, k, iters=60, seed=12345):
    rng = np.random.RandomState(seed)
    v = np.asarray(vals, dtype=np.float64)
    if len(v) == 0:
        return np.zeros((0, 3))
    k = min(k, len(v))
    idx = rng.choice(len(v), k, replace=False)
    c = v[idx].copy()
    for _ in range(iters):
        d = ((v[:, None, :] - c[None, :, :]) ** 2).sum(axis=2)
        lab = d.argmin(axis=1)
        new = np.array([v[lab == j].mean(axis=0) if (lab == j).any() else c[j]
                        for j in range(k)])
        if np.abs(new - c).max() < 1e-6:
            c = new
            break
        c = new
    d = ((v[:, None, :] - c[None, :, :]) ** 2).sum(axis=2)
    lab = d.argmin(axis=1)
    return c, lab



def refine_grid(lum, valid, lines, axis, lo, hi, half=2.0, min_samples=12):
    """Sub-pixel refinement of every lattice line.

    For each nominal grid line the 50 % intensity crossing is measured on
    profiles perpendicular to the line (0.5 px sampling along the line); the
    median offset is the line's measured position.  Lines without enough
    contrast support keep a value interpolated from their neighbours.
    """
    offs = []
    counts = []
    for t in lines:
        acc = []
        for u in np.arange(lo, hi, 0.5):
            ts = np.linspace(t - half, t + half, 81)
            if axis == "v":
                prof = G.sample(lum, ts, np.full_like(ts, u))
                vmask = G.sample(valid.astype(np.float64), ts, np.full_like(ts, u)) > 0.99
            else:
                prof = G.sample(lum, np.full_like(ts, u), ts)
                vmask = G.sample(valid.astype(np.float64), np.full_like(ts, u), ts) > 0.99
            if not vmask.any():
                continue
            prof = np.where(vmask, prof, np.nan)
            if np.isnan(prof).all():
                continue
            vmin = float(np.nanmin(prof)); vmax = float(np.nanmax(prof))
            if vmax - vmin < 60.0:
                continue
            level = 0.5 * (vmin + vmax)
            s = prof - level
            idx = np.nonzero(np.diff(np.sign(s)) != 0)[0]
            found = None
            for i in idx:
                if np.isnan(s[i]) or np.isnan(s[i + 1]):
                    continue
                d = s[i + 1] - s[i]
                f = 0.0 if d == 0 else -s[i] / d
                c = ts[i] + f * (ts[i + 1] - ts[i])
                if abs(c - t) < 1.8:
                    found = c
                    break
            if found is not None:
                acc.append(found - t)
        counts.append(len(acc))
        offs.append(float(np.median(acc)) if len(acc) >= min_samples else None)
    known = [(i, o) for i, o in enumerate(offs) if o is not None]
    if not known:
        offs = [0.0] * len(lines)
    else:
        xs = np.array([k for k, _ in known], dtype=np.float64)
        ys = np.array([o for _, o in known], dtype=np.float64)
        offs = [float(np.interp(i, xs, ys)) if offs[i] is None else offs[i]
                for i in range(len(offs))]
    refined = [lines[i] + offs[i] for i in range(len(lines))]
    return refined, offs, counts


def cell_rect_median(rgb, valid, x0, y0, x1, y1, frac=0.25, min_frac=0.35):
    """Median RGB over the central part of an arbitrary (warped) cell.

    Hairline pixels are excluded when enough of the cell survives (a stroke
    crossing a cell diagonally can cover ~40 % of it and would bias the
    colour); if masking leaves less than min_frac of the cell the unmasked
    median is used, because a thin stroke never moves the median by itself.
    """
    dx = (x1 - x0); dy = (y1 - y0)
    xa = max(int(math.floor(x0 + frac * dx)), 0)
    xb = min(int(math.ceil(x1 - frac * dx)), rgb.shape[1])
    ya = max(int(math.floor(y0 + frac * dy)), 0)
    yb = min(int(math.ceil(y1 - frac * dy)), rgb.shape[0])
    if xb <= xa or yb <= ya:
        return None, 0
    patch = rgb[ya:yb, xa:xb]
    m = valid[ya:yb, xa:xb]
    if m.sum() >= min_frac * m.size:
        return np.median(patch[m], axis=0), int(m.sum())
    return np.median(patch.reshape(-1, 3), axis=0), int(patch.shape[0] * patch.shape[1])


def analyse_block(lum, rgb, valid, geom, name, coarse_box, ncells, step_phase):
    x0, y0, x1, y1 = coarse_box
    cb = bbox_of_content(lum, valid, (x0 + 8, y0 + 8, x1 - 8, y1 - 8), thr=90.0)
    win_box = (cb[0] - 6, cb[1] - 6, cb[2] + 6, cb[3] + 6)
    win = Window(lum, valid, win_box)
    s, x0p, y0p, sc, phases = search(win, step_phase)
    s, x0p, y0p = refine(win, s, x0p, y0p)[1]
    sc, ncol, nrow = win.score(s, x0p, y0p)
    abs_x0 = win_box[0] + x0p
    abs_y0 = win_box[1] + y0p
    print("[%s] content bbox %s window %s" % (name, cb, win_box))
    print("[%s] s=%.3f phase=(%.3f, %.3f) abs grid origin=(%.3f, %.3f) score=%.3f"
          % (name, s, x0p, y0p, abs_x0, abs_y0, sc))

    # grid index range covering the content box
    ci0 = int(math.floor((cb[0] - abs_x0) / s))
    ci1 = int(math.ceil((cb[2] - abs_x0) / s))
    cj0 = int(math.floor((cb[1] - abs_y0) / s))
    cj1 = int(math.ceil((cb[3] - abs_y0) / s))
    ncols = ci1 - ci0
    nrows = cj1 - cj0

    # snap to the nominal canvas size (Minecraft item icon = 16x16) when given:
    # choose the N x N cell block that contains all content cells and whose
    # centre is closest to the fitted hexagon centre.
    if ncells is not None:
        hx = (geom["hex"]["cx"] + geom["hex"]["field_cx"]) / 2.0
        hy = (geom["hex"]["cy"] + geom["hex"]["field_cy"]) / 2.0
        cand = []
        for a in range(ncells - ncols + 1):
            for b in range(ncells - nrows + 1):
                i0 = ci0 - a
                j0 = cj0 - b
                cxc = abs_x0 + (i0 + ncells / 2.0) * s
                cyc = abs_y0 + (j0 + ncells / 2.0) * s
                cand.append((math.hypot(cxc - hx, cyc - hy), i0, j0))
        cand.sort()
        _, ci0, cj0 = cand[0]
        ncols = nrows = ncells

    # ---- sub-pixel refinement of every lattice line -----------------------
    gx = [abs_x0 + ci * s for ci in range(ci0, ci0 + ncols + 1)]
    gy = [abs_y0 + cj * s for cj in range(cj0, cj0 + nrows + 1)]
    gx_ref, offs_x, cnt_x = refine_grid(lum, valid, gx, "v", cb[1] + 2.0, cb[3] - 2.0)
    gy_ref, offs_y, cnt_y = refine_grid(lum, valid, gy, "h", cb[0] + 2.0, cb[2] - 2.0)
    print("[%s] line offsets x: %s" % (name, ["%+.2f" % v for v in offs_x]))
    print("[%s] line offsets y: %s" % (name, ["%+.2f" % v for v in offs_y]))

    valid_cell = ~line_mask(lum.shape, geom, pad=1.0)
    cells = []
    for cj in range(cj0, cj0 + nrows):
        for ci in range(ci0, ci0 + ncols):
            xa = gx_ref[ci - ci0]; xb = gx_ref[ci - ci0 + 1]
            ya = gy_ref[cj - cj0]; yb = gy_ref[cj - cj0 + 1]
            med, npx = cell_rect_median(rgb, valid, xa, ya, xb, yb)
            # bright pixels that are NOT hairline pixels: real pixel-art content
            xa2 = max(int(math.floor(xa)), 0); xb2 = min(int(math.ceil(xb)), lum.shape[1])
            ya2 = max(int(math.floor(ya)), 0); yb2 = min(int(math.ceil(yb)), lum.shape[0])
            sub_l = lum[ya2:yb2, xa2:xb2]
            sub_m = valid_cell[ya2:yb2, xa2:xb2]
            area = max(sub_l.size, 1)
            content = float(((sub_l > 120.0) & sub_m).sum()) / area
            cells.append({"i": ci, "j": cj, "rgb": None if med is None else
                          [round(float(v), 2) for v in med], "n": npx,
                          "x0": round(xa, 3), "y0": round(ya, 3),
                          "x1": round(xb, 3), "y1": round(yb, 3),
                          "content": round(content, 4)})
    lum_of = np.array([-1.0 if c["rgb"] is None else
                       0.299 * c["rgb"][0] + 0.587 * c["rgb"][1] + 0.114 * c["rgb"][2]
                       for c in cells])
    ink = np.array([c["rgb"] is not None and l > INK_THR for c, l in zip(cells, lum_of)])
    print("[%s] grid %dx%d cells, %d inked (index i=%d..%d j=%d..%d)"
          % (name, ncols, nrows, int(ink.sum()), ci0, ci0 + ncols - 1,
             cj0, cj0 + nrows - 1))

    palette = []
    labels = [-1] * len(cells)
    if ink.any():
        vals = np.array([c["rgb"] for c, k in zip(cells, ink) if k])
        # choose the smallest k that is within 75 % of the previous inertia
        # (elbow rule), then merge near-duplicate centres
        ks = [2, 3, 4, 5, 6]
        best = None
        for k in ks:
            cen, lab = kmeans_colors(vals, k)
            inertia = float((((vals - cen[lab]) ** 2).sum(axis=1)).mean())
            if best is None or (best[2] > 1.0 and inertia < 0.75 * best[2]):
                best = (k, (cen, lab), inertia)
        k, (cen, lab), inertia = best
        cen, lab = merge_palette(cen, lab)
        k = len(cen)
        palette = [[int(round(min(255, max(0, v)))) for v in c] for c in cen]
        it = iter(lab.tolist())
        for n, c in enumerate(cells):
            if ink[n]:
                labels[n] = next(it)
        print("[%s] palette k=%d inertia=%.2f" % (name, k, inertia))
        for n, col in enumerate(palette):
            cnt = sum(1 for l in labels if l == n)
            print("   P%d #%02X%02X%02X  %d cells" % (n, col[0], col[1], col[2], cnt))

    grid = []
    for n, c in enumerate(cells):
        grid.append({"i": c["i"], "j": c["j"], "c": labels[n]})
        if c["rgb"] is not None and labels[n] < 0:
            pass
    return {
        "name": name,
        "s": round(s, 4),
        "x0": round(abs_x0, 4),
        "y0": round(abs_y0, 4),
        "phase_in_window": [round(x0p, 4), round(y0p, 4)],
        "window": list(win_box),
        "content_bbox": list(cb),
        "cols": ncols, "rows": nrows,
        "i0": ci0, "j0": cj0,
        "score": round(sc, 4),
        "palette": palette,
        "grid_x": [round(v, 3) for v in gx_ref],
        "grid_y": [round(v, 3) for v in gy_ref],
        "grid_x_uniform": [round(v, 3) for v in gx],
        "grid_y_uniform": [round(v, 3) for v in gy],
        "line_offsets_x": [None if v is None else round(v, 3) for v in offs_x],
        "line_offsets_y": [None if v is None else round(v, 3) for v in offs_y],
        "line_samples_x": cnt_x, "line_samples_y": cnt_y,
        "cells": grid,
        "cell_medians": [[c["i"], c["j"], c["rgb"]] for c in cells],
        "cell_boxes": [[c["i"], c["j"], c["x0"], c["y0"], c["x1"], c["y1"]] for c in cells],
        "ink_cells": int(ink.sum()),
    }


def main():
    fast = "--fast" in sys.argv
    lum = G.load_lum()
    rgb = G.load_rgb()
    geom = G.load_json(GEOM)
    valid = ~line_mask(lum.shape, geom, pad=3.0)
    # the white ring / background is not part of any pixel block
    out = {"source": os.path.basename(G.source_path()),
           "search": {"s": [S_MIN, S_MAX, S_STEP],
                      "phase_step": P_STEP_FAST if fast else P_STEP},
           "blocks": {}}
    for name, box, ncells in BLOCKS:
        out["blocks"][name] = analyse_block(
            lum, rgb, valid, geom, name, box, ncells,
            P_STEP_FAST if fast else P_STEP)
    G.save_json(OUT, out)
    print("wrote", OUT)


if __name__ == "__main__":
    main()
