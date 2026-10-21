#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Generate the comparison sheets in design/wtumcl-logo/preview/.

    python design/wtumcl-logo/tools/make_previews.py
"""
import os
from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
PREV = os.path.join(ROOT, "preview")
REF = os.path.join(ROOT, "reference", "WTUMCLOGO-reference.png")

INK = (18, 22, 26)
LABEL = (232, 238, 242)


def _sheet(items, out, pad=18, label_h=34, bg=INK):
    w = sum(i.width for i, _l in items) + pad * (len(items) + 1)
    h = max(i.height for i, _l in items) + pad * 2 + label_h
    canvas = Image.new("RGB", (w, h), bg)
    d = ImageDraw.Draw(canvas)
    x = pad
    for img, label in items:
        canvas.paste(img, (x, pad + label_h), img if img.mode == "RGBA" else None)
        d.text((x, pad + 8), label, fill=LABEL)
        x += img.width + pad
    canvas.save(out)
    print("wrote", out, canvas.size)


def main():
    os.makedirs(PREV, exist_ok=True)
    src = Image.open(REF).convert("RGBA")
    normal = Image.open(os.path.join(ROOT, "png", "WTUMCL-logo-1024.png"))
    inv = Image.open(os.path.join(ROOT, "png", "WTUMCL-logo-inverse-1024.png"))

    box = (63, 21, 604, 645)
    s = src.crop(box).resize((541, 624), Image.LANCZOS)
    on_black = Image.new("RGBA", normal.size, (0, 0, 0, 255))
    n = Image.alpha_composite(on_black, normal).convert("RGBA")
    n = n.resize((664, 664), Image.LANCZOS).crop(box).resize((541, 624), Image.LANCZOS)
    _sheet([(s, "source bitmap (664 px)"), (n, "vector redraw @1024")],
           os.path.join(PREV, "comparison-source-vs-redraw.png"))

    _sheet([(normal.resize((420, 420), Image.LANCZOS), "normal / transparent"),
            (inv.resize((420, 420), Image.LANCZOS), "inverse / white field")],
           os.path.join(PREV, "colorways.png"))

    mb = (265, 265, 393, 393)
    ms = src.crop(mb).resize((384, 384), Image.NEAREST)
    mn = Image.alpha_composite(on_black, normal).convert("RGBA").resize((664, 664), Image.LANCZOS)
    mn = mn.crop(mb).resize((384, 384), Image.NEAREST)
    _sheet([(ms, "source mascot"), (mn, "redrawn mascot")],
           os.path.join(PREV, "mascot-detail.png"))


if __name__ == "__main__":
    main()
