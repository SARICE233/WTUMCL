#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Build the human-review sheets for the v2 WTUMCL icon kit.

    python design/wtumcl-logo/v2/tools/make_compare.py

Outputs (all under v2/compare/)
    compare.png       source 664 | v2 redraw @664 | 50 % overlay, 2288 px wide
    diff.png          difference heat map of the same pair, with metrics
    icon-sheet.png    every exported size on dark and light plates, plus 8x
                      nearest-neighbour zooms of 16/24 (standard and optical)
    optical-8x.png    enlarged standard-vs-optical pairing for 16 and 24 px
"""
import os

from PIL import Image, ImageDraw, ImageFont
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
V2 = os.path.dirname(HERE)
V1 = os.path.dirname(V2)
SRC_REF = os.path.join(V1, "reference", "WTUMCLOGO-reference.png")
SRC_BIG = os.path.join(V2, "png", "WTUMCL-logo-4096.png")
ICONS = os.path.join(V2, "icons")
OPTICAL = os.path.join(ICONS, "optical")
OUT = os.path.join(V2, "compare")

PANEL = 664
BG = (16, 17, 22)
LINE = (52, 54, 64)
TEXT = (234, 236, 241)
MUTED = (150, 154, 166)
ACCENT = (148, 196, 202)
SIZES = (16, 24, 32, 48, 64, 128, 256, 512, 1024)
SHEET_SIZES = (16, 24, 32, 48, 64, 128, 256)
OPT_SIZES = (16, 24)


def font(size, bold=False):
    cands = [r"C:\Windows\Fonts\segoeuib.ttf" if bold else r"C:\Windows\Fonts\segoeui.ttf",
             r"C:\Windows\Fonts\arialbd.ttf" if bold else r"C:\Windows\Fonts\arial.ttf"]
    for p in cands:
        if os.path.isfile(p):
            try:
                return ImageFont.truetype(p, size)
            except Exception:
                pass
    return ImageFont.load_default()


def premultiplied(img):
    a = np.asarray(img.convert("RGBA"), dtype=np.float32)
    out = a.copy()
    out[..., :3] = a[..., :3] * (a[..., 3:4] / 255.0)
    return Image.fromarray(np.rint(out).astype(np.uint8), "RGBA")


def unpremultiplied(img):
    a = np.asarray(img, dtype=np.float32)
    af = a[..., 3:4] / 255.0
    rgb = np.divide(a[..., :3], af, out=np.zeros_like(a[..., :3]), where=af > 1e-4)
    out = a.copy()
    out[..., :3] = np.clip(np.rint(rgb), 0, 255)
    return Image.fromarray(out.astype(np.uint8), "RGBA")


def lanczos(img, size):
    pre = premultiplied(img)
    small = pre.resize((size, size), Image.LANCZOS)
    pre.close()
    out = unpremultiplied(small)
    small.close()
    return out


def on_black(img):
    bg = Image.new("RGBA", img.size, (0, 0, 0, 255))
    return Image.alpha_composite(bg, img).convert("RGB")


def label(dr, xy, text, fnt, fill=TEXT, anchor="la"):
    dr.text(xy, text, font=fnt, fill=fill, anchor=anchor)


def frame(dr, box, color=LINE, width=1):
    dr.rectangle(box, outline=color, width=width)


def accent_rule(dr, x, y, w, color=ACCENT):
    dr.rectangle([x, y, x + w, y + 3], fill=color)


# ------------------------------------------------------------------ compare --
def build_compare(source, redraw_black):
    margin, gap = 64, 44
    width = margin * 2 + PANEL * 3 + gap * 2
    label_y, panel_y = 118, 154
    height = panel_y + PANEL + 30 + 26 + 34

    canvas = Image.new("RGB", (width, height), BG)
    dr = ImageDraw.Draw(canvas)
    f_title, f_sub, f_lab, f_foot = font(30, True), font(18), font(22, True), font(18)

    label(dr, (margin, 28), "WTUMCL logo v2 - source vs redraw vs overlay", f_title)
    label(dr, (margin, 76),
          "source: reference/WTUMCLOGO-reference.png (664x664, opaque)   |   "
          "redraw: png/WTUMCL-logo-4096.png Lanczos -> 664x664, composited on black"
          "   |   overlay: 50% blend of the two", f_sub, MUTED)

    overlay = Image.blend(source, redraw_black, 0.5)
    panels = [
        ("Left: source", "(reference bitmap, 664x664)", source),
        ("Center: v2 redraw", "(vector master rendered at 664)", redraw_black),
        ("Right: overlay", "(source 50% + redraw 50%)", overlay),
    ]
    for i, (title, sub, img) in enumerate(panels):
        x = margin + i * (PANEL + gap)
        label(dr, (x, label_y), title, f_lab)
        tw = int(dr.textlength(title, font=f_lab))
        label(dr, (x + tw + 10, label_y + 5), sub, f_sub, MUTED)
        accent_rule(dr, x, panel_y - 12, PANEL)
        canvas.paste(img, (x, panel_y))
        frame(dr, [x - 1, panel_y - 1, x + PANEL, panel_y + PANEL], LINE)

    foot = ("panels are pixel-identical in scale; the redraw is centred on the canvas while the source "
            "bitmap sits at its measured centre, so the overlay shows the deliberate centring shift.")
    label(dr, (margin, height - 26), foot, f_foot, MUTED)
    return canvas


# --------------------------------------------------------------------- diff --
def heat_diff(source, redraw_black):
    s = np.asarray(source, dtype=np.float32)
    r = np.asarray(redraw_black, dtype=np.float32)
    d = np.abs(s - r).mean(axis=2) / 255.0
    amp = np.clip(d * 3.0, 0.0, 1.0)
    base = float(90)
    heat = np.zeros_like(s)
    heat[..., 0] = 255.0
    heat[..., 1] = 212.0 - 182.0 * amp
    heat[..., 2] = 40.0 - 30.0 * amp
    out = base * (1.0 - amp[..., None]) + heat * amp[..., None]
    img = Image.fromarray(np.clip(np.rint(out), 0, 255).astype(np.uint8), "RGB")
    return img, d


def build_diff(source, heat, d):
    margin, gap = 64, 44
    width = margin * 2 + PANEL * 2 + gap
    label_y, panel_y = 118, 154
    legend_y = panel_y + PANEL + 20
    height = legend_y + 92
    canvas = Image.new("RGB", (width, height), BG)
    dr = ImageDraw.Draw(canvas)
    f_title, f_sub, f_lab, f_foot = font(30, True), font(18), font(22, True), font(18)

    label(dr, (margin, 28), "WTUMCL logo v2 - source vs redraw difference heat map", f_title)
    label(dr, (margin, 76),
          "per-pixel mean |RGB difference| amplified x3; grey = match, yellow -> red = mismatch. "
          "Source is opaque black-backed, redraw is composited on black first.", f_sub, MUTED)

    gray = source.convert("L").point(lambda v: int(56 + v * 0.55))
    gray_rgb = Image.merge("RGB", (gray, gray, gray))

    panels = [("Left: source (grey reference)", gray_rgb), ("Right: diff heat map (x3)", heat)]
    for i, (title, img) in enumerate(panels):
        x = margin + i * (PANEL + gap)
        label(dr, (x, label_y), title, f_lab)
        accent_rule(dr, x, panel_y - 12, PANEL)
        canvas.paste(img, (x, panel_y))
        frame(dr, [x - 1, panel_y - 1, x + PANEL, panel_y + PANEL], LINE)

    lx = margin + PANEL + gap
    steps = 220
    for i in range(steps):
        t = i / float(steps - 1)
        col = (255, int(212 - 182 * t), int(40 - 30 * t))
        dr.rectangle([lx + i, legend_y, lx + i + 1, legend_y + 12], fill=col)
    frame(dr, [lx - 1, legend_y - 1, lx + steps, legend_y + 13], LINE)
    label(dr, (lx, legend_y + 20), "match", f_sub, MUTED)
    label(dr, (lx + steps, legend_y + 20), "difference", f_sub, MUTED, anchor="ra")

    mean_abs = float(d.mean() * 255.0)
    big = d > (8.0 / 255.0)
    stats = ("mean |diff| = %.2f/255   |   pixels with |diff| > 8: %.2f%%   |   "
             "amplification x3, ramp 0..85/255" % (mean_abs, 100.0 * big.mean()))
    label(dr, (margin, legend_y + 58), stats, f_foot, MUTED)
    return canvas


# ---------------------------------------------------------------- icon sheet --
def load_icon(path):
    im = Image.open(path)
    im.load()
    return im.convert("RGBA")


def build_sheet():
    icons = {s: load_icon(os.path.join(ICONS, "%d.png" % s)) for s in SHEET_SIZES}
    opts = {s: load_icon(os.path.join(OPTICAL, "%d.png" % s)) for s in OPT_SIZES}

    margin, cell, row_h = 64, 280, 320
    width = margin * 2 + cell * len(SHEET_SIZES)
    top = 152
    sheet = Image.new("RGB", (width, top + row_h * 4 + 44), BG)
    dr = ImageDraw.Draw(sheet)
    f_title, f_sub, f_cell, f_note = font(30, True), font(18), font(17), font(16)

    label(dr, (margin, 28), "WTUMCL v2 icon kit - exported sizes on dark and light plates", f_title)
    label(dr, (margin, 76),
          "rows 1-2: native sizes 16..256, drawn 1:1 with no resampling; rows 3-4: 8x nearest-neighbour "
          "zoom of the 16 / 24 px icons, standard next to the whole-pixel optical build.", f_sub, MUTED)
    label(dr, (margin, 102),
          "the light rows put the transparent icon on a grey contrast plate: on plain white the white hexagon "
          "rim disappears - use master/WTUMCL-logo-inverse.svg for white backgrounds.", f_sub, MUTED)

    dark, light = (20, 21, 26), (242, 242, 245)
    cellw = width - 2 * margin
    layouts = [
        ("dark background  #14151a", dark, [(icons[s], s, 1, "%dpx" % s) for s in SHEET_SIZES]),
        ("light background  #f2f2f5  (grey contrast plate)", light,
         [(icons[s], s, 1, "%dpx" % s) for s in SHEET_SIZES]),
        ("dark background, 8x nearest zoom - standard vs optical", dark,
         [(icons[s], s, 8, "%d px standard (8x)" % s) for s in OPT_SIZES] +
         [(opts[s], s, 8, "%d px optical (8x)" % s) for s in OPT_SIZES]),
        ("light background, 8x nearest zoom - standard vs optical", light,
         [(icons[s], s, 8, "%d px standard (8x)" % s) for s in OPT_SIZES] +
         [(opts[s], s, 8, "%d px optical (8x)" % s) for s in OPT_SIZES]),
    ]
    for r, (name, bg, items) in enumerate(layouts):
        y0 = top + r * row_h
        dr.rectangle([margin, y0, width - margin, y0 + row_h - 24], fill=bg)
        fg = MUTED if bg[0] < 128 else (70, 72, 80)
        label(dr, (margin + 14, y0 + 8), name, f_cell, fg)
        band_top, band_h = y0 + 30, row_h - 100
        colw = cellw // max(len(items), 1)
        for c, (im, s, scale, lab) in enumerate(items):
            shown = im.resize((s * scale, s * scale), Image.NEAREST)
            cx = margin + int((c + 0.5) * colw)
            x = cx - shown.size[0] // 2
            y = band_top + (band_h - shown.size[1]) // 2
            if bg[0] > 128:
                dr.rectangle([x - 8, y - 8, x + shown.size[0] + 7, y + shown.size[1] + 7],
                             fill=(208, 209, 215))
            sheet.paste(shown, (x, y), shown)
            label(dr, (cx, y0 + row_h - 44), lab, f_cell, fg, anchor="ma")
    return sheet


# -------------------------------------------------------------- optical 8x --
def build_optical_sheet():
    std = {s: load_icon(os.path.join(ICONS, "%d.png" % s)) for s in OPT_SIZES}
    opt = {s: load_icon(os.path.join(OPTICAL, "%d.png" % s)) for s in OPT_SIZES}
    zoom = 12
    margin, gap = 56, 40
    cw = ch = 24 * zoom
    rows = [("dark background  #14151a", (20, 21, 26)),
            ("light background  #f2f2f5", (242, 242, 245))]
    cols = [(16, "16 px standard", std[16]), (16, "16 px optical", opt[16]),
            (24, "24 px standard", std[24]), (24, "24 px optical", opt[24])]
    width = margin * 2 + cw * 4 + gap * 3
    top = 190
    height = top + (ch + 132) * 2 + 96
    canvas = Image.new("RGB", (width, height), BG)
    dr = ImageDraw.Draw(canvas)
    f_title, f_sub, f_lab, f_note = font(30, True), font(18), font(20, True), font(16)

    label(dr, (margin, 26), "WTUMCL v2 - 16 / 24 px: standard downscale vs whole-pixel build", f_title)
    label(dr, (margin, 74),
          "left of each pair: premultiplied-alpha Lanczos of the 4096 master, where the sub-pixel hairline "
          "web fades to grey.", f_sub, MUTED)
    label(dr, (margin, 98),
          "right: the same model rasterised on the whole-pixel grid - each pixel fully transparent or an "
          "opaque palette colour, hairlines 1 px wide.", f_sub, MUTED)
    label(dr, (margin, 122),
          "12x nearest-neighbour zoom, on a dark plate and on a light plate.", f_sub, MUTED)

    for r, (name, bg) in enumerate(rows):
        y0 = top + r * (ch + 132)
        fg = MUTED if bg[0] < 128 else (70, 72, 80)
        dr.rectangle([margin, y0, width - margin, y0 + ch + 84], fill=bg)
        label(dr, (margin + 14, y0 + 8), name, f_lab, fg)
        for c, (s, lab, im) in enumerate(cols):
            shown = im.resize((s * zoom, s * zoom), Image.NEAREST)
            cx = margin + gap // 2 + c * (cw + gap) + cw // 2
            x = cx - shown.size[0] // 2
            y = y0 + 40 + (ch - shown.size[1])
            if bg[0] > 128:
                dr.rectangle([x - 10, y - 10, x + shown.size[0] + 9, y + shown.size[1] + 9],
                             fill=(208, 209, 215))
            canvas.paste(shown, (x, y), shown)
            label(dr, (cx, y0 + ch + 52), lab, f_note, fg, anchor="ma")

    label(dr, (margin, height - 62),
          "optical rule: 8x supersampled coverage thresholded at 50 %, sprite snapped to the nearest palette "
          "entry, hairline web simplified per size", f_note, MUTED)
    label(dr, (margin, height - 38),
          "(16 px: main axes; 24 px: axes + rails + basket).  Rebuild with tools/make_icons.py; rebuilt by "
          "tools/make_compare.py.", f_note, MUTED)
    return canvas


def main():
    os.makedirs(OUT, exist_ok=True)
    for p in (SRC_REF, SRC_BIG):
        if not os.path.isfile(p):
            raise SystemExit("missing input: %s" % p)

    source = Image.open(SRC_REF)
    source.load()
    source = source.convert("RGB")
    big = Image.open(SRC_BIG)
    big.load()
    redraw = lanczos(big, PANEL)
    redraw_black = on_black(redraw)

    cmp_img = build_compare(source, redraw_black)
    p = os.path.join(OUT, "compare.png")
    cmp_img.save(p, format="PNG", optimize=True)
    print("wrote %-28s %dx%d" % (os.path.basename(p), cmp_img.size[0], cmp_img.size[1]))

    heat, d = heat_diff(source, redraw_black)
    diff_img = build_diff(source, heat, d)
    p = os.path.join(OUT, "diff.png")
    diff_img.save(p, format="PNG", optimize=True)
    print("wrote %-28s %dx%d   mean |diff| = %.2f/255, pixels>8 = %.2f%%" % (
        os.path.basename(p), diff_img.size[0], diff_img.size[1],
        float(d.mean() * 255.0), 100.0 * float((d > 8.0 / 255).mean())))

    sheet = build_sheet()
    p = os.path.join(OUT, "icon-sheet.png")
    sheet.save(p, format="PNG", optimize=True)
    print("wrote %-28s %dx%d" % (os.path.basename(p), sheet.size[0], sheet.size[1]))

    opt = build_optical_sheet()
    p = os.path.join(OUT, "optical-8x.png")
    opt.save(p, format="PNG", optimize=True)
    print("wrote %-28s %dx%d" % (os.path.basename(p), opt.size[0], opt.size[1]))


if __name__ == "__main__":
    main()
