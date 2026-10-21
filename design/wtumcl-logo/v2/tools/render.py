# -*- coding: utf-8 -*-
"""Rasterise the v2 SVG masters to RGBA PNGs.

    python design/wtumcl-logo/v2/tools/render.py            # all sizes
    python design/wtumcl-logo/v2/tools/render.py 1024       # one size
"""
import os
import sys

import svgraster

HERE = os.path.dirname(os.path.abspath(__file__))
V2 = os.path.dirname(HERE)
MASTER = os.path.join(V2, "master")
PNG = os.path.join(V2, "png")

JOBS = [(1024, 4.0), (2048, 2.0), (4096, 1.5)]


def main():
    os.makedirs(PNG, exist_ok=True)
    want = [int(a) for a in sys.argv[1:] if a.isdigit()]
    for size, ss in JOBS:
        if want and size not in want:
            continue
        img = svgraster.render(os.path.join(MASTER, "WTUMCL-logo.svg"), size, ss)
        p = os.path.join(PNG, "WTUMCL-logo-%d.png" % size)
        img.save(p)
        print("wrote %s %dx%d" % (p, img.size[0], img.size[1]))
    if not want or 1024 in want:
        img = svgraster.render(os.path.join(MASTER, "WTUMCL-logo-inverse.svg"),
                               1024, 4.0)
        p = os.path.join(PNG, "WTUMCL-logo-inverse-1024.png")
        img.save(p)
        print("wrote %s %dx%d" % (p, img.size[0], img.size[1]))


if __name__ == "__main__":
    main()
