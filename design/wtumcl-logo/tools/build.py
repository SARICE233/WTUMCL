#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Build the two SVG masters from tools/logo_model.py.

    python design/wtumcl-logo/tools/build.py
"""
import os
import logo_model as M

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
MASTER = os.path.join(ROOT, "master")


def _is_rect(poly):
    if len(poly) != 4:
        return False
    (x0, y0), (x1, y1), (x2, y2), (x3, y3) = poly
    return (abs(y0 - y1) < 1e-6 and abs(y2 - y3) < 1e-6
            and abs(x1 - x2) < 1e-6 and abs(x3 - x0) < 1e-6
            and x1 > x0 and y2 > y0)


def _f(v):
    s = ("%.2f" % v).rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def _path_d(poly):
    d = "M%s %s" % (_f(poly[0][0]), _f(poly[0][1]))
    for x, y in poly[1:]:
        d += "L%s %s" % (_f(x), _f(y))
    return d + "Z"


def svg(inverse=False):
    layers = M.build(inverse)
    title = "WTUMCL logo (inverse / negative colourway)" if inverse else "WTUMCL logo"
    desc = ("Inverse colourway: white canvas, black outer hexagon, white inner "
            "field, black hairlines and wordmark." if inverse else
            "Pointy-top white hexagon ring, black inner field, white hairlines, "
            "pixel mascot and WTU/MC wordmark.")
    out = ['<?xml version="1.0" encoding="UTF-8"?>',
           '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" '
           'width="1024" height="1024" shape-rendering="geometricPrecision">',
           "  <title>%s</title>" % title,
           "  <desc>%s</desc>" % desc,
           '  <g id="' + ("inverse" if inverse else "normal") + '">']
    for idx, (key, polys) in enumerate(layers):
        col = M.PALETTE[key]
        out.append('    <g id="layer-%d-%s" fill="%s" stroke="none">' % (idx, key, col))
        for poly in polys:
            if _is_rect(poly):
                (x0, y0) = poly[0]
                out.append('      <rect x="%s" y="%s" width="%s" height="%s"/>'
                           % (_f(x0), _f(y0), _f(poly[1][0] - x0), _f(poly[2][1] - y0)))
            else:
                out.append('      <path d="%s"/>' % _path_d(poly))
        out.append("    </g>")
    out.append("  </g>")
    out.append("</svg>")
    return "\n".join(out) + "\n"


def main():
    os.makedirs(MASTER, exist_ok=True)
    for name, inv in (("WTUMCL-logo.svg", False), ("WTUMCL-logo-inverse.svg", True)):
        data = svg(inv)
        p = os.path.join(MASTER, name)
        with open(p, "w", encoding="utf-8", newline="\n") as f:
            f.write(data)
        print("wrote %s (%d bytes)" % (p, len(data.encode("utf-8"))))


if __name__ == "__main__":
    main()
