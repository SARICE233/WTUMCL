# -*- coding: utf-8 -*-
"""Sub-pixel geometry fit for the WTUMCL logo (source bitmap space, 664 px).

* regular pointy-top hexagon (outer edge of the white ring, inner edge of the
  ring = black field) fitted to 50% intensity crossings with a linear
  interpolation on a 0.05 px sampling grid;
* every hairline fitted as a total-least-squares line to sub-pixel centre
  crossings, endpoints resolved by junction intersection / hexagon clipping /
  50% fall-off walk; one common stroke width.

    python design/wtumcl-logo/v2/tools/fit_geometry.py

Writes v2/data/geometry.json.
"""
import math
import os
import sys

import numpy as np

import geomlib as G

OUT = os.path.join(G.DATA, "geometry.json")

# approximate starting values (v1 measurements, only used to bracket the scan)
APPROX_C = (333.5, 333.0)
APPROX_R_OUT = 312.3
APPROX_R_FIELD = 226.9
APOTHEM_K = math.sqrt(3.0) / 2.0


def _normals(phi):
    return np.array([[math.cos(math.radians(60 * k) + phi),
                      math.sin(math.radians(60 * k) + phi)] for k in range(6)])


def edge_points(lum, c, apothem, phi, half_span, n_par, n_prof, crossing_up):
    """Collect sub-pixel edge points around a regular hexagon.

    c          : approximate centre (x, y)
    apothem    : approximate centre -> edge distance
    phi        : hexagon rotation (radians)
    half_span  : tangential half extent of the sampled edge section
    crossing_up: True when the transition is dark -> bright along the outward
                 normal, False for bright -> dark.
    """
    ns = _normals(phi)
    pts, edge_idx = [], []
    ts = np.linspace(-9.0, 9.0, n_prof)
    for k in range(6):
        n = ns[k]
        tv = np.array([-n[1], n[0]])
        us = np.linspace(-half_span, half_span, 35)
        for u in us:
            # walk from inside (t = apothem - 6) to outside (+6)
            for direc in (1.0, -1.0):
                t = apothem + direc * ts
                p = (c[0] + t * n[0] + u * tv[0], c[1] + t * n[1] + u * tv[1])
                prof = G.sample(lum, p[0], p[1])
                lo, hi = float(prof.min()), float(prof.max())
                if hi - lo < 40.0:
                    continue
                level = 0.5 * (lo + hi)
                if direc > 0:
                    ok = (prof[0] > level) if not crossing_up else (prof[0] < level)
                else:
                    ok = (prof[0] < level) if not crossing_up else (prof[0] > level)
                if not ok:
                    continue
                # locate the transition (first sign change of prof - level)
                idx = np.nonzero(np.diff(np.sign(prof - level)) != 0)[0]
                if len(idx) == 0:
                    continue
                i = int(idx[0])
                d = (prof[i + 1] - level) - (prof[i] - level)
                f = 0.0 if d == 0 else -(prof[i] - level) / d
                tc = t[i] + f * (t[i + 1] - t[i])
                if abs(tc - apothem) > 4.0:
                    continue
                pts.append((c[0] + tc * n[0] + u * tv[0],
                            c[1] + tc * n[1] + u * tv[1]))
                edge_idx.append(k)
                break
    return np.array(pts), np.array(edge_idx)


def fit_regular_hex(pts, edge_idx, cx, cy, R):
    """Least squares fit of an axis-aligned regular pointy-top hexagon.

    Rotation is deliberately locked to 0 deg: the source hairlines are exactly
    vertical / horizontal (measured direction cosines 1.0000 / 0.0000), so a
    free rotation parameter is degenerate with R (rotation and radius can trade
    off against each other).  Fitting is then a plain linear least squares
    problem in (cx, cy, R).
    """
    p = np.asarray(pts, dtype=np.float64)
    k = np.asarray(edge_idx, dtype=int)
    ns = _normals(0.0)
    n = ns[k]
    for _ in range(6):                      # 3 unknowns -> converges in 1 step
        rel = p - np.array([cx, cy])
        r = (rel * n).sum(axis=1) - R * APOTHEM_K
        j = np.empty((len(p), 3))
        j[:, 0] = -n[:, 0]
        j[:, 1] = -n[:, 1]
        j[:, 2] = -APOTHEM_K
        step, *_ = np.linalg.lstsq(j, -r, rcond=None)
        cx += float(step[0])
        cy += float(step[1])
        R += float(step[2])
        if np.abs(step).max() < 1e-10:
            break
    rel = p - np.array([cx, cy])
    r = (rel * n).sum(axis=1) - R * APOTHEM_K
    per_edge = [float(r[k == e].mean()) for e in range(6)]
    per_edge_rms = [float(np.sqrt(np.mean(r[k == e] ** 2))) for e in range(6)]
    rms = float(np.sqrt(np.mean(r ** 2)))
    return (cx, cy, R), rms, per_edge, per_edge_rms


