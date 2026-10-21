#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Read-only acceptance validator for the WTUMCL icon kit.

    python design/wtumcl-logo/check-icons.py

Verifies, one PASS/FAIL line per item:
  1. icons/ holds exactly the nine PNGs 16..1024, exact square size, real alpha,
     fully transparent corners.
  2. icons/WTUMCL.ico is a valid ICONDIR with >= 6 frames including 256, and
     every frame decodes to the advertised size with alpha.
  3. compare/compare.png is >= 1600 px wide and really contains three distinct
     panels (source / redraw / overlay).
  4. compare/diff.png exists and is a grey-based differential heat map
     (warm highlights present, image differs from a flat grey card).

Exits 0 only when every item passes. Nothing is written or modified.
"""
import os
import struct
import sys

try:
    from PIL import Image
    import numpy as np
except Exception as exc:                      # pragma: no cover
    print("FAIL  dependencies            -- %s: %s" % (type(exc).__name__, exc))
    sys.exit(1)

ROOT = os.path.dirname(os.path.abspath(__file__))
ICONS = os.path.join(ROOT, "icons")
COMPARE = os.path.join(ROOT, "compare")
SIZES = (16, 24, 32, 48, 64, 128, 256, 512, 1024)
ICO = os.path.join(ICONS, "WTUMCL.ico")
ICO_MIN_FRAMES = 6
ICO_REQUIRED = 256
CMP = os.path.join(COMPARE, "compare.png")
DIFF = os.path.join(COMPARE, "diff.png")
CMP_MIN_WIDTH = 1600
CMP_MIN_PANEL_LIT = 3000       # bright pixels expected inside each panel
DIFF_MIN_WARM = 2000           # warm (highlight) pixels expected in the heat map
DIFF_MIN_GREY = 0.20           # at least 20% of the image stays neutral grey

RESULTS = []


def report(name, fn):
    try:
        ok, detail = fn()
    except Exception as exc:
        ok, detail = False, "%s: %s" % (type(exc).__name__, exc)
    RESULTS.append(ok)
    print("%s  %-46s %s" % ("PASS" if ok else "FAIL", name, detail or ""))
    return ok


def load(path):
    im = Image.open(path)
    im.load()
    return im


def as_rgba(im):
    return im if im.mode == "RGBA" else im.convert("RGBA")


# ------------------------------------------------------------------ 1: PNGs --
def c_png(size):
    def fn():
        path = os.path.join(ICONS, "%d.png" % size)
        if not os.path.isfile(path):
            return False, "missing %d.png" % size
        im = load(path)
        if im.size != (size, size):
            return False, "%dx%d (expected %dx%d)" % (im.size[0], im.size[1], size, size)
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
        return True, "%dx%d RGBA, corners transparent, %d lit px" % (
            im.size[0], im.size[1], lit)
    return fn


def c_png_extra_absent():
    def fn():
        names = sorted(f for f in os.listdir(ICONS) if f.lower().endswith(".png"))
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
        is_png = blob[:8] == b"\x89PNG\r\n\x1a\n"
        entries.append({"w": w, "h": h, "bpp": bpp, "len": blen, "png": is_png})
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
    square = all(w == h for w, h in frames)
    if not square:
        return False, "non-square frames %s" % (frames,)
    bad = [e for e in entries if e["len"] <= 0 or not e["png"]]
    if bad:
        return False, "%d frame(s) not PNG-compressed" % len(bad)
    # decode every frame so a corrupt blob cannot pass on the directory alone
    im = load(ICO)
    got = sorted(im.ico.sizes()) if hasattr(im, "ico") and im.ico else []
    for w, h in frames:
        fr = im.ico.getimage((w, h))
        fr.load()
        if fr.size != (w, h):
            return False, "frame %dx%d decodes as %dx%d" % (w, h, fr.size[0], fr.size[1])
    return True, "%d frames %s, 256 present, %d bytes, decodes ok" % (
        len(entries), [w for w, _h in frames], size)


# --------------------------------------------------------------- 3: compare --
def _thirds(path):
    im = load(path).convert("RGB")
    a = np.asarray(im).astype(np.float32)
    w = a.shape[1]
    third = w // 3
    return im.size, [a[:, i * third:(i + 1) * third] for i in range(3)]


def c_compare():
    if not os.path.isfile(CMP):
        return False, "missing compare/compare.png"
    im = load(CMP)
    w, h = im.size
    if w < CMP_MIN_WIDTH:
        return False, "%dx%d, width < %d" % (w, h, CMP_MIN_WIDTH)
    if h < 400:
        return False, "%dx%d, height too small" % (w, h)
    size, thirds = _thirds(CMP)
    lum = [0.299 * t[..., 0] + 0.587 * t[..., 1] + 0.114 * t[..., 2] for t in thirds]
    lit = [int((l > 190).sum()) for l in lum]
    if min(lit) < CMP_MIN_PANEL_LIT:
        return False, "panel lit pixels %s (min %d each)" % (lit, CMP_MIN_PANEL_LIT)
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


# ------------------------------------------------------------------ 4: diff --
def c_diff():
    if not os.path.isfile(DIFF):
        return False, "missing compare/diff.png"
    size = os.path.getsize(DIFF)
    im = load(DIFF)
    if "A" not in im.getbands():
        im = im.convert("RGB")
    else:
        im = im.convert("RGB")
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
        return False, "only %d warm highlight px (min %d)" % (warm_px, DIFF_MIN_WARM)
    if grey_frac < DIFF_MIN_GREY:
        return False, "only %.1f%% neutral grey background" % (grey_frac * 100.0)
    return True, "%dx%d, %d warm px, %.1f%% grey base, %d bytes" % (
        w, h, warm_px, grey_frac * 100.0, size)


def main():
    print("WTUMCL icon kit acceptance check -- %s" % ROOT)
    print("-" * 78)
    report("icons/ dir exists", lambda: (os.path.isdir(ICONS), "ok"))
    for s in SIZES:
        report("PNG %4d x %4d RGBA, corners transparent" % (s, s), c_png(s))
    report("icons/ contains exactly the nine PNGs", c_png_extra_absent())
    report("WTUMCL.ico >= %d frames incl. %d" % (ICO_MIN_FRAMES, ICO_REQUIRED), c_ico)
    report("compare.png width >= %d, three panels" % CMP_MIN_WIDTH, c_compare)
    report("diff.png exists and is a heat map", c_diff)
    print("-" * 78)
    bad = RESULTS.count(False)
    print("%d/%d passed%s" % (len(RESULTS) - bad, len(RESULTS),
                              "" if bad == 0 else "  -- %d FAILED" % bad))
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
