#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Read-only acceptance validator for the WTUMCL logo v2 deliverables.

    python design/wtumcl-logo/v2/check_v2.py

Every number is measured from the files on disk: the SVG master is parsed and
rasterised here (no import of the build pipeline), the PNGs are opened, the
reference bitmap is compared after an integer translation alignment search.

Prints one PASS/FAIL line per criterion and exits 0 only when all pass.
Nothing is written by this script.
"""
import math
import os
import re
import sys
import xml.etree.ElementTree as ET

try:
    from PIL import Image, ImageDraw
    import numpy as np
except Exception as exc:                       # pragma: no cover
    print("FAIL  dependencies -- %s: %s" % (type(exc).__name__, exc))
    sys.exit(1)

HERE = os.path.dirname(os.path.abspath(__file__))
MASTER = os.path.join(HERE, "master")
PNG = os.path.join(HERE, "png")
SRC_CANDIDATES = [
    r"F:\Desktop\WTUMC-AI-VEDIO\WTUMCLOGO.png",
    os.path.join(os.path.dirname(HERE), "reference", "WTUMCLOGO-reference.png"),
]

SVG = os.path.join(MASTER, "WTUMCL-logo.svg")
SVG_INV = os.path.join(MASTER, "WTUMCL-logo-inverse.svg")
SIZES = (1024, 2048, 4096)
VIEWBOX = "0 0 1024 1024"

# thresholds demanded by the task
IOU_GLOBAL_MIN = 0.95
IOU_CENTER_MIN = 0.90
MAE_CENTER_MAX = 8.0
IOU_WORD_MIN = 0.90
HEX_RMS_MAX = 0.6
SVG_CONSIST_MAX = 1.0 / 255.0

# analysis windows in source (664) pixels
CENTER_BOX = (265, 255, 395, 395)     # centre sprite + 6 px margin all round
WORD_BOX = (283, 450, 386, 528)       # WTU + MC block, ring / X lines excluded
BORDER = 6                            # margin ignored by the global mask score

RESULTS = []


def report(name, fn):
    try:
        ok, detail = fn()
    except Exception as exc:
        ok, detail = False, "%s: %s" % (type(exc).__name__, exc)
    RESULTS.append(ok)
    print("%s  %-46s %s" % ("PASS" if ok else "FAIL", name, detail or ""))
    return ok


# --------------------------------------------------------------- SVG raster --
TOKEN = re.compile(r"[MLZmlz]|-?\d*\.?\d+")


def parse_svg(path):
    root = ET.parse(path).getroot()
    tag = root.tag.split("}")[-1]
    if tag != "svg":
        raise ValueError("root element is <%s>" % tag)
    raw = open(path, "rb").read()
    if b"<image" in raw or b"base64" in raw or b"data:" in raw:
        raise ValueError("embedded bitmap found in the SVG")
    vb = (root.get("viewBox") or "").replace(",", " ").split()
    if len(vb) != 4:
        raise ValueError("missing viewBox")
    viewbox = [float(v) for v in vb]

    def rgb(col):
        col = (col or "#000000").strip()
        if col.startswith("#"):
            h = col[1:]
            if len(h) == 3:
                h = "".join(c * 2 for c in h)
            return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))
        m = re.match(r"rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)", col)
        if m:
            return tuple(int(g) for g in m.groups())
        return (0, 0, 0)

    layers, elements = [], 0

    def walk(node, fill):
        nonlocal elements
        t = node.tag.split("}")[-1]
        fill = node.get("fill", fill)
        if t in ("svg", "g"):
            for ch in node:
                walk(ch, fill)
        elif t == "rect":
            elements += 1
            x = float(node.get("x", 0)); y = float(node.get("y", 0))
            w = float(node.get("width", 0)); h = float(node.get("height", 0))
            layers.append((rgb(fill), [(x, y), (x + w, y), (x + w, y + h), (x, y + h)]))
        elif t == "path":
            elements += 1
            toks = TOKEN.findall(node.get("d", ""))
            pts, cur, i = [], None, 0
            while i < len(toks):
                if toks[i] in ("M", "L", "m", "l"):
                    x = float(toks[i + 1]); y = float(toks[i + 2])
                    if toks[i] in ("m", "l") and cur is not None:
                        x += cur[0]; y += cur[1]
                    cur = (x, y)
                    pts.append(cur)
                    i += 3
                else:
                    i += 1
            if len(pts) >= 3:
                layers.append((rgb(fill), pts))
        elif t in ("title", "desc", "metadata"):
            return
        else:
            raise ValueError("unsupported element <%s>" % t)

    walk(root, "#000000")
    return root, viewbox, layers, elements, len(raw)


def render_layers(layers, viewbox, size, ss):
    vx, vy, vw, vh = viewbox
    big = int(round(size * ss))
    scale = big / float(vw)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    groups = []
    for col, poly in layers:
        if groups and groups[-1][0] == col:
            groups[-1][1].append(poly)
        else:
            groups.append((col, [poly]))
    for col, polys in groups:
        mask = Image.new("L", (big, big), 0)
        d = ImageDraw.Draw(mask)
        for poly in polys:
            d.polygon([((x - vx) * scale, (y - vy) * scale) for x, y in poly], fill=255)
        small = mask.resize((size, size), Image.LANCZOS)
        mask.close()
        layer = Image.new("RGBA", (size, size), col + (255,))
        layer.putalpha(small)
        canvas = Image.alpha_composite(canvas, layer)
        layer.close()
        small.close()
    return canvas


# ------------------------------------------------------------------ helpers --
def lum(rgb):
    a = np.asarray(rgb, dtype=np.float64)
    return 0.299 * a[..., 0] + 0.587 * a[..., 1] + 0.114 * a[..., 2]


def load_source():
    for p in SRC_CANDIDATES:
        if os.path.isfile(p):
            im = Image.open(p).convert("RGB")
            if im.size == (664, 664):
                return np.asarray(im).astype(np.float64), p
    raise FileNotFoundError("no 664x664 source bitmap found")


def flatten(im):
    """RGBA image -> RGB over white (the logo is used on a light canvas)."""
    a = np.asarray(im).astype(np.float64)
    al = a[..., 3:4] / 255.0
    return a[..., :3] * al + 255.0 * (1.0 - al)


def fg_mask(rgb, thr=160.0):
    return lum(rgb) > thr


def render_mask(flat, alpha, thr=160.0):
    """Opaque + bright: the drawn (non-transparent) foreground."""
    return (alpha > 16.0 / 255.0) & (lum(flat) > thr)


def align(src_mask, ren_mask, radius=6):
    """Integer translation maximising the mask intersection."""
    best = (None, -1.0)
    h, w = src_mask.shape
    for dy in range(-radius, radius + 1):
        for dx in range(-radius, radius + 1):
            m = np.zeros_like(ren_mask)
            ys0, ys1 = max(0, dy), min(h, h + dy)
            xs0, xs1 = max(0, dx), min(w, w + dx)
            m[ys0:ys1, xs0:xs1] = ren_mask[ys0 - dy:ys1 - dy, xs0 - dx:xs1 - dx]
            inter = float(np.logical_and(src_mask, m).sum())
            if inter > best[1]:
                best = ((dx, dy), inter)
    return best[0]


def shift_subpixel(img, dx, dy, stride=1):
    """Bilinear translation of a 2-D or 3-D image by (dx, dy)."""
    h, w = img.shape[:2]
    ys = np.arange(0, h, stride, dtype=np.float64)
    xs = np.arange(0, w, stride, dtype=np.float64)
    sx = xs[None, :] - dx
    sy = ys[:, None] - dy
    x0 = np.floor(sx).astype(int)
    y0 = np.floor(sy).astype(int)
    fx = sx - x0
    fy = sy - y0
    x0 = np.clip(x0, 0, w - 1); x1 = np.clip(x0 + 1, 0, w - 1)
    y0 = np.clip(y0, 0, h - 1); y1 = np.clip(y0 + 1, 0, h - 1)
    a00 = img[y0, x0]; a10 = img[y0, x1]
    a01 = img[y1, x0]; a11 = img[y1, x1]
    if img.ndim == 3:
        fx = fx[..., None]; fy = fy[..., None]
    return a00 * (1 - fx) * (1 - fy) + a10 * fx * (1 - fy) + a01 * (1 - fx) * fy + a11 * fx * fy


def refine_shift(src_rgb, ren_rgb, dx0, dy0, radius=0.5, step=0.05):
    """Sub-pixel refinement of the translation, minimising the image MAE."""
    best = (None, 1e18)
    for dx in np.arange(dx0 - radius, dx0 + radius + 1e-9, step):
        for dy in np.arange(dy0 - radius, dy0 + radius + 1e-9, step):
            v = shift_subpixel(ren_rgb, dx, dy, stride=4)
            e = float(np.abs(src_rgb[::4, ::4] - v).mean())
            if e < best[1]:
                best = ((float(dx), float(dy)), e)
    return best[0]


def shift_image(img, dx, dy):
    dx, dy = int(round(dx)), int(round(dy))
    h, w = img.shape[:2]
    out = np.zeros_like(img)
    ys0, ys1 = max(0, dy), min(h, h + dy)
    xs0, xs1 = max(0, dx), min(w, w + dx)
    out[ys0:ys1, xs0:xs1] = img[ys0 - dy:ys1 - dy, xs0 - dx:xs1 - dx]
    return out


def iou(a, b):
    u = float(np.logical_or(a, b).sum())
    return float(np.logical_and(a, b).sum()) / u if u else 1.0


# ------------------------------------------------------------------ criteria --
STATE = {}


def prepare():
    src, path = load_source()
    STATE["src_rgb"] = src
    STATE["src_mask"] = fg_mask(src)
    STATE["src_path"] = path
    root, viewbox, layers, elements, nbytes = parse_svg(SVG)
    STATE["svg"] = (root, viewbox, layers, elements, nbytes)
    # the master is a 1024x1024 document: render it at its native size with
    # 8x supersampling, then downsample to the 664 px reference grid
    im = render_layers(layers, viewbox, 1024, 8.0)
    STATE["ren1024"] = im
    small = np.asarray(im.resize((664, 664), Image.LANCZOS)).astype(np.float64)
    rgb = small[..., :3]
    alpha = small[..., 3:4] / 255.0
    flat = rgb * alpha + 255.0 * (1.0 - alpha)
    STATE["ren664"] = flat
    STATE["ren664_alpha"] = alpha[..., 0]
    STATE["ren664_mask"] = render_mask(flat, alpha[..., 0])
    dxi, dyi = align(STATE["src_mask"], STATE["ren664_mask"])
    dx, dy = refine_shift(STATE["src_rgb"], flat, float(dxi), float(dyi))
    STATE["shift"] = (dx, dy)
    STATE["shift_int"] = (dxi, dyi)
    STATE["ren664_aligned"] = shift_subpixel(flat, dx, dy)
    STATE["ren664_alpha_aligned"] = shift_subpixel(alpha[..., 0], dx, dy)
    STATE["ren664_mask_aligned"] = shift_image(STATE["ren664_mask"], dxi, dyi)


def c_svg_parse():
    root, viewbox, layers, elements, nbytes = STATE["svg"]
    vb = " ".join("%g" % v for v in viewbox)
    if vb != VIEWBOX:
        return False, 'viewBox="%s"' % vb
    if elements < 8:
        return False, "%d drawing elements" % elements
    if any(hasattr(n, "tag") and n.tag.split("}")[-1] == "image"
           for n in root.iter()):
        return False, "embedded <image>"
    txt = open(SVG, "r", encoding="utf-8").read()
    nums = re.findall(r"-?\d+\.\d+", txt)
    if nums and any(len(n.split(".")[1]) > 3 for n in nums):
        return False, "coordinates with more than 3 decimals"
    return True, "%d bytes, %d elements, viewBox ok, %.3f decimals" % (
        nbytes, elements, 3.0)


def c_svg_inverse():
    _, viewbox, layers, elements, nbytes = parse_svg(SVG_INV)
    if " ".join("%g" % v for v in viewbox) != VIEWBOX:
        return False, "viewBox mismatch"
    return True, "%d bytes, %d elements" % (nbytes, elements)


def c_png_size(size):
    def fn():
        p = os.path.join(PNG, "WTUMCL-logo-%d.png" % size)
        if not os.path.isfile(p):
            return False, "missing WTUMCL-logo-%d.png" % size
        im = Image.open(p)
        im.load()
        if im.size != (size, size):
            return False, "%dx%d" % im.size
        if im.mode != "RGBA":
            return False, "mode %s (expected RGBA)" % im.mode
        a = np.asarray(im)
        corners = [int(a[0, 0, 3]), int(a[0, -1, 3]), int(a[-1, 0, 3]), int(a[-1, -1, 3])]
        if any(c != 0 for c in corners):
            return False, "corner alpha %s" % corners
        return True, "%dx%d RGBA, corners alpha=0" % im.size
    return fn


def c_iou_global():
    a = STATE["src_mask"].copy()
    b = STATE["ren664_mask_aligned"].copy()
    b[:BORDER, :] = False
    b[-BORDER:, :] = False
    b[:, :BORDER] = False
    b[:, -BORDER:] = False
    a[:BORDER, :] = False
    a[-BORDER:, :] = False
    a[:, :BORDER] = False
    a[:, -BORDER:] = False
    v = iou(a, b)
    dx, dy = STATE["shift"]
    return v >= IOU_GLOBAL_MIN, "IoU=%.4f (min %.2f) shift=(%.1f,%.1f)" % (
        v, IOU_GLOBAL_MIN, dx, dy)


def _box(arr, box):
    x0, y0, x1, y1 = box
    return arr[y0:y1, x0:x1]


def c_iou_center():
    s = _box(STATE["src_mask"], CENTER_BOX)
    r = _box(STATE["ren664_mask_aligned"], CENTER_BOX)
    v = iou(s, r)
    return v >= IOU_CENTER_MIN, "IoU=%.4f (min %.2f)" % (v, IOU_CENTER_MIN)


def c_mae_center():
    s = _box(STATE["src_rgb"], CENTER_BOX)
    r = _box(STATE["ren664_aligned"], CENTER_BOX)
    mae = float(np.abs(s - r).mean())
    return mae <= MAE_CENTER_MAX, "MAE=%.3f/255 (max %.1f)" % (mae, MAE_CENTER_MAX)


def c_iou_word():
    s = _box(STATE["src_mask"], WORD_BOX)
    r = _box(STATE["ren664_mask_aligned"], WORD_BOX)
    v = iou(s, r)
    return v >= IOU_WORD_MIN, "IoU=%.4f (min %.2f)" % (v, IOU_WORD_MIN)


def _crossing(v, radii, level):
    """First 50 % intensity crossing (either direction), linearly interpolated."""
    s = v - level
    idx = np.nonzero(np.diff(np.sign(s)) != 0)[0]
    for i in idx:
        if s[i] == 0:
            return float(radii[i])
        d = s[i + 1] - s[i]
        f = 0.0 if d == 0 else -s[i] / d
        return float(radii[i] + f * (radii[i + 1] - radii[i]))
    return None


def _hex_radius_profile(img, cx, cy, k, angles, radii):
    """50 % crossing radius along hexagon normal k for every tangential offset."""
    th = math.radians(60.0 * k)
    nx, ny = math.cos(th), math.sin(th)
    tx, ty = -ny, nx
    out = []
    for u in angles:
        xs = cx + u * tx + radii * nx
        ys = cy + u * ty + radii * ny
        if xs.min() < 1 or ys.min() < 1 or xs.max() > img.shape[1] - 2 \
                or ys.max() > img.shape[0] - 2:
            continue
        x0 = np.floor(xs).astype(int)
        y0 = np.floor(ys).astype(int)
        fx = xs - x0
        fy = ys - y0
        v = (img[y0, x0] * (1 - fx) * (1 - fy) + img[y0, x0 + 1] * fx * (1 - fy)
             + img[y0 + 1, x0] * (1 - fx) * fy + img[y0 + 1, x0 + 1] * fx * fy)
        if v.max() - v.min() < 40:
            continue
        r = _crossing(v, radii, 0.5 * (v.max() + v.min()))
        if r is None:
            continue
        out.append((round(float(u), 4), r))
    return out


def c_hex_rms():
    src = STATE["src_mask"].astype(np.float64) * 255.0
    ren_l = STATE["ren664_alpha_aligned"] * 255.0
    ys, xs = np.nonzero(STATE["src_mask"])
    cx = 0.5 * (xs.min() + xs.max())
    cy = 0.5 * (ys.min() + ys.max())
    radii = np.arange(255.0, 325.0, 0.05)
    res = []
    for k in range(6):
        span = 0.22 * 270.0
        ang = np.linspace(-span, span, 41)
        a = dict(_hex_radius_profile(src, cx, cy, k, ang, radii))
        b = dict(_hex_radius_profile(ren_l, cx, cy, k, ang, radii))
        for u, r in a.items():
            if u in b:
                res.append(r - b[u])
    if not res:
        return False, "no edge crossings measured"
    res = np.asarray(res)
    rms = float(np.sqrt(np.mean(res ** 2)))
    return rms <= HEX_RMS_MAX, "RMS=%.3f px (max %.2f, n=%d)" % (
        rms, HEX_RMS_MAX, len(res))


def c_svg_consistency():
    root, viewbox, layers, elements, nbytes = STATE["svg"]
    a = render_layers(layers, viewbox, 1024, 8.0)
    b = render_layers(layers, viewbox, 4096, 2.0)
    b1024 = b.resize((1024, 1024), Image.LANCZOS)
    aa = np.asarray(a).astype(np.float64)
    bb = np.asarray(b1024).astype(np.float64)
    mae = float(np.abs(aa - bb).mean() / 255.0)
    return mae <= SVG_CONSIST_MAX, "MAE=%.5f/255 (max %.5f)" % (
        mae * 255.0, SVG_CONSIST_MAX * 255.0)


def c_inverse_png():
    p = os.path.join(PNG, "WTUMCL-logo-inverse-1024.png")
    if not os.path.isfile(p):
        return False, "missing WTUMCL-logo-inverse-1024.png"
    im = Image.open(p)
    im.load()
    if im.size != (1024, 1024) or im.mode != "RGBA":
        return False, "%s %s" % (im.size, im.mode)
    a = np.asarray(im)
    corners = [int(a[0, 0, 3]), int(a[0, -1, 3]), int(a[-1, 0, 3]), int(a[-1, -1, 3])]
    if any(c != 0 for c in corners):
        return False, "corner alpha %s" % corners
    return True, "1024x1024 RGBA, corners alpha=0"


def main():
    print("WTUMCL logo v2 acceptance check -- %s" % HERE)
    print("-" * 80)
    try:
        prepare()
    except Exception as exc:
        print("FAIL  preparation -- %s: %s" % (type(exc).__name__, exc))
        return 1
    print("      source      %s" % STATE["src_path"])
    print("      alignment   shift=(%.1f, %.1f) px @664 (integer seed %s)"
          % (STATE["shift"][0], STATE["shift"][1], STATE["shift_int"]))
    print("-" * 80)
    report("SVG master parses / viewBox / no bitmap", c_svg_parse)
    report("SVG inverse master", c_svg_inverse)
    for s in SIZES:
        report("PNG %d x %d RGBA, corners transparent" % (s, s), c_png_size(s))
    report("PNG inverse 1024 RGBA", c_inverse_png)
    report("Global foreground IoU (aligned)", c_iou_global)
    report("Centre sprite IoU", c_iou_center)
    report("Centre sprite MAE", c_mae_center)
    report("Wordmark IoU", c_iou_word)
    report("Hexagon edge residual RMS", c_hex_rms)
    report("SVG self-render 4096->1024 vs 1024", c_svg_consistency)
    print("-" * 80)
    bad = RESULTS.count(False)
    print("%d/%d passed%s" % (len(RESULTS) - bad, len(RESULTS),
                              "" if bad == 0 else "  -- %d FAILED" % bad))
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
