# -*- coding: utf-8 -*-
"""Minimal rasteriser for the SVG subset emitted by build_svg.py.

Only <rect> and straight-line <path d="M..L..Z"> elements with a solid fill
are used by the masters; this module parses exactly that subset and renders it
with an SS x supersample + Lanczos downscale, preserving document z-order.
"""
import re

import numpy as np
from PIL import Image, ImageDraw

TOKEN = re.compile(r"[MLZmlz]|-?\d*\.?\d+")


def parse_svg(path):
    """Return (viewbox, [(fill_rgb, [polygon, ...]), ...]) in document order."""
    import xml.etree.ElementTree as ET
    root = ET.parse(path).getroot()
    vb = (root.get("viewBox") or "").replace(",", " ").split()
    viewbox = [float(v) for v in vb] if len(vb) == 4 else [0.0, 0.0, 1024.0, 1024.0]

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

    layers = []

    def walk(node, fill):
        tag = node.tag.split("}")[-1]
        fill = node.get("fill", fill)
        if tag in ("svg", "g"):
            for ch in node:
                walk(ch, fill)
        elif tag == "rect":
            x = float(node.get("x", 0)); y = float(node.get("y", 0))
            w = float(node.get("width", 0)); h = float(node.get("height", 0))
            poly = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
            layers.append((rgb(fill), [poly]))
        elif tag == "path":
            d = node.get("d", "")
            toks = TOKEN.findall(d)
            pts, cur, i = [], None, 0
            while i < len(toks):
                t = toks[i]
                if t in ("M", "L", "m", "l"):
                    x = float(toks[i + 1]); y = float(toks[i + 2])
                    if t in ("m", "l") and cur is not None:
                        x += cur[0]; y += cur[1]
                    cur = (x, y)
                    pts.append(cur)
                    i += 3
                else:
                    i += 1
            if len(pts) >= 3:
                layers.append((rgb(fill), [pts]))

    walk(root, "#000000")
    return viewbox, layers


def _group(layers):
    out = []
    for col, polys in layers:
        if out and out[-1][0] == col:
            out[-1][1].extend(polys)
        else:
            out.append((col, list(polys)))
    return out


def render(path, size, ss=4.0):
    viewbox, layers = parse_svg(path)
    vx, vy, vw, vh = viewbox
    big = int(round(size * ss))
    scale = big / float(vw)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    for col, polys in _group(layers):
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