def fit_bar(lum, p0, p1, name):
    """Sub-pixel fit of one hairline: returns dict with line + width."""
    p0 = np.asarray(p0, dtype=np.float64)
    p1 = np.asarray(p1, dtype=np.float64)
    d = p1 - p0
    L = float(np.hypot(*d))
    d = d / L
    n = np.array([-d[1], d[0]])
    ts = np.linspace(-4.0, 4.0, 161)
    centers, widths, peaks = [], [], []
    for u in np.arange(0.08 * L, 0.92 * L, 0.25):
        q = p0 + u * d
        prof = G.sample(lum, q[0] + ts * n[0], q[1] + ts * n[1])
        peak = float(prof.max())
        base = max(float(prof[0]), float(prof[-1]))
        if peak < 100.0 or base > 70.0:
            continue
        level = 0.5 * (peak + base)
        idx = np.nonzero(np.diff(np.sign(prof - level)) != 0)[0]
        if len(idx) < 2:
            continue
        i0, i1 = int(idx[0]), int(idx[1])
        if i1 - i0 < 1:
            continue
        def interp(i):
            dd = (prof[i + 1] - level) - (prof[i] - level)
            f = 0.0 if dd == 0 else -(prof[i] - level) / dd
            return ts[i] + f * (ts[i + 1] - ts[i])
        a, b = interp(i0), interp(i1)
        w = b - a
        if not (0.4 <= w <= 6.0):
            continue
        centers.append(q + 0.5 * (a + b) * n)
        widths.append(w)
        peaks.append(peak)
    if len(centers) < 6:
        raise RuntimeError("hairline %s: only %d usable samples" % (name, len(centers)))
    centers = np.array(centers)
    cen, dd, rms = G.line_from_points(centers)
    return {"name": name, "center": cen, "dir": dd,
            "width": float(np.median(widths)),
            "width_iqr": (float(np.percentile(widths, 25)),
                          float(np.percentile(widths, 75))),
            "peak": float(np.median(peaks)), "rms": rms,
            "samples": len(centers)}


def walk_end(lum, line, from_pt, direction, peak, limit=800.0):
    """Walk along the fitted line until brightness falls below 50% of peak."""
    level = 0.5 * peak
    step = 0.05
    s = 0.0
    n = np.array([-line["dir"][1], line["dir"][0]])
    while s < limit:
        q = from_pt + direction * s * line["dir"]
        v = float(np.mean(G.sample(lum, q[0] + np.linspace(-0.25, 0.25, 5) * n[0],
                                   q[1] + np.linspace(-0.25, 0.25, 5) * n[1])))
        if v < level:
            # refine between s-step and s
            lo, hi = s - step, s
            for _ in range(24):
                mid = 0.5 * (lo + hi)
                qm = from_pt + direction * mid * line["dir"]
                vm = float(np.mean(G.sample(lum, qm[0] + np.linspace(-0.25, 0.25, 5) * n[0],
                                            qm[1] + np.linspace(-0.25, 0.25, 5) * n[1])))
                if vm >= level:
                    lo = mid
                else:
                    hi = mid
            return from_pt + direction * (0.5 * (lo + hi)) * line["dir"]
        s += step
    return from_pt + direction * limit * line["dir"]


def line_intersect(l1, l2):
    p1, d1 = l1["center"], l1["dir"]
    p2, d2 = l2["center"], l2["dir"]
    den = d1[0] * d2[1] - d1[1] * d2[0]
    if abs(den) < 1e-12:
        raise RuntimeError("parallel lines %s / %s" % (l1["name"], l2["name"]))
    t = ((p2[0] - p1[0]) * d2[1] - (p2[1] - p1[1]) * d2[0]) / den
    return p1 + t * d1


