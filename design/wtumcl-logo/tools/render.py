#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Rasterise the SVG master model to RGBA PNGs at 1024 / 2048 / 4096.

Every layer polygon is drawn at SS x the target resolution into an 8-bit
coverage mask, the mask is Lanczos-downsampled to the exact output size and
then composited in z-order with straight alpha, so the exported PNG is a true
anti-aliased supersample of the same geometry the SVG contains.

    python design/wtumcl-logo/tools/render.py            # all sizes
    python design/wtumcl-logo/tools/render.py 1024       # one size
"""
import os
import sys
from PIL import Image, ImageDraw
import logo_model as M

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
PNG = os.path.join(ROOT, "png")

# target size -> supersample factor
JOBS = [(1024, 4.0), (2048, 2.0), (4096, 1.5)]


def _rgb(hexcol):
    return tuple(int(hexcol[i:i + 2], 16) for i in (1, 3, 5))


def render(size, ss=2.0, inverse=False):
    big = int(round(size * ss))
    scale = float(big) / M.CANVAS
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    for key, polys in M.build(inverse):
        mask_big = Image.new("L", (big, big), 0)
        d = ImageDraw.Draw(mask_big)
        for poly in polys:
            d.polygon([(x * scale, y * scale) for x, y in poly], fill=255)
        mask = mask_big.resize((size, size), Image.LANCZOS)
        mask_big.close()
        layer = Image.new("RGBA", (size, size), _rgb(M.PALETTE[key]) + (255,))
        layer.putalpha(mask)
        canvas = Image.alpha_composite(canvas, layer)
        layer.close()
        mask.close()
    return canvas


def main():
    os.makedirs(PNG, exist_ok=True)
    want = [int(a) for a in sys.argv[1:] if a.isdigit()]
    for size, ss in JOBS:
        if want and size not in want:
            continue
        img = render(size, ss)
        p = os.path.join(PNG, "WTUMCL-logo-%d.png" % size)
        img.save(p)
        print("wrote %s %dx%d" % (p, img.size[0], img.size[1]))
    if not want or 1024 in want:
        img = render(1024, 4.0, inverse=True)
        p = os.path.join(PNG, "WTUMCL-logo-inverse-1024.png")
        img.save(p)
        print("wrote %s %dx%d" % (p, img.size[0], img.size[1]))


if __name__ == "__main__":
    main()
