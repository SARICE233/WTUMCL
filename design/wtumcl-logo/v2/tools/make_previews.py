# -*- coding: utf-8 -*-
"""Preview images for the v2 logo.

    python design/wtumcl-logo/v2/tools/make_previews.py

* preview/grid-overlay.png          source + fitted (warped) 16x16 lattice
* preview/sprite-cellmap.png        every cell enlarged, judged colour + hex
* preview/comparison-v1-v2-source.png   source / v1 / v2
* preview/overlay-v2.png            v2 over the source (red = v2 only, green = source only)
"""
import json
import os
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
V2 = os.path.dirname(HERE)
LOGODIR = os.path.dirname(V2)
sys.path.insert(0, HERE)

import geomlib as G
import svgraster

PREVIEW = os.path.join(V2, "preview")
MASTER = os.path.join(V2, "master")
FONT = ImageFont.load_default()


def lum_arr(rgb):
    a = np.asarray(rgb, dtype=np.float64)
    return 0.299 * a[..., 0] + 0.587 * a[..., 1] + 0.114 * a[..., 2]


def load_render(path, size, ss=8.0):
    im = svgraster.render(path, size, ss)
    return im


def flat(im):
    a = np.asarray(im).astype(np.float64)
    al = a[..., 3:4] / 255.0
    return a[..., :3] * al + 255.0 * (1.0 - al)


def grid_overlay(src_rgb, lat):
    sp = lat["blocks"]["sprite"]
    box = (250, 248, 420, 418)
    Z = 6
    crop = Image.fromarray(np.clip(src_rgb, 0, 255).astype(np.uint8)).crop(box)
    big = crop.resize((crop.width * Z, crop.height * Z), Image.NEAREST).convert("RGB")
    d = ImageDraw.Draw(big, "RGBA")
    pal = sp["palette"]
    boxes = {(b[0], b[1]): b[2:] for b in sp["cell_boxes"]}
    for c in sp["cells"]:
        ci = c["c"]
        if ci < 0:
            continue
        x0, y0, x1, y1 = boxes[(c["i"], c["j"])]
        col = pal[ci]
        d.rectangle([(x0 - box[0]) * Z, (y0 - box[1]) * Z,
                     (x1 - box[0]) * Z, (y1 - box[1]) * Z],
                    fill=(col[0], col[1], col[2], 90))
    gx, gy = sp["grid_x"], sp["grid_y"]
    for v in gx:
        x = (v - box[0]) * Z
        d.line([x, 0, x, big.height], fill=(255, 40, 40, 200), width=1)
    for v in gy:
        y = (v - box[1]) * Z
        d.line([0, y, big.width, y], fill=(255, 40, 40, 200), width=1)
    gxu, gyu = sp["grid_x_uniform"], sp["grid_y_uniform"]
    for v in gxu:
        x = (v - box[0]) * Z
        d.line([x, 0, x, big.height], fill=(60, 140, 255, 120), width=1)
    for v in gyu:
        y = (v - box[1]) * Z
        d.line([0, y, big.width, y], fill=(60, 140, 255, 120), width=1)
    d.text((8, 8), "v2 lattice  s=%.3f  origin=(%.3f, %.3f)  16x16 cells"
           % (sp["s"], sp["x0"], sp["y0"]), fill=(255, 255, 0, 255), font=FONT)
    d.text((8, 20), "red = sub-pixel refined lines, blue = uniform lattice",
           fill=(160, 200, 255, 255), font=FONT)
    big.save(os.path.join(PREVIEW, "grid-overlay.png"))
    return big


