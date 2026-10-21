#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Export the v2 WTUMCL application icon kit.

    python design/wtumcl-logo/v2/tools/make_icons.py

Inputs
    png/WTUMCL-logo-4096.png        the v2 master render (bump-free source)
    data/geometry.json              hairline segments (names, endpoints)
    data/lattice.json               sprite palette + 16x16 cell boxes

Outputs
    icons/16.png ... icons/1024.png nine RGBA PNGs, square, transparent corners
    icons/WTUMCL.ico                multi-frame ICO (16/24/32/48/64/128/256)
    icons/optical/16.png, 24.png    whole-pixel (pixel-aligned) variants

Two rendering paths, by design:

  standard  premultiplied-alpha Lanczos downscale of the 4096 master, so the
            anti-aliased vector look survives at every size.  The check script
            re-derives these nine files bit for bit.

  optical   only for 16 and 24 px, where a 2.276 source-px hairline maps to
            0.05-0.08 px: the Lanczos version leaves the hairline web as a grey
            haze.  The optical variant re-renders the model on the *whole-pixel
            grid*: 8x supersampled coverage is thresholded at 50 %, so every
            output pixel is either fully transparent or an opaque palette
            colour - no grey, no fractional edges.  Hairlines are drawn at
            exactly 1 px (the minimum visible line width), never as sub-pixel
            ghosts.  Because 1 px is ~12 % of a 16 px icon, the web is admitted
            in symmetric groups under an explicit per-size policy: 16 px
            keeps the two main axes, 24 px adds the rails and the basket
            outline.  The admitted set and its measured ink are printed.