def on_hexagon(line, cx, cy, R, from_pt, direction):
    """Sub-pixel point where the line leaves the fitted hexagon."""
    apothem = R * APOTHEM_K
    s = 0.0
    dt = 0.02
    prev = None
    while s < 900.0:
        q = from_pt + direction * s * line["dir"]
        r = max(abs((q[0] - cx) * math.cos(math.radians(60 * k) + 0.0)
                    + (q[1] - cy) * math.sin(math.radians(60 * k) + 0.0))
                for k in range(6))
        inside = r <= apothem
        if prev is not None and prev != inside:
            lo, hi = s - dt, s
            for _ in range(24):
                mid = 0.5 * (lo + hi)
                qm = from_pt + direction * mid * line["dir"]
                rm = max(abs((qm[0] - cx) * math.cos(math.radians(60 * k))
                             + (qm[1] - cy) * math.sin(math.radians(60 * k)))
                         for k in range(6))
                if (rm <= apothem) == prev:
                    lo = mid
                else:
                    hi = mid
            return from_pt + direction * (0.5 * (lo + hi)) * line["dir"]
        prev = inside
        s += dt
    return from_pt + direction * 900.0 * line["dir"]


# segment table: name, approx p0, approx p1, end0 mode, end1 mode
SEGMENTS = [
    ("vcenter", (333.5, 21.0), (333.5, 645.0), "walk", "walk"),
    ("hmid", (137.0, 343.5), (529.0, 343.5), "hex", "hex"),
    ("rail_l", (193.5, 186.4), (193.5, 343.5), "hex", ("i", "hmid")),
    ("rail_r", (472.5, 186.4), (472.5, 343.5), "hex", ("i", "hmid")),
    ("short_l", (264.5, 145.9), (264.5, 228.0), "hex", ("i", "cup_l")),
    ("short_r", (402.5, 145.9), (402.5, 228.0), "hex", ("i", "cup_r")),
    ("cup_l", (264.5, 228.0), (333.5, 262.0), ("i", "short_l"), ("i", "cup_r")),
    ("cup_r", (402.5, 228.0), (333.5, 262.0), ("i", "short_r"), ("i", "cup_l")),
    ("x_ul", (137.0, 343.5), (333.5, 423.8), ("i", "hmid"), ("i", "x_ur")),
    ("x_ur", (529.0, 343.5), (333.5, 423.8), ("i", "hmid"), ("i", "x_ul")),
    ("x_ll", (333.5, 423.8), (241.5, 465.0), ("i", "x_ul"), ("i", "basket_l")),
    ("x_lr", (333.5, 423.8), (425.5, 465.0), ("i", "x_ur"), ("i", "basket_r")),
    ("basket_l", (241.5, 465.0), (241.5, 502.0), ("i", "x_ll"), "walk"),
    ("basket_r", (425.5, 465.0), (425.5, 502.0), ("i", "x_lr"), "walk"),
]