def sprite_cellmap(lat):
    sp = lat["blocks"]["sprite"]
    pal = sp["palette"]
    i0, j0 = sp["i0"], sp["j0"]
    n = 16
    cell = 44
    pad = 40
    W = pad + n * cell + 10
    H = pad + n * cell + 40
    im = Image.new("RGB", (W, H), (24, 24, 28))
    d = ImageDraw.Draw(im)
    med = {(c[0], c[1]): c[2] for c in sp["cell_medians"]}
    for dj in range(n):
        for di in range(n):
            i, j = i0 + di, j0 + dj
            x = pad + di * cell
            y = pad + dj * cell
            c = next((cc for cc in sp["cells"] if cc["i"] == i and cc["j"] == j), None)
            if c is None or c["c"] < 0:
                d.rectangle([x, y, x + cell - 2, y + cell - 2], fill=(10, 10, 12),
                            outline=(48, 48, 54))
                continue
            col = pal[c["c"]]
            d.rectangle([x, y, x + cell - 2, y + cell - 2], fill=tuple(col),
                        outline=(40, 40, 40))
            m = med.get((i, j))
            if m is not None:
                l = 0.299 * m[0] + 0.587 * m[1] + 0.114 * m[2]
                tc = (0, 0, 0) if l > 120 else (255, 255, 255)
                d.text((x + 4, y + cell - 16), "P%d" % c["c"], fill=tc, font=FONT)
                d.text((x + 4, y + cell - 28), "#%02X%02X%02X" % tuple(col), fill=tc, font=FONT)
    for k in range(n + 1):
        d.line([pad + k * cell, pad, pad + k * cell, pad + n * cell], fill=(90, 90, 100))
        d.line([pad, pad + k * cell, pad + n * cell, pad + k * cell], fill=(90, 90, 100))
    for k in range(n):
        d.text((pad + k * cell + cell // 2 - 3, 16), str(i0 + k), fill=(200, 200, 200), font=FONT)
        d.text((10, pad + k * cell + cell // 2 - 4), str(j0 + k), fill=(200, 200, 200), font=FONT)
    d.text((pad, H - 22), "Minecraft String sprite recovered on the 16x16 integer lattice "
           "(s=%.3f)" % sp["s"], fill=(220, 220, 220), font=FONT)
    im.save(os.path.join(PREVIEW, "sprite-cellmap.png"))
    return im


def comparison(src_rgb, v1_rgb, v2_rgb, box=(250, 248, 420, 418)):
    Z = 3
    tiles = []
    labels = ["source 664x664", "v1 (accepted)", "v2 (this build)"]
    ims = []
    for arr in (src_rgb, v1_rgb, v2_rgb):
        crop = Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8)).crop(box)
        ims.append(crop.resize((crop.width * Z, crop.height * Z), Image.NEAREST))
    w, h = ims[0].size
    out = Image.new("RGB", (w * 3 + 24, h + 26), (24, 24, 28))
    d = ImageDraw.Draw(out)
    for k, im in enumerate(ims):
        out.paste(im, (k * (w + 12), 26))
        d.text((k * (w + 12) + 4, 8), labels[k], fill=(230, 230, 230), font=FONT)
    out.save(os.path.join(PREVIEW, "comparison-v1-v2-source.png"))
    return out


def overlay(src_rgb, v2_rgb):
    a = np.clip(src_rgb, 0, 255).astype(np.uint8)
    b = np.clip(v2_rgb, 0, 255).astype(np.uint8)
    la = lum_arr(a)
    lb = lum_arr(b)
    comp = np.zeros_like(a)
    comp[..., 0] = np.clip(lb, 0, 255)          # v2 -> red channel
    comp[..., 1] = np.clip(la, 0, 255)          # source -> green channel
    comp[..., 2] = np.clip(np.minimum(la, lb), 0, 255)
    im = Image.fromarray(comp)
    im = im.resize((im.width * 2, im.height * 2), Image.NEAREST)
    d = ImageDraw.Draw(im)
    d.text((8, 8), "overlay: magenta/red = v2 only, green = source only, white = both",
           fill=(255, 255, 255), font=FONT)
    im.save(os.path.join(PREVIEW, "overlay-v2.png"))
    return im


def main():
    os.makedirs(PREVIEW, exist_ok=True)
    lat = G.load_json(os.path.join(V2, "data", "lattice.json"))
    src = G.load_rgb()
    v2 = flat(load_render(os.path.join(MASTER, "WTUMCL-logo.svg"), 1024, 8.0)
              .resize((664, 664), Image.LANCZOS))
    v1p = os.path.join(LOGODIR, "png", "WTUMCL-logo-1024.png")
    if os.path.isfile(v1p):
        im = Image.open(v1p).convert("RGBA").resize((664, 664), Image.LANCZOS)
        a = np.asarray(im).astype(np.float64)
        al = a[..., 3:4] / 255.0
        v1 = a[..., :3] * al + 255.0 * (1 - al)
    else:
        v1 = src
    grid_overlay(src, lat)
    sprite_cellmap(lat)
    comparison(src, v1, v2)
    overlay(src, v2)
    print("wrote grid-overlay.png, sprite-cellmap.png, comparison-v1-v2-source.png, overlay-v2.png")


if __name__ == "__main__":
    main()