"""
import math
import os
import struct

from PIL import Image
import numpy as np

import logo_model as M

HERE = os.path.dirname(os.path.abspath(__file__))
V2 = os.path.dirname(HERE)
SRC = os.path.join(V2, "png", "WTUMCL-logo-4096.png")
ICONS = os.path.join(V2, "icons")
OPTICAL = os.path.join(ICONS, "optical")

SIZES = (16, 24, 32, 48, 64, 128, 256, 512, 1024)
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)   # Windows caps one frame at 256 px
OPTICAL_SIZES = (16, 24)
SS = 8                    # supersampling for the whole-pixel coverage masks
# Hairline policy for the two optical sizes.  A whole-pixel hairline is ~12 %
# of a 16 px icon, so the web is simplified instead of thickened: the symmetric
# groups below are the smallest subset that still reads at each size.
HAIR_POLICY = {
    16: ("axes",),
    24: ("axes", "rails", "basket"),
}

# symmetric hairline groups, coarsest first
HAIR_GROUPS = (
    ("axes", ("vcenter", "hmid")),
    ("rails", ("rail_l", "rail_r")),
    ("basket", ("basket_l", "basket_r")),
    ("short", ("short_l", "short_r")),
    ("cup", ("cup_l", "cup_r")),
    ("web", ("x_ul", "x_ur", "x_ll", "x_lr")),
)


# --------------------------------------------------------------- standard --
def premultiply(img):
    a = np.asarray(img.convert("RGBA"), dtype=np.float32)
    out = a.copy()
    out[..., :3] = a[..., :3] * (a[..., 3:4] / 255.0)
    return Image.fromarray(np.rint(out).astype(np.uint8), "RGBA")


def unpremultiply(img):
    a = np.asarray(img, dtype=np.float32)
    af = a[..., 3:4] / 255.0
    rgb = np.divide(a[..., :3], af, out=np.zeros_like(a[..., :3]), where=af > 1e-4)
    out = a.copy()
    out[..., :3] = np.clip(np.rint(rgb), 0, 255)
    return Image.fromarray(out.astype(np.uint8), "RGBA")


def resize_lanczos(img, size):
    pre = premultiply(img)
    small = pre.resize((size, size), Image.LANCZOS)
    pre.close()
    out = unpremultiply(small)
    small.close()
    return out


def save_ico(frames, path):
    """Write a PNG-frame ICO containing exactly the given frames."""
    blobs = []
    for im in frames:
        assert im.size[0] == im.size[1] and im.size[0] <= 256
        tmp = os.path.join(ICONS, ".ico-tmp-%d.png" % im.size[0])
        im.save(tmp, format="PNG", optimize=True)
        with open(tmp, "rb") as f:
            blobs.append(f.read())
        os.remove(tmp)
    n = len(frames)
    header = struct.pack("<HHH", 0, 1, n)
    offset = 6 + 16 * n
    entries = b""
    for im, blob in zip(frames, blobs):
        s = im.size[0]
        w = 0 if s >= 256 else s
        entries += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
    with open(path, "wb") as f:
        f.write(header + entries + b"".join(blobs))


# ---------------------------------------------------------------- optical --
def _rgb(col):
    c = col.lstrip("#")
    return (int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16))


def coverage(polys, size, ss=SS):
    big = int(size * ss)
    mask = Image.new("L", (big, big), 0)
    from PIL import ImageDraw
    dr = ImageDraw.Draw(mask)
    k = big / M.CANVAS
    for poly in polys:
        dr.polygon([(x * k, y * k) for x, y in poly], fill=255)
    small = mask.resize((size, size), Image.BOX)
    cov = np.asarray(small, dtype=np.float32) / 255.0
    mask.close()
    small.close()
    return cov


def segment_band(x1, y1, x2, y2, size, half):
    """Pixels whose centre lies within half px of the segment (1 px wide line)."""
    xs = np.arange(size) + 0.5
    ys = np.arange(size) + 0.5
    X, Y = np.meshgrid(xs, ys)
    dx, dy = x2 - x1, y2 - y1
    l2 = dx * dx + dy * dy
    t = np.clip(((X - x1) * dx + (Y - y1) * dy) / l2, 0.0, 1.0)
    return np.hypot(X - (x1 + t * dx), Y - (y1 + t * dy)) <= half + 1e-9


def optical(size):
    """Whole-pixel render: hard 50 % coverage rule, 1 px hairlines, no grey."""
    k = size / M.CANVAS
    outer = [[M.P(*p) for p in M.hexagon(M.HEX["cx"], M.HEX["cy"],
                                         M.HEX["R_out"])]]
    field = [[M.P(*p) for p in M.hexagon(M.HEX["field_cx"], M.HEX["field_cy"],
                                         M.HEX["R_field"])]]
    field_cov = coverage(field, size)

    sprite = M.sprite_rects()
    groups = []
    for ci, _col in enumerate(M.SPRITE["palette"]):
        polys = [p for p, c in sprite if c == ci]
        if polys:
            groups.append(("#%02X%02X%02X" % tuple(M.SPRITE["palette"][ci]),
                           coverage(polys, size)))
    word_cov = coverage(M.wordmark_rects(), size)

    # hairlines: 1 whole pixel wide, admitted group-wise under the ink budget
    by_name = {ln["name"]: ln for ln in M.GEOM["lines"]}
    half = max(1.0, M.HAIR_W * size / M.SRC) / 2.0
    bands = {}
    for name, ln in by_name.items():
        bands[name] = segment_band(ln["x1"] * k, ln["y1"] * k, ln["x2"] * k,
                                   ln["y2"] * k, size, half)
    field_area = 2.598 * (M.HEX["R_field"] * size / M.SRC) ** 2
    keep = set()
    for gname, names in HAIR_GROUPS:
        if gname in HAIR_POLICY.get(size, ()):
            keep.update(names)
    hair = np.zeros((size, size), bool)
    chosen = []
    for gname, names in HAIR_GROUPS:
        if gname not in HAIR_POLICY.get(size, ()):
            continue
        band = np.zeros((size, size), bool)
        for nm in names:
            band |= bands[nm]
        band &= (field_cov >= 0.5)          # hairlines live inside the field
        hair |= band
        chosen.extend(names)
    ink = float(hair.sum()) / field_area

    out = np.zeros((size, size, 4), dtype=np.uint8)

    def put(mask, col):
        out[mask, :3] = _rgb(col)
        out[mask, 3] = 255

    put(coverage(outer, size) >= 0.5, M.WHITE)
    put(field_cov >= 0.5, M.INK)
    if groups:
        # tiny sprite: snap the coverage-weighted average colour to the nearest
        # palette entry instead of letting one cell colour win the pixel
        pal = np.array([_rgb(c) for c, _c in groups] + [_rgb(M.INK)],
                       dtype=np.float32)
        acc = np.zeros((size, size, 3), dtype=np.float32)
        used = np.zeros((size, size), dtype=np.float32)
        for col, cov in groups:
            w = np.clip(cov, 0.0, 1.0)
            acc += w[..., None] * np.array(_rgb(col), dtype=np.float32)
            used += w
        acc += np.clip(1.0 - used, 0.0, 1.0)[..., None] * np.array(
            _rgb(M.INK), dtype=np.float32)
        avg = np.clip(acc / np.maximum(np.clip(used, 0.0, 1.0)[..., None]
                                       + np.clip(1.0 - used, 0.0, 1.0)[..., None],
                                       1e-6), 0.0, 255.0)
        d = ((avg[..., None, :] - pal[None, None, :, :]) ** 2).sum(axis=-1)
        pick = d.argmin(axis=-1)
        for ci in range(len(pal)):
            put((pick == ci) & (used >= 0.5) & (field_cov >= 0.5), M.INK if ci == len(pal) - 1 else groups[ci][0])
    put(hair, M.WHITE)
    put(word_cov >= 0.5, M.WHITE)
    return Image.fromarray(out, "RGBA"), chosen, ink


# -------------------------------------------------------------------- main --
def main():
    if not os.path.isfile(SRC):
        raise SystemExit("missing master render: %s" % SRC)
    os.makedirs(ICONS, exist_ok=True)
    os.makedirs(OPTICAL, exist_ok=True)

    src = Image.open(SRC)
    src.load()
    if src.mode != "RGBA":
        src = src.convert("RGBA")
    print("source: %s %dx%d %s" % (os.path.basename(SRC), src.size[0],
                                   src.size[1], src.mode))

    frames = {}
    for size in SIZES:
        im = resize_lanczos(src, size)
        p = os.path.join(ICONS, "%d.png" % size)
        im.save(p, format="PNG", optimize=True)
        frames[size] = im
        a = np.asarray(im)[..., 3]
        print("wrote %-22s %4dx%d  alpha>0: %6d px  corners=%d" % (
            os.path.relpath(p, V2), im.size[0], im.size[1], int((a > 0).sum()),
            int(a[0, 0]) + int(a[0, -1]) + int(a[-1, 0]) + int(a[-1, -1])))

    ico = os.path.join(ICONS, "WTUMCL.ico")
    save_ico([frames[s] for s in ICO_SIZES], ico)
    print("wrote %-22s frames: %s  (%d bytes)" % (
        os.path.relpath(ico, V2), ", ".join(str(s) for s in ICO_SIZES),
        os.path.getsize(ico)))

    for size in OPTICAL_SIZES:
        im, chosen, ink = optical(size)
        p = os.path.join(OPTICAL, "%d.png" % size)
        im.save(p, format="PNG", optimize=True)
        a = np.asarray(im)
        opaque = int((a[..., 3] == 255).sum())
        print("wrote %-22s %4dx%d  opaque: %5d px  hairline ink %.3f  kept: %s" % (
            os.path.relpath(p, V2), im.size[0], im.size[1], opaque, ink,
            ", ".join(chosen)))


if __name__ == "__main__":
    main()