def main():
    lum = G.load_lum()
    print("source:", G.source_path())

    # ---- outer hexagon -----------------------------------------------------
    pts, eidx = edge_points(lum, APPROX_C, APPROX_R_OUT * APOTHEM_K, 0.0,
                            half_span=0.30 * APPROX_R_OUT, n_par=35, n_prof=73,
                            crossing_up=False)
    (cx, cy, R_out), rms_out, off_out, per_out = fit_regular_hex(
        pts, eidx, APPROX_C[0], APPROX_C[1], APPROX_R_OUT)
    print("outer hex  centre=(%.3f, %.3f) R=%.3f  n=%d  rms=%.4f"
          % (cx, cy, R_out, len(pts), rms_out))
    print("   per-edge mean offset:", ["%+.3f" % v for v in off_out])
    print("   per-edge rms:", ["%.3f" % v for v in per_out])

    # ---- white ring inner edge (black field) -------------------------------
    pts2, eidx2 = edge_points(lum, (cx, cy), APPROX_R_FIELD * APOTHEM_K, 0.0,
                              half_span=0.26 * APPROX_R_FIELD, n_par=35, n_prof=73,
                              crossing_up=True)
    (fx, fy, R_field), rms_field, off_field, per_field = fit_regular_hex(
        pts2, eidx2, cx, cy, APPROX_R_FIELD)
    print("field hex  centre=(%.3f, %.3f) R=%.3f  n=%d  rms=%.4f"
          % (fx, fy, R_field, len(pts2), rms_field))
    print("   per-edge mean offset:", ["%+.3f" % v for v in off_field])
    print("   per-edge rms:", ["%.3f" % v for v in per_field])

    # ---- hairlines ---------------------------------------------------------
    bars = {}
    for name, a, b, _m0, _m1 in SEGMENTS:
        bars[name] = fit_bar(lum, a, b, name)
        print("bar %-9s dir=(%+.4f,%+.4f) w=%.3f (%.2f..%.2f) rms=%.3f n=%d"
              % (name, bars[name]["dir"][0], bars[name]["dir"][1],
                 bars[name]["width"], bars[name]["width_iqr"][0],
                 bars[name]["width_iqr"][1], bars[name]["rms"],
                 bars[name]["samples"]))

    widths = [bars[n]["width"] for n, *_ in SEGMENTS]
    w_global = float(np.median(widths))
    w_rms = float(np.sqrt(np.mean((np.array(widths) - w_global) ** 2)))
    print("global hairline width = %.3f px (rms of per-segment widths %.3f)" % (w_global, w_rms))

    # ---- endpoints ---------------------------------------------------------
    out_lines = []
    for name, a, b, m0, m1 in SEGMENTS:
        ln = bars[name]
        c = ln["center"]
        ends = []
        for mode, other_sign in ((m0, -1.0), (m1, +1.0)):
            if isinstance(mode, tuple) and mode[0] == "i":
                p = line_intersect(ln, bars[mode[1]])
            elif mode == "hex":
                p = on_hexagon(ln, fx, fy, R_field, c, other_sign)
            else:
                p = walk_end(lum, ln, c, other_sign, ln["peak"])
            ends.append(p)
        # keep the measured order consistent with the approximate segment
        if np.hypot(*(ends[0] - np.asarray(a))) > np.hypot(*(ends[1] - np.asarray(a))):
            ends.reverse()
        out_lines.append({"name": name,
                          "x1": round(float(ends[0][0]), 3),
                          "y1": round(float(ends[0][1]), 3),
                          "x2": round(float(ends[1][0]), 3),
                          "y2": round(float(ends[1][1]), 3),
                          "w": round(float(ln["width"]), 3),
                          "rms": round(float(ln["rms"]), 4)})
        print("seg %-9s (%.3f, %.3f) -> (%.3f, %.3f)" % (name, ends[0][0], ends[0][1],
                                                          ends[1][0], ends[1][1]))

    # ---- inscribed (hairline) hexagon radius -------------------------------
    rail_l = bars["rail_l"]
    rail_r = bars["rail_r"]
    x_l = float(rail_l["center"][0])
    x_r = float(rail_r["center"][0])
    half_w = 0.5 * (x_r - x_l)
    R_insc = half_w / APOTHEM_K
    print("rails x=%.3f / %.3f -> half width %.3f -> R_insc %.3f"
          % (x_l, x_r, half_w, R_insc))

    # hexagon edge residual: combined RMS of the two fitted hexagons
    rms_edge = math.sqrt(0.5 * (rms_out ** 2 + rms_field ** 2))

    data = {
        "source": os.path.basename(G.source_path()),
        "src_size": G.SRC_SIZE,
        "hex": {
            "cx": round(cx, 3), "cy": round(cy, 3),
            "rot_deg": 0.0,
            "R_out": round(R_out, 3),
            "R_field": round(R_field, 3),
            "R_insc": round(R_insc, 3),
            "field_cx": round(fx, 3), "field_cy": round(fy, 3),
            "field_rot_deg": 0.0,
            "rms_outer": round(rms_out, 4),
            "rms_field": round(rms_field, 4),
            "rms_edges": round(rms_edge, 4),
            "per_edge_offset_outer": [round(v, 4) for v in off_out],
            "per_edge_offset_field": [round(v, 4) for v in off_field],
            "per_edge_outer": [round(v, 4) for v in per_out],
            "per_edge_field": [round(v, 4) for v in per_field],
            "n_outer": int(len(pts)), "n_field": int(len(pts2)),
        },
        "hairline_width": round(w_global, 3),
        "hairline_width_rms": round(w_rms, 3),
        "lines": out_lines,
        "rms_edges_px": round(rms_edge, 4),
    }
    G.save_json(OUT, data)
    print("wrote", OUT)


if __name__ == "__main__":
    main()
