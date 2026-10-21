#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Export the WTUMCL application icon kit.

Source of truth is png/WTUMCL-logo-4096.png (rendered from the same vector
model as master/WTUMCL-logo.svg). Every export is a *premultiplied-alpha*
Lanczos downscale, so semi-transparent rim pixels keep the white colour of the
frame instead of picking up a dark fringe from the transparent black around it.

    python design/wtumcl-logo/tools/make_icons.py

Outputs
    icons/16.png ... icons/1024.png     9 RGBA PNGs, square, transparent corners
    icons/WTUMCL.ico                    multi-frame ICO (16/24/32/48/64/128/256)
"""
import os

from PIL import Image
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SRC = os.path.join(ROOT, "png", "WTUMCL-logo-4096.png")
OUT = os.path.join(ROOT, "icons")

SIZES = (16, 24, 32, 48, 64, 128, 256, 512, 1024)
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)   # Windows caps one frame at 256px


def _premultiply(img):
    a = np.asarray(img.convert("RGBA"), dtype=np.float32)
    af = a[..., 3:4] / 255.0
    out = a.copy()
    out[..., :3] = a[..., :3] * af
    return Image.fromarray(np.rint(out).astype(np.uint8), "RGBA")


def _unpremultiply(img):
    a = np.asarray(img, dtype=np.float32)
    af = a[..., 3:4] / 255.0
    rgb = np.divide(a[..., :3], af, out=np.zeros_like(a[..., :3]), where=af > 1e-4)
    out = a.copy()
    out[..., :3] = np.clip(np.rint(rgb), 0, 255)
    return Image.fromarray(out.astype(np.uint8), "RGBA")


def resize_lanczos(img, size):
    """High-quality alpha-correct Lanczos downscale to size x size."""
    pre = _premultiply(img)
    small = pre.resize((size, size), Image.LANCZOS)
    pre.close()
    out = _unpremultiply(small)
    small.close()
    return out


def save_ico(frames, path):
    """Write a PNG-frame ICO containing exactly the given frames."""
    import struct

    blobs = []
    for im in frames:
        assert im.size[0] == im.size[1] and im.size[0] <= 256
        tmp = os.path.join(OUT, ".ico-tmp-%d.png" % im.size[0])
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


def main():
    if not os.path.isfile(SRC):
        raise SystemExit("missing master render: %s" % SRC)
    os.makedirs(OUT, exist_ok=True)
    src = Image.open(SRC)
    src.load()
    if src.mode != "RGBA":
        src = src.convert("RGBA")
    print("source: %s %dx%d %s" % (os.path.basename(SRC), src.size[0], src.size[1], src.mode))

    frames = {}
    for size in SIZES:
        im = resize_lanczos(src, size)
        p = os.path.join(OUT, "%d.png" % size)
        im.save(p, format="PNG", optimize=True)
        frames[size] = im
        alpha = np.asarray(im)[..., 3]
        print("wrote %-16s %4dx%d  alpha>0: %6d px  corners=%d" % (
            os.path.basename(p), im.size[0], im.size[1],
            int((alpha > 0).sum()), int(alpha[0, 0]) + int(alpha[0, -1]) +
            int(alpha[-1, 0]) + int(alpha[-1, -1])))

    ico = os.path.join(OUT, "WTUMCL.ico")
    save_ico([frames[s] for s in ICO_SIZES], ico)
    print("wrote %-16s frames: %s  (%d bytes)" % (
        os.path.basename(ico), ", ".join(str(s) for s in ICO_SIZES), os.path.getsize(ico)))


if __name__ == "__main__":
    main()
