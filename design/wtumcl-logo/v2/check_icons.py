#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Read-only acceptance validator for the WTUMCL v2 icon kit.

    python design/wtumcl-logo/v2/check_icons.py

Verifies, one PASS/FAIL line per item:
  1. icons/ holds the nine PNGs 16..1024: exact square size, real alpha channel,
     fully transparent corners, nothing fully transparent.
  2. icons/WTUMCL.ico is a valid ICONDIR with >= 6 frames including 256, every
     frame decodes with its advertised size and carries alpha.
  3. icons/optical/ holds the whole-pixel 16 and 24 px builds at the right size,
     with binary alpha (every pixel fully transparent or fully opaque).
  4. compare/compare.png is >= 1600 px wide and holds three distinct panels;
     compare/diff.png and compare/icon-sheet.png exist and look like what they
     claim to be (heat map with a grey base, sheet with light and dark plates).
  5. every one of the nine PNGs equals a *recomputed* premultiplied-alpha
     Lanczos downscale of png/WTUMCL-logo-4096.png, per-pixel mean absolute
     error <= 1/255.  The downscale is re-derived here from the master PNG, so a
     plain up/down-scale of the wrong source cannot pass.

Prints the measured value next to every line and exits 0 only when all pass.
Nothing is written, created or modified by this script.
"""
import os
import struct
import sys

try:
    from PIL import Image
    import numpy as np
except Exception as exc:                      # pragma: no cover
    print("FAIL  dependencies -- %s: %s" % (type(exc).__name__, exc))
    sys.exit(1)

HERE = os.path.dirname(os.path.abspath(__file__))
ICONS = os.path.join(HERE, "icons")
OPTICAL = os.path.join(ICONS, "optical")
COMPARE = os.path.join(HERE, "compare")
MASTER = os.path.join(HERE, "png", "WTUMCL-logo-4096.png")

SIZES = (16, 24, 32, 48, 64, 128, 256, 512, 1024)
OPTICAL_SIZES = (16, 24)
ICO = os.path.join(ICONS, "WTUMCL.ico")
ICO_MIN_FRAMES = 6
ICO_REQUIRED = 256
CMP = os.path.join(COMPARE, "compare.png")
DIFF = os.path.join(COMPARE, "diff.png")
SHEET = os.path.join(COMPARE, "icon-sheet.png")
CMP_MIN_WIDTH = 1600
CMP_MIN_PANEL_LIT = 3000
DIFF_MIN_WARM = 2000
DIFF_MIN_GREY = 0.20
DOWNSCALE_MAE_MAX = 1.0 / 255.0       # in 0..1 units == 1/255 per channel

RESULTS = []


def report(name, fn):
    try:
        ok, detail = fn()
    except Exception as exc:
        ok, detail = False, "%s: %s" % (type(exc).__name__, exc)
    RESULTS.append(ok)
    print("%s  %-50s %s" % ("PASS" if ok else "FAIL", name, detail or ""))
    return ok


def load(path):
    im = Image.open(path)
    im.load()
    return im


def as_rgba(im):
    return im if im.mode == "RGBA" else im.convert("RGBA")


def premultiply(im):
    a = np.asarray(im.convert("RGBA"), dtype=np.float32)
    out = a.copy()
    out[..., :3] = a[..., :3] * (a[..., 3:4] / 255.0)
    return Image.fromarray(np.rint(out).astype(np.uint8), "RGBA")


def unpremultiply(im):
    a = np.asarray(im, dtype=np.float32)
    af = a[..., 3:4] / 255.0
    rgb = np.divide(a[..., :3], af, out=np.zeros_like(a[..., :3]), where=af > 1e-4)
    out = a.copy()
    out[..., :3] = np.clip(np.rint(rgb), 0, 255)
    return Image.fromarray(out.astype(np.uint8), "RGBA")


def reference_downscale(src, size):
    """Premultiplied-alpha Lanczos, re-derived from the 4096 master."""
    pre = premultiply(src)
    small = pre.resize((size, size), Image.LANCZOS)
    pre.close()
    out = unpremultiply(small)
    small.close()
    return out


# ------------------------------------------------------------------ 1: PNGs --
def c_png(size):
    def fn():
        path = os.path.join(ICONS, "%d.png" % size)
        if not os.path.isfile(path):
            return False, "missing %d.png" % size
        im = load(path)
        if im.size != (size, size):
            return False, "%dx%d (expected %dx%d)" % (im.size[0], im.size[1],
                                                      size, size)
        if "A" not in im.getbands() and "transparency" not in im.info:
            return False, "no alpha channel (mode %s)" % im.mode
        a = np.asarray(as_rgba(im))[..., 3]
        corners = (int(a[0, 0]), int(a[0, -1]), int(a[-1, 0]), int(a[-1, -1]))
        if any(v != 0 for v in corners):
            return False, "corner alpha %s (expected all 0)" % (corners,)
        lit = int((a > 0).sum())
        if lit == 0:
            return False, "image is fully transparent"
        if int(a.max()) < 200:
            return False, "max alpha %d (not solid anywhere)" % int(a.max())
        return True, "%dx%d RGBA, corners alpha 0, %d lit px, max alpha %d" % (
            im.size[0], im.size[1], lit, int(a.max()))
    return fn


def c_png_set():
    def fn():
        names = sorted(f for f in os.listdir(ICONS)
                       if f.lower().endswith(".png"))
        expect = set("%d.png" % s for s in SIZES)
        extra = [n for n in names if n not in expect]
        missing = sorted(expect - set(names))
        if missing:
            return False, "missing %s" % ", ".join(missing)
        if extra:
            return False, "unexpected png(s): %s" % ", ".join(extra)
        return True, "exactly %d PNGs (16..1024), no extras" % len(names)
    return fn


# ------------------------------------------------------------------- 2: ICO --
def _ico_entries(path):
    with open(path, "rb") as f:
        data = f.read()
    if len(data) < 6:
        raise ValueError("file shorter than ICONDIR")
    reserved, kind, count = struct.unpack("<HHH", data[:6])
    if reserved != 0:
        return None, "reserved=%d" % reserved
    if kind != 1:
        return None, "type=%d (expected 1 = icon)" % kind
    if count == 0:
        return None, "zero frames"
    if len(data) < 6 + 16 * count:
        return None, "truncated directory"
    entries = []
    for i in range(count):
        off = 6 + 16 * i
        w, h, colors, res, planes, bpp, blen, boff = struct.unpack(
            "<BBBBHHII", data[off:off + 16])
        w = w or 256
        h = h or 256
        blob = data[boff:boff + blen]
        entries.append({"w": w, "h": h, "bpp": bpp, "len": blen,
                        "png": blob[:8] == b"\x89PNG\r\n\x1a\n"})
    return entries, None


def c_ico():
    if not os.path.isfile(ICO):
        return False, "missing icons/WTUMCL.ico"
    size = os.path.getsize(ICO)
    entries, err = _ico_entries(ICO)
    if entries is None:
        return False, "invalid ICONDIR: %s" % err
    frames = sorted(set((e["w"], e["h"]) for e in entries))
    if len(entries) < ICO_MIN_FRAMES:
        return False, "%d frames (min %d)" % (len(entries), ICO_MIN_FRAMES)
    if ICO_REQUIRED not in [w for w, _h in frames]:
        return False, "no %dpx frame, frames=%s" % (ICO_REQUIRED, frames)
    if not all(w == h for w, h in frames):
        return False, "non-square frames %s" % (frames,)
    bad = [e for e in entries if e["len"] <= 0 or not e["png"]]
    if bad:
        return False, "%d frame(s) not PNG-compressed" % len(bad)
    im = load(ICO)
    if not (hasattr(im, "ico") and im.ico):
        return False, "Pillow cannot open the ICO"
    for w, h in frames:
        fr = im.ico.getimage((w, h))
        fr.load()
        if fr.size != (w, h):
            return False, "frame %dx%d decodes as %dx%d" % (w, h, fr.size[0],
                                                            fr.size[1])
        if "A" not in fr.getbands():
            return False, "frame %dx%d has no alpha (mode %s)" % (w, h, fr.mode)
    return True, "%d frames %s incl. 256, %d bytes, all decode with alpha" % (
        len(entries), [w for w, _h in frames], size)


# --------------------------------------------------------------- 3: optical --
def c_optical(size):
    def fn():
        path = os.path.join(OPTICAL, "%d.png" % size)
        if not os.path.isfile(path):
            return False, "missing optical/%d.png" % size
        im = load(path)
        if im.size != (size, size):
            return False, "optical/%d.png is %dx%d" % (size, im.size[0],
                                                       im.size[1])
        a = np.asarray(as_rgba(im))[..., 3]
        corners = (int(a[0, 0]), int(a[0, -1]), int(a[-1, 0]), int(a[-1, -1]))
        if any(v != 0 for v in corners):
            return False, "corner alpha %s (expected all 0)" % (corners,)
        levels = np.unique(a)
        opaque = int((a == 255).sum())
        if opaque == 0:
            return False, "no fully opaque pixel"
        if not np.all(np.isin(levels, (0, 255))):
            return False, "non-binary alpha: %d levels (%s..)" % (
                len(levels), ", ".join(str(int(v)) for v in levels[:6]))
        return True, "%dx%d, binary alpha (0/255), %d opaque px, corners 0" % (
            im.size[0], im.size[1], opaque)
    return fn


# --------------------------------------------------------------- 4: compare --
def c_compare():
    if not os.path.isfile(CMP):
        return False, "missing compare/compare.png"
    im = load(CMP)
    w, h = im.size
    if w < CMP_MIN_WIDTH:
        return False, "%dx%d, width < %d" % (w, h, CMP_MIN_WIDTH)
    if h < 400:
        return False, "%dx%d, height too small" % (w, h)
    a = np.asarray(im.convert("RGB")).astype(np.float32)
    third = w // 3
    thirds = [a[:, i * third:(i + 1) * third] for i in range(3)]
    lum = [0.299 * t[..., 0] + 0.587 * t[..., 1] + 0.114 * t[..., 2]
           for t in thirds]
    lit = [int((l > 190).sum()) for l in lum]
    if min(lit) < CMP_MIN_PANEL_LIT:
        return False, "panel lit pixels %s (min %d each)" % (lit,
                                                           CMP_MIN_PANEL_LIT)
    diffs = []
    for i in range(3):
        for j in range(i + 1, 3):
            n = min(thirds[i].shape[1], thirds[j].shape[1])
            diffs.append(float(np.abs(thirds[i][:, :n] - thirds[j][:, :n]).mean()))
    if min(diffs) < 2.0:
        return False, "panels look identical (pairwise mean diff %s)" % (
            ["%.2f" % d for d in diffs],)
    return True, "%dx%d, 3 distinct panels (lit %s, diff %s)" % (
        w, h, lit, ["%.1f" % d for d in diffs])


def c_diff():
    if not os.path.isfile(DIFF):
        return False, "missing compare/diff.png"
    im = load(DIFF).convert("RGB")
    w, h = im.size
    if w < 400 or h < 300:
        return False, "%dx%d too small" % (w, h)
    a = np.asarray(im).astype(np.int16)
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    warm = ((r - b) > 60) & (r > 120)
    neutral = (np.abs(r - g) < 8) & (np.abs(g - b) < 8)
    warm_px = int(warm.sum())
    grey_frac = float(neutral.mean())
    if warm_px < DIFF_MIN_WARM:
        return False, "only %d warm highlight px (min %d)" % (warm_px,
                                                             DIFF_MIN_WARM)
    if grey_frac < DIFF_MIN_GREY:
        return False, "only %.1f%% neutral grey base" % (grey_frac * 100.0)
    return True, "%dx%d, %d warm px, %.1f%% grey base" % (w, h, warm_px,
                                                          grey_frac * 100.0)


def c_sheet():
    if not os.path.isfile(SHEET):
        return False, "missing compare/icon-sheet.png"
    im = load(SHEET).convert("RGB")
    w, h = im.size
    if w < 1200 or h < 600:
        return False, "%dx%d too small for a size sheet" % (w, h)
    a = np.asarray(im).astype(np.float32)
    lum = 0.299 * a[..., 0] + 0.587 * a[..., 1] + 0.114 * a[..., 2]
    dark = float((lum < 40).mean())
    light = float((lum > 200).mean())
    if dark < 0.15:
        return False, "no dark plate (%.1f%% dark px)" % (dark * 100.0)
    if light < 0.15:
        return False, "no light plate (%.1f%% light px)" % (light * 100.0)
    return True, "%dx%d, %.1f%% dark + %.1f%% light plate" % (
        w, h, dark * 100.0, light * 100.0)


# ------------------------------------------------------- 5: downscale parity --
def c_downscale_parity():
    if not os.path.isfile(MASTER):
        return False, "missing png/WTUMCL-logo-4096.png"
    src = load(MASTER)
    if src.size != (4096, 4096):
        return False, "master is %dx%d (expected 4096)" % (src.size[0],
                                                          src.size[1])
    if src.mode != "RGBA":
        src = src.convert("RGBA")
    worst = (None, -1.0)
    rows = []
    for size in SIZES:
        path = os.path.join(ICONS, "%d.png" % size)
        if not os.path.isfile(path):
            return False, "missing %d.png" % size
        got = np.asarray(as_rgba(load(path))).astype(np.float32) / 255.0
        ref = np.asarray(reference_downscale(src, size)).astype(np.float32) / 255.0
        mae = float(np.abs(got - ref).mean())
        rows.append("%d:%.2f" % (size, mae * 255.0))
        if mae > worst[1]:
            worst = (size, mae)
    ok = worst[1] <= DOWNSCALE_MAE_MAX + 1e-12
    detail = ("worst %dpx MAE=%.4f/255 (max 1.0000)  [%s]"
              % (worst[0], worst[1] * 255.0, " ".join(rows)))
    return ok, detail


def main():
    print("WTUMCL v2 icon kit acceptance check -- %s" % HERE)
    print("master: png/WTUMCL-logo-4096.png   threshold: MAE <= 1/255")
    print("-" * 96)
    report("icons/ dir exists", lambda: (os.path.isdir(ICONS), "ok"))
    for s in SIZES:
        report("PNG %4d RGBA, exact size, corners transparent" % s, c_png(s))
    report("icons/ contains exactly the nine PNGs", c_png_set())
    report("WTUMCL.ico >= %d frames incl. %d" % (ICO_MIN_FRAMES, ICO_REQUIRED),
           c_ico)
    report("icons/optical/ dir exists",
           lambda: (os.path.isdir(OPTICAL), "ok"))
    for s in OPTICAL_SIZES:
        report("optical %4d whole-pixel PNG" % s, c_optical(s))
    for f, check, name in ((CMP, c_compare, "compare.png width >= %d, 3 panels" % CMP_MIN_WIDTH),
                           (DIFF, c_diff, "diff.png exists and is a heat map"),
                           (SHEET, c_sheet, "icon-sheet.png exists with plates")):
        report(name, check)
    report("nine PNGs == Lanczos(4096) downscale, MAE <= 1/255",
           c_downscale_parity)
    print("-" * 96)
    bad = RESULTS.count(False)
    print("%d/%d passed%s" % (len(RESULTS) - bad, len(RESULTS),
                              "" if bad == 0 else "  -- %d FAILED" % bad))
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
