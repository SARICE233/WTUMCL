# -*- coding: utf-8 -*-
"""Write the two v2 SVG masters from tools/logo_model.py.

    python design/wtumcl-logo/v2/tools/build_svg.py
"""
import os

import logo_model as M

HERE = os.path.dirname(os.path.abspath(__file__))
V2 = os.path.dirname(HERE)
MASTER = os.path.join(V2, "master")


def f(v):
    s = "%.3f" % float(v)
    if s == "-0.000":
        s = "0.000"
    return s


def is_rect(poly):
    if len(poly) != 4:
        return False
    (x0, y0), (x1, y1), (x2, y2), (x3, y3) = poly
    return (abs(y0 - y1) < 1e-9 and abs(y2 - y3) < 1e-9
            and abs(x1 - x2) < 1e-9 and abs(x3 - x0) < 1e-9
            and x1 > x0 and y2 > y0)


def path_d(poly):
    d = "M%s %s" % (f(poly[0][0]), f(poly[0][1]))
    for x, y in poly[1:]:
        d += "L%s %s" % (f(x), f(y))
    return d + "Z"


def svg(inverse=False):
    layers = M.build(inverse)
    title = ("WTUMCL logo v2 (inverse colourway)" if inverse
             else "WTUMCL logo v2")
    desc = ("Inverse colourway: black hexagon ring, white inner field, black "
            "hairlines and wordmark, blue-grey sprite."
            if inverse else
            "Pointy-top white hexagon ring, black inner field, sub-pixel "
            "fitted hairlines, Minecraft String icon recovered on its 16x16 "
            "integer lattice, WTU / MC wordmark.")
    out = ['<?xml version="1.0" encoding="UTF-8"?>',
           '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" '
           'width="1024" height="1024" shape-rendering="geometricPrecision">',
           "  <title>%s</title>" % title,
           "  <desc>%s</desc>" % desc,
           '  <g id="%s">' % ("inverse" if inverse else "normal")]
    for idx, (col, polys) in enumerate(layers):
        out.append('    <g id="layer-%d" fill="%s" stroke="none">' % (idx, col))
        for poly in polys:
            if is_rect(poly):
                (x0, y0) = poly[0]
                w = poly[1][0] - x0
                h = poly[2][1] - y0
                out.append('      <rect x="%s" y="%s" width="%s" height="%s"/>'
                           % (f(x0), f(y0), f(w), f(h)))
            else:
                out.append('      <path d="%s"/>' % path_d(poly))
        out.append("    </g>")
    out.append("  </g>")
    out.append("</svg>")
    return "\n".join(out) + "\n"


def main():
    os.makedirs(MASTER, exist_ok=True)
    for name, inv in (("WTUMCL-logo.svg", False),
                      ("WTUMCL-logo-inverse.svg", True)):
        data = svg(inv)
        p = os.path.join(MASTER, name)
        with open(p, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(data)
        print("wrote %s (%d bytes)" % (p, len(data.encode("utf-8"))))


if __name__ == "__main__":
    main()
