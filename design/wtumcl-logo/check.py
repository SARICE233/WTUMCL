#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Read-only acceptance validator for the WTUMCL logo deliverables.

    python design/wtumcl-logo/check.py

Prints one PASS/FAIL line per item; exits 0 only when every item passes.
Nothing in design/ is written or modified by this script.
"""
import math
import os
import sys
import xml.etree.ElementTree as ET

try:
    from PIL import Image
    import numpy as np
except Exception as exc:                      # pragma: no cover
    print("FAIL  dependencies  -- %s: %s" % (type(exc).__name__, exc))
    sys.exit(1)

ROOT = os.path.dirname(os.path.abspath(__file__))
PNG = os.path.join(ROOT, "png")
MASTER = os.path.join(ROOT, "master")
REF_LOCAL = os.path.join(ROOT, "reference", "WTUMCLOGO-reference.png")
REF_EXTERNAL = r"F:\Desktop\WTUMC-AI-VEDIO\WTUMCLOGO.png"

SVG = os.path.join(MASTER, "WTUMCL-logo.svg")
SVG_INV = os.path.join(MASTER, "WTUMCL-logo-inverse.svg")
SIZES = (1024, 2048, 4096)
PNG_NORMAL = {s: os.path.join(PNG, "WTUMCL-logo-%d.png" % s) for s in SIZES}
PNG_INV = os.path.join(PNG, "WTUMCL-logo-inverse-1024.png")

VIEWBOX = "0 0 1024 1024"
MAX_SVG_BYTES = 500 * 1024
MIN_ELEMENTS = 8
IOU_MIN = 0.85
CENTER = (0.36, 0.36, 0.64, 0.62)     # x0, y0, x1, y1 (fractions of the canvas)
CENTER_MIN_PIXELS = 500

RESULTS = []


def report(name, fn):
    try:
        ok, detail = fn()
    except Exception as exc:
        ok, detail = False, "%s: %s" % (type(exc).__name__, exc)
    RESULTS.append(ok)
    print("%s  %-42s %s" % ("PASS" if ok else "FAIL", name, detail or ""))
    return ok


def lum_of(rgb):
    a = np.asarray(rgb, dtype=np.float64)
    return 0.299 * a[..., 0] + 0.587 * a[..., 1] + 0.114 * a[..., 2]


def load_rgba(path):
    im = Image.open(path)
    im.load()
    return im


# --------------------------------------------------------------------- 1 SVG --
def c_svg():
    if not os.path.isfile(SVG):
        return False, "missing %s" % os.path.basename(SVG)
    size = os.path.getsize(SVG)
    if size > MAX_SVG_BYTES:
        return False, "file too large: %d bytes" % size
    tree = ET.parse(SVG)
    root = tree.getroot()
    tag = root.tag.split("}")[-1]
    if tag != "svg":
        return False, "root element is <%s>" % tag
    vb = (root.get("viewBox") or "").strip()
    if vb != VIEWBOX:
        return False, 'viewBox="%s" (expected "%s")' % (vb, VIEWBOX)
    n = len(list(root.iter()))
    if n < MIN_ELEMENTS:
        return False, "%d elements (min %d)" % (n, MIN_ELEMENTS)
    return True, "%d bytes, %d elements, viewBox ok" % (size, n)


def c_svg_inverse():
    if not os.path.isfile(SVG_INV):
        return False, "missing %s" % os.path.basename(SVG_INV)
    if os.path.getsize(SVG_INV) > MAX_SVG_BYTES:
        return False, "file too large"
    root = ET.parse(SVG_INV).getroot()
    if (root.get("viewBox") or "").strip() != VIEWBOX:
        return False, "viewBox mismatch"
    n = len(list(root.iter()))
    if n < MIN_ELEMENTS:
        return False, "%d elements" % n
    return True, "%d elements" % n


# --------------------------------------------------------------------- 2 PNG --
def c_png_size(size):
    def fn():
        p = PNG_NORMAL[size]
        if not os.path.isfile(p):
            return False, "missing WTUMCL-logo-%d.png" % size
        im = load_rgba(p)
        if im.size != (size, size):
            return False, "%dx%d" % im.size
        if "A" not in im.getbands():
            return False, "no alpha channel (mode %s)" % im.mode
        a = np.asarray(im)
        corners = [a[0, 0, 3], a[0, -1, 3], a[-1, 0, 3], a[-1, -1, 3]]
        if any(int(c) != 0 for c in corners):
            return False, "corner alpha %s" % corners
        return True, "%dx%d %s, corners transparent" % (im.size[0], im.size[1], im.mode)
    return fn


# --------------------------------------------------------------- 3 structure --
def _source_mask():
    for p in (REF_LOCAL, REF_EXTERNAL):
        if os.path.isfile(p):
            im = Image.open(p).convert("RGB")
            if im.size != (664, 664):
                continue
            return lum_of(im) > 160, p
    raise FileNotFoundError("no 664x664 source/reference bitmap found")


def c_iou():
    im = load_rgba(PNG_NORMAL[1024])
    small = im.resize((664, 664), Image.LANCZOS)
    a = np.asarray(small).astype(np.float64)
    alpha = a[..., 3]
    rgb = a[..., :3] * (alpha[..., None] / 255.0) + 255.0 * (1.0 - alpha[..., None] / 255.0)
    mask = (alpha > 16) & (lum_of(rgb) > 160)
    src, used = _source_mask()
    inter = float(np.logical_and(mask, src).sum())
    union = float(np.logical_or(mask, src).sum())
    iou = inter / union if union else 0.0
    ok = iou >= IOU_MIN
    return ok, "IoU=%.4f (min %.2f) vs %s" % (iou, IOU_MIN, os.path.basename(used))


# ------------------------------------------------------------ 4 hex geometry --
def c_pointy_top():
    im = load_rgba(PNG_NORMAL[1024])
    a = np.asarray(im)
    alpha = a[..., 3]
    ys, xs = np.nonzero(alpha > 16)
    if len(xs) == 0:
        return False, "empty canvas"
    w = float(xs.max() - xs.min() + 1)
    h = float(ys.max() - ys.min() + 1)
    ratio = w / h
    ideal = math.sqrt(3.0) / 2.0
    ok = abs(ratio - ideal) / ideal <= 0.03
    return ok, "bbox %.0fx%.0f w/h=%.4f (pointy-top ideal %.4f)" % (w, h, ratio, ideal)


def c_rim():
    """Horizontal rim thickness at the equator vs the measured source value."""
    im = load_rgba(PNG_NORMAL[1024])
    a = np.asarray(im)
    y = a.shape[0] // 2
    row = a[y]
    alpha = row[:, 3]
    xs = np.nonzero(alpha > 16)[0]
    if len(xs) == 0:
        return False, "empty row"
    left = int(xs.min())
    lum = lum_of(row[:, :3])
    dark = np.nonzero((alpha > 200) & (lum < 40) & (np.arange(len(alpha)) > left))[0]
    if len(dark) == 0:
        return False, "no inner ink region on the equator"
    measured = float(dark[0] - left)
    R, r = 312.3, 226.9
    expected = (math.sqrt(3.0) / 2.0) * (R - r) * (1024.0 / 664.0)
    err = abs(measured - expected) / expected
    ok = err <= 0.03
    return ok, "rim=%.1fpx expected=%.1fpx err=%.2f%%" % (measured, expected, err * 100.0)


# ------------------------------------------------------------- 5 inverse tone --
def _center_stats(im):
    a = np.asarray(im).astype(np.float64)
    h, w = a.shape[:2]
    x0, y0, x1, y1 = (int(CENTER[0] * w), int(CENTER[1] * h),
                      int(CENTER[2] * w), int(CENTER[3] * h))
    sub = a[y0:y1, x0:x1]
    alpha = sub[..., 3]
    rgb = sub[..., :3] * (alpha[..., None] / 255.0) + 255.0 * (1.0 - alpha[..., None] / 255.0)
    return float(lum_of(rgb).mean())


def c_inverse():
    if not os.path.isfile(PNG_INV):
        return False, "missing WTUMCL-logo-inverse-1024.png"
    inv = load_rgba(PNG_INV)
    if inv.size != (1024, 1024):
        return False, "%dx%d" % inv.size
    if "A" not in inv.getbands():
        return False, "no alpha channel"
    normal_mean = _center_stats(load_rgba(PNG_NORMAL[1024]))
    inv_mean = _center_stats(inv)
    delta = inv_mean - normal_mean
    ok = delta > 40
    return ok, "centre luminance normal=%.1f inverse=%.1f (+%.1f)" % (
        normal_mean, inv_mean, delta)


# ------------------------------------------------------------ 6 centre motif --
def c_center_content():
    im = load_rgba(PNG_NORMAL[1024])
    a = np.asarray(im).astype(np.float64)
    h, w = a.shape[:2]
    x0, y0, x1, y1 = (int(CENTER[0] * w), int(CENTER[1] * h),
                      int(CENTER[2] * w), int(CENTER[3] * h))
    sub = a[y0:y1, x0:x1]
    alpha = sub[..., 3]
    rgb = sub[..., :3]
    lum = lum_of(rgb)
    colored = (rgb[..., 2] - rgb[..., 0]) > 10
    n = int(((alpha > 16) & ((lum > 160) | colored)).sum())
    ok = n > CENTER_MIN_PIXELS
    return ok, "%d lit pixels (min %d)" % (n, CENTER_MIN_PIXELS)


def main():
    print("WTUMCL logo acceptance check -- %s" % ROOT)
    print("-" * 78)
    report("SVG master exists / parses / viewBox / size", c_svg)
    report("SVG inverse master", c_svg_inverse)
    for s in SIZES:
        report("PNG %d x %d RGBA, corners transparent" % (s, s), c_png_size(s))
    report("Structure IoU vs source bitmap", c_iou)
    report("Hexagon is pointy-top regular", c_pointy_top)
    report("Hexagon rim thickness", c_rim)
    report("Inverse colourway exists, centre inverted", c_inverse)
    report("Centre mascot / wordmark pixels present", c_center_content)
    print("-" * 78)
    bad = RESULTS.count(False)
    print("%d/%d passed%s" % (len(RESULTS) - bad, len(RESULTS),
                              "" if bad == 0 else "  -- %d FAILED" % bad))
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
